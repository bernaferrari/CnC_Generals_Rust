//! # Particle System Implementation
//!
//! Complete particle system implementation matching C++ behavior exactly.
//! Handles particle creation, physics simulation, and lifecycle management.

use nalgebra::{Matrix3, Point3, Vector3};
use rand::prelude::*;
use std::collections::VecDeque;
use std::sync::Arc;

use super::particle_manager::*;
use crate::core::DrawableId;
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};

/// Individual particle information (matches C++ ParticleInfo)
#[derive(Debug, Clone)]
pub struct ParticleInfo {
    pub velocity: Vector3<f32>,
    pub position: Point3<f32>,
    pub emitter_position: Point3<f32>,
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
            velocity: Vector3::zeros(),
            position: Point3::origin(),
            emitter_position: Point3::origin(),
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
    // Basic properties from ParticleInfo
    pub velocity: Vector3<f32>,
    pub position: Point3<f32>,
    pub emitter_position: Point3<f32>,
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
    pub acceleration: Vector3<f32>,
    pub last_position: Point3<f32>,
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
            acceleration: Vector3::zeros(),
            last_position: info.position,
            lifetime_left: info.lifetime,
            create_timestamp,

            alpha: 1.0,
            alpha_rate: 0.0,
            alpha_target_key: 1,

            color: [1.0, 1.0, 1.0],
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

        // Set initial alpha and color from first keyframes
        if particle.alpha_keys[0].frame > 0 {
            particle.alpha = particle.alpha_keys[0].value;
            particle.compute_alpha_rate();
        }

        if particle.color_keys[0].frame > 0 {
            particle.color = particle.color_keys[0].color;
            particle.compute_color_rate();
        }

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
        drift_velocity: Vector3<f32>,
        current_frame: u32,
        shader_type: ParticleShaderType,
    ) -> bool {
        if self.lifetime_left == 0 {
            return false;
        }

        // Integrate acceleration into velocity (C++ lines 316-318)
        self.velocity += self.acceleration;

        // Apply velocity damping (C++ lines 320-322)
        self.velocity.x *= self.vel_damping;
        self.velocity.y *= self.vel_damping;
        self.velocity.z *= self.vel_damping;

        // Store last position for interpolation
        self.last_position = self.position;

        // Integrate velocity into position with drift velocity (C++ lines 325-327)
        // CRITICAL: drift_velocity is applied directly, NOT as force
        self.position.x += self.velocity.x + drift_velocity.x;
        self.position.y += self.velocity.y + drift_velocity.y;
        self.position.z += self.velocity.z + drift_velocity.z;

        // Update rotation (C++ lines 336-337)
        self.angle_z += self.angular_rate_z;
        self.angular_rate_z *= self.angular_damping;

        // Handle particleUpTowardsEmitter rotation (C++ lines 339-348)
        // This adjusts rotation so 0 degrees points toward the emitter
        if self.particle_up_towards_emitter {
            let emitter_dir_x = self.position.x - self.emitter_position.x;
            let emitter_dir_y = self.position.y - self.emitter_position.y;
            let emitter_len =
                (emitter_dir_x * emitter_dir_x + emitter_dir_y * emitter_dir_y).sqrt();

            if emitter_len > 0.0 {
                // Calculate angle from up vector (0,1) to emitter direction
                let up_x = 0.0f32;
                let up_y = 1.0f32;
                let norm_x = emitter_dir_x / emitter_len;
                let norm_y = emitter_dir_y / emitter_len;

                // Angle between (0,1) and (norm_x, norm_y) using atan2
                let angle_to_emitter = norm_y.atan2(norm_x);
                self.angle_z = angle_to_emitter + std::f32::consts::PI;
            }
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

        // Clamp color components to [0,1] (C++ lines 430-444)
        self.color[0] = self.color[0].clamp(0.0, 1.0);
        self.color[1] = self.color[1].clamp(0.0, 1.0);
        self.color[2] = self.color[2].clamp(0.0, 1.0);

        // Clear acceleration for next frame (C++ line 447)
        self.acceleration = Vector3::zeros();

        // Monitor lifetime (C++ lines 450-451)
        if self.lifetime_left > 0 {
            self.lifetime_left -= 1;
        }

        if self.lifetime_left == 0 {
            return false;
        }

        // Check if invisible (C++ lines 454-455)
        if self.is_invisible(shader_type) {
            return false;
        }

        true
    }

    /// Apply force to particle (matches C++ Particle::applyForce)
    pub fn apply_force(&mut self, force: Vector3<f32>) {
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
    pub fn do_wind_motion(&mut self, wind_angle: f32, system_pos: Point3<f32>) {
        // C++ constants for wind force (lines 501-502)
        const FULL_FORCE_DISTANCE: f32 = 75.0;
        const NO_FORCE_DISTANCE: f32 = 200.0;

        // Calculate distance from emitter to particle (C++ lines 518-519)
        let dx = self.position.x - system_pos.x;
        let dy = self.position.y - system_pos.y;
        let dz = self.position.z - system_pos.z;
        let dist_from_wind = (dx * dx + dy * dy + dz * dz).sqrt();

        // Only apply force if within the circle of influence (C++ line 524)
        if dist_from_wind < NO_FORCE_DISTANCE {
            // Base wind force strength (C++ line 526)
            let mut wind_force_strength = 2.0 * self.wind_randomness;

            // Reduce force with distance (C++ lines 529-531)
            if dist_from_wind > FULL_FORCE_DISTANCE {
                wind_force_strength *= 1.0
                    - ((dist_from_wind - FULL_FORCE_DISTANCE)
                        / (NO_FORCE_DISTANCE - FULL_FORCE_DISTANCE));
            }

            // Apply wind motion directly to position (C++ lines 534-535)
            // NOT as force/acceleration - this is intentional for visual effect
            self.position.x += wind_angle.cos() * wind_force_strength;
            self.position.y += wind_angle.sin() * wind_force_strength;
        }
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
            ParticleShaderType::Additive => {
                // If color is black, particle is invisible for additive blending (C++ lines 468-476)
                // Check that we're not transitioning to another color
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
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
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
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
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
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
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
        self.alpha_target_key = alpha_target_key.max(0) as usize;

        for component in &mut self.color {
            xfer.xfer_real(component).map_err(|e| e.to_string())?;
        }
        for component in &mut self.color_rate {
            xfer.xfer_real(component).map_err(|e| e.to_string())?;
        }

        let mut color_target_key = self.color_target_key as i32;
        xfer.xfer_int(&mut color_target_key)
            .map_err(|e| e.to_string())?;
        self.color_target_key = color_target_key.max(0) as usize;

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
        self.alpha_target_key = self.alpha_target_key.min(MAX_KEYFRAMES.saturating_sub(1));
        self.color_target_key = self.color_target_key.min(MAX_KEYFRAMES.saturating_sub(1));
        Ok(())
    }
}

impl Snapshotable for ParticleSystem {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        let mut system_info = self.template.info().clone();
        system_info.wind_angle = self.wind_angle;
        system_info.wind_angle_change = self.wind_angle_change;
        system_info.wind_motion_moving_to_end_angle = self.wind_motion_moving_to_end_angle;
        system_info.xfer(xfer)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.wind_angle = system_info.wind_angle;
            self.wind_angle_change = system_info.wind_angle_change;
            self.wind_motion_moving_to_end_angle = system_info.wind_motion_moving_to_end_angle;
            Arc::make_mut(&mut self.template)
                .info_mut()
                .clone_from(&system_info);
        }

        xfer.xfer_unsigned_int(&mut self.system_id)
            .map_err(|e| e.to_string())?;

        let mut attached_drawable_id = self.attached_drawable_id.0;
        xfer.xfer_drawable_id(&mut attached_drawable_id)
            .map_err(|e| e.to_string())?;
        self.attached_drawable_id = DrawableId(attached_drawable_id);

        xfer.xfer_object_id(&mut self.attached_object_id)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_local_identity)
            .map_err(|e| e.to_string())?;
        xfer_matrix3(xfer, &mut self.local_transform)?;
        xfer.xfer_bool(&mut self.is_identity)
            .map_err(|e| e.to_string())?;
        xfer_matrix3(xfer, &mut self.transform)?;
        xfer.xfer_unsigned_int(&mut self.burst_delay_left)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.delay_left)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.start_timestamp)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.system_lifetime_left)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.personality_counter)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_forever)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.accumulated_size_bonus)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_stopped)
            .map_err(|e| e.to_string())?;
        xfer_vec3(xfer, &mut self.vel_coeff)?;
        xfer.xfer_real(&mut self.count_coeff)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.delay_coeff)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.size_coeff)
            .map_err(|e| e.to_string())?;
        xfer_point3(xfer, &mut self.position)?;
        xfer_point3(xfer, &mut self.last_position)?;
        xfer.xfer_bool(&mut self.is_first_pos)
            .map_err(|e| e.to_string())?;

        let mut slave_system = self.slave_system.unwrap_or(INVALID_PARTICLE_SYSTEM_ID);
        let mut master_system = self.master_system.unwrap_or(INVALID_PARTICLE_SYSTEM_ID);
        xfer.xfer_unsigned_int(&mut slave_system)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut master_system)
            .map_err(|e| e.to_string())?;
        self.slave_system = if slave_system == INVALID_PARTICLE_SYSTEM_ID {
            None
        } else {
            Some(slave_system)
        };
        self.master_system = if master_system == INVALID_PARTICLE_SYSTEM_ID {
            None
        } else {
            Some(master_system)
        };

        let mut particle_count = self.particles.len() as u32;
        xfer.xfer_unsigned_int(&mut particle_count)
            .map_err(|e| e.to_string())?;

        if xfer.get_xfer_mode() == XferMode::Load {
            self.particles.clear();
            for _ in 0..particle_count {
                let mut particle = Particle::new(&ParticleInfo::default(), 0, 0);
                particle.xfer(xfer)?;
                self.particles.push_back(particle);
            }
        } else {
            for particle in &mut self.particles {
                particle.xfer(xfer)?;
            }
        }

        self.particle_count = self.particles.len();
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.control_particle = None;
        self.particle_count = self.particles.len();

        for (index, particle) in self.particles.iter_mut().enumerate() {
            particle.load_post_process()?;
            if particle.controlled_system.is_some() {
                self.control_particle = Some(index);
            }
        }

        Ok(())
    }
}

/// Merge master and slave particle systems to produce a combined ParticleInfo
/// for the slave particle (matches C++ ParticleSystem::mergeRelatedParticleSystems).
///
/// This is a standalone function because the manager needs to call it with
/// immutable references to two systems, then create particles on the slave.
///
/// # Arguments
/// * `master` - The master particle system
/// * `slave` - The slave particle system
/// * `slave_needs_full_promotion` - If true, slave system's burst/velocity/volume
///   params get promoted from master (C++ line 2317)
pub(crate) fn merge_related_particle_systems(
    master: &ParticleSystem,
    slave: &ParticleSystem,
    slave_needs_full_promotion: bool,
) -> ParticleInfo {
    // Generate fresh particle info from master (C++ line 2286)
    let mut merge_info = master.generate_particle_info(1, 1).unwrap_or_default();

    // Generate fresh particle info from slave (C++ line 2289)
    let slave_info = slave.generate_particle_info(1, 1).unwrap_or_default();

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
    let offset = slave.slave_position_offset();
    merge_info.position.x += offset.x;
    merge_info.position.y += offset.y;
    merge_info.position.z += offset.z;

    // Full promotion: copy burst/velocity/volume from master to slave system
    // (C++ lines 2317-2345). This requires &mut access to the slave, which
    // the manager handles separately after this function returns.
    let _ = slave_needs_full_promotion; // Handled by the manager if needed

    merge_info
}

fn xfer_vec3(xfer: &mut dyn Xfer, value: &mut Vector3<f32>) -> Result<(), String> {
    xfer.xfer_real(&mut value.x).map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut value.y).map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut value.z).map_err(|e| e.to_string())?;
    Ok(())
}

fn xfer_point3(xfer: &mut dyn Xfer, value: &mut Point3<f32>) -> Result<(), String> {
    xfer.xfer_real(&mut value.x).map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut value.y).map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut value.z).map_err(|e| e.to_string())?;
    Ok(())
}

fn xfer_matrix3(xfer: &mut dyn Xfer, value: &mut Matrix3<f32>) -> Result<(), String> {
    for row in 0..3 {
        for col in 0..3 {
            xfer.xfer_real(&mut value[(row, col)])
                .map_err(|e| e.to_string())?;
        }
        let mut translation = 0.0f32;
        xfer.xfer_real(&mut translation)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
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
    xfer_cpp_enum(
        xfer,
        &mut raw,
        |v| (1..=4).contains(&v),
        ParticleShaderType::Alpha as u32,
    )?;
    *value = match raw {
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
    xfer_cpp_enum(
        xfer,
        &mut raw,
        |v| (1..=5).contains(&v),
        ParticleType::Particle as u32,
    )?;
    *value = match raw {
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
        |v| (1..=5).contains(&v),
        EmissionVelocityType::Spherical as u32,
    )?;
    *value = match raw {
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
    xfer_cpp_enum(
        xfer,
        &mut raw,
        |v| (1..=5).contains(&v),
        EmissionVolumeType::Point as u32,
    )?;
    *value = match raw {
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
    xfer_cpp_enum(
        xfer,
        &mut raw,
        |v| (1..=3).contains(&v),
        WindMotion::NotUsed as u32,
    )?;
    *value = match raw {
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
        EmissionVolumeType::Point => {
            *volume = EmissionVolume::Point;
        }
        EmissionVolumeType::Line => {
            let (mut start, mut end) = match *volume {
                EmissionVolume::Line { start, end } => (start, end),
                _ => (Point3::origin(), Point3::origin()),
            };
            xfer_point3(xfer, &mut start)?;
            xfer_point3(xfer, &mut end)?;
            *volume = EmissionVolume::Line { start, end };
        }
        EmissionVolumeType::Box => {
            let mut half_size = match *volume {
                EmissionVolume::Box { half_size } => half_size,
                _ => Vector3::zeros(),
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

/// Particle system (matches C++ ParticleSystem)
pub struct ParticleSystem {
    template: Arc<ParticleSystemTemplate>,
    system_id: ParticleSystemId,

    // Particle storage
    particles: VecDeque<Particle>,
    particle_count: usize,
    personality_counter: u32,

    // Attachment
    attached_drawable_id: DrawableId,
    attached_object_id: ObjectId,

    // Transform
    local_transform: Matrix3<f32>,
    transform: Matrix3<f32>,
    position: Point3<f32>,
    last_position: Point3<f32>,

    // Timing
    burst_delay_left: u32,
    delay_left: u32,
    start_timestamp: u32,
    system_lifetime_left: u32,

    // Size accumulation for StartSizeRate
    accumulated_size_bonus: f32,

    // Coefficients for scaling
    vel_coeff: Vector3<f32>,
    count_coeff: f32,
    delay_coeff: f32,
    size_coeff: f32,

    // Slave system
    slave_system: Option<ParticleSystemId>,
    master_system: Option<ParticleSystemId>,

    // Control particle
    control_particle: Option<usize>, // Index into particles vector

    // State flags
    is_local_identity: bool,
    is_identity: bool,
    is_forever: bool,
    is_stopped: bool,
    is_destroyed: bool,
    is_first_pos: bool,
    is_saveable: bool,
    skip_parent_transform: bool,

    // Wind motion state
    wind_angle: f32,
    wind_angle_change: f32,
    wind_motion_moving_to_end_angle: bool,
    wind_motion_start_angle: f32,
    wind_motion_start_angle_min: f32,
    wind_motion_start_angle_max: f32,
    wind_motion_end_angle: f32,
    wind_motion_end_angle_min: f32,
    wind_motion_end_angle_max: f32,

    // Shroud/visibility state (C++ ParticleSystem::m_isShrouded)
    is_shrouded: bool,

    // Parent transform override for attached systems
    parent_transform: Option<Matrix3<f32>>,

    // Particle scale multiplier
    particle_scale: f32,

    // Emission overrides
    emission_volume_override: Option<EmissionVolume>,
    emission_volume_type_override: Option<EmissionVolumeType>,

    // Slave emission buffer: during emit_particles, if a slave system is present,
    // the emitted particle count is recorded here so the manager can create
    // corresponding slave particles in a separate pass (avoids double-&mut borrow).
    // Matches C++ ParticleSystem::update lines 2004-2009.
    slave_emission_count: u32,
}

impl ParticleSystem {
    /// Create a new particle system (matches C++ ParticleSystem constructor)
    pub fn new(
        template: Arc<ParticleSystemTemplate>,
        system_id: ParticleSystemId,
        create_slaves: bool,
    ) -> Self {
        let info = template.info();

        let mut system = Self {
            template: template.clone(),
            system_id,

            particles: VecDeque::new(),
            particle_count: 0,
            personality_counter: 0,

            attached_drawable_id: DrawableId::INVALID,
            attached_object_id: 0,

            local_transform: Matrix3::identity(),
            transform: Matrix3::identity(),
            position: Point3::origin(),
            last_position: Point3::origin(),

            burst_delay_left: info.burst_delay.sample() as u32,
            delay_left: info.initial_delay.sample() as u32,
            start_timestamp: 0, // Will be set when started
            system_lifetime_left: info.system_lifetime,

            accumulated_size_bonus: 0.0,

            vel_coeff: Vector3::new(1.0, 1.0, 1.0),
            count_coeff: 1.0,
            delay_coeff: 1.0,
            size_coeff: 1.0,

            slave_system: None,
            master_system: None,

            control_particle: None,

            is_local_identity: true,
            is_identity: true,
            is_forever: info.system_lifetime == 0,
            is_stopped: false,
            is_destroyed: false,
            is_first_pos: true,
            is_saveable: true,
            skip_parent_transform: false,

            wind_angle: info.wind_angle,
            wind_angle_change: info.wind_angle_change,
            wind_motion_moving_to_end_angle: false,
            wind_motion_start_angle: info.wind_motion_start_angle_min,
            wind_motion_start_angle_min: info.wind_motion_start_angle_min,
            wind_motion_start_angle_max: info.wind_motion_start_angle_max,
            wind_motion_end_angle: info.wind_motion_end_angle_min,
            wind_motion_end_angle_min: info.wind_motion_end_angle_min,
            wind_motion_end_angle_max: info.wind_motion_end_angle_max,

            is_shrouded: false,
            parent_transform: None,
            particle_scale: 1.0,

            emission_volume_override: None,
            emission_volume_type_override: None,

            slave_emission_count: 0,
        };

        // Initialize wind motion
        system.initialize_wind_motion();

        system
    }

    /// Get immutable access to particles
    pub fn particles(&self) -> &VecDeque<Particle> {
        &self.particles
    }

    /// Get mutable access to particles  
    pub fn particles_mut(&mut self) -> &mut VecDeque<Particle> {
        &mut self.particles
    }

    /// Update the particle system (matches C++ ParticleSystem::update)
    ///
    /// # Arguments
    /// * `local_player_index` - Player index for visibility checks
    /// * `current_frame` - Current game frame for timing
    pub fn update(&mut self, _local_player_index: i32, current_frame: u32) -> bool {
        if self.is_destroyed && self.particles.is_empty() {
            return false;
        }

        // Update wind motion (C++ line 1978)
        self.update_wind_motion();

        // C++ parity: parent transform concatenation (ParticleSys.cpp lines 1847-1932)
        self.update_transform_from_parent();

        // Update existing particles (C++ lines 2143-2158)
        self.update_particles(current_frame);

        // C++ parity: ParticleSys.cpp line 1970
        if !self.is_stopped && !self.is_destroyed && !self.is_shrouded {
            self.emit_particles(current_frame);
        }

        // Update timing
        if !self.is_forever {
            if self.system_lifetime_left > 0 {
                self.system_lifetime_left -= 1;
            } else if !self.is_destroyed {
                self.destroy();
            }
        }

        // System is alive if it has particles or isn't destroyed
        !self.particles.is_empty() || !self.is_destroyed
    }

    /// Set position (matches C++ ParticleSystem::setPosition)
    pub fn set_position(&mut self, pos: Point3<f32>) {
        if self.is_first_pos {
            self.last_position = pos;
            self.is_first_pos = false;
        } else {
            self.last_position = self.position;
        }
        self.position = pos;
    }

    /// Get position (matches C++ ParticleSystem::getPosition)
    pub fn position(&self) -> Point3<f32> {
        self.position
    }

    /// Get emission volume type (uses overrides if present).
    pub fn get_emission_volume_type(&self) -> EmissionVolumeType {
        if let Some(kind) = self.emission_volume_type_override {
            return kind;
        }
        self.template.info().emission_volume_type
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
    pub fn set_local_transform(&mut self, matrix: Matrix3<f32>) {
        self.local_transform = matrix;
        self.is_local_identity = matrix == Matrix3::identity();
        self.update_transform();
    }

    /// Scale particle launch velocity for this system instance.
    pub fn set_velocity_multiplier(&mut self, multiplier: Vector3<f32>) {
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
        self.template.info().shader_type
    }

    /// Get drift velocity
    pub fn drift_velocity(&self) -> Vector3<f32> {
        self.template.info().drift_velocity
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
        &self.template.info().particle_type_name
    }

    /// Get slave position offset
    pub fn slave_position_offset(&self) -> Vector3<f32> {
        self.template.info().slave_pos_offset
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

    /// Drain the count of slave particles emitted in the last update pass.
    /// The manager calls this after updating each system to create slave particles.
    pub fn drain_slave_emission_count(&mut self) -> u32 {
        std::mem::take(&mut self.slave_emission_count)
    }

    /// Set skip parent transform
    pub fn set_skip_parent_xfrm(&mut self, enable: bool) {
        self.skip_parent_transform = enable;
    }

    /// Set shroud visibility state. C++ parity: ParticleSys.cpp lines 1860-1884
    pub fn set_shrouded(&mut self, shrouded: bool) {
        self.is_shrouded = shrouded;
    }

    /// Set parent transform matrix. C++ parity: ParticleSys.cpp lines 1863, 1886-1890
    pub fn set_parent_transform(&mut self, transform: Option<Matrix3<f32>>) {
        self.parent_transform = transform;
    }

    pub fn update_particle_scale(&mut self) {
        self.particle_scale = Self::read_particle_scale_from_global_data();
    }

    fn read_particle_scale_from_global_data() -> f32 {
        match game_engine::common::ini::ini_game_data::get_global_data() {
            Some(arc) => arc.read().particle_scale,
            None => 1.0,
        }
    }

    /// Get velocity multiplier
    pub fn velocity_multiplier(&self) -> Vector3<f32> {
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

    /// Get burst count multiplier
    pub fn burst_count_multiplier(&self) -> f32 {
        self.count_coeff
    }

    /// Should billboard
    pub fn should_billboard(&self) -> bool {
        !self.template.info().is_ground_aligned
    }

    /// Get start frame
    pub fn start_frame(&self) -> u32 {
        self.start_timestamp
    }

    /// Set initial delay
    pub fn set_initial_delay(&mut self, delay: u32) {
        self.delay_left = delay;
    }

    /// Set lifetime range
    pub fn set_lifetime_range(&mut self, min: f32, max: f32) {
        let info = self.template.info();
        let mut info = info.clone();
        info.lifetime = GameClientRandomVariable::new(min, max);
    }

    /// Rotate local transform X (matches C++ ParticleSystem::rotateLocalTransformX)
    pub fn rotate_local_transform_x(&mut self, angle: f32) {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let rot = Matrix3::from_columns(&[
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, cos_a, sin_a),
            Vector3::new(0.0, -sin_a, cos_a),
        ]);
        self.local_transform = rot * self.local_transform;
        self.is_local_identity = false;
        self.update_transform();
    }

    /// Rotate local transform Y (matches C++ ParticleSystem::rotateLocalTransformY)
    pub fn rotate_local_transform_y(&mut self, angle: f32) {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let rot = Matrix3::from_columns(&[
            Vector3::new(cos_a, 0.0, -sin_a),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(sin_a, 0.0, cos_a),
        ]);
        self.local_transform = rot * self.local_transform;
        self.is_local_identity = false;
        self.update_transform();
    }

    /// Rotate local transform Z (matches C++ ParticleSystem::rotateLocalTransformZ)
    pub fn rotate_local_transform_z(&mut self, angle: f32) {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let rot = Matrix3::from_columns(&[
            Vector3::new(cos_a, sin_a, 0.0),
            Vector3::new(-sin_a, cos_a, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
        ]);
        self.local_transform = rot * self.local_transform;
        self.is_local_identity = false;
        self.update_transform();
    }

    /// Start the particle system (matches C++ ParticleSystem::start)
    pub fn start(&mut self) {
        self.is_stopped = false;
        self.is_destroyed = false;
        self.start_timestamp = 0;
    }

    /// Stop the particle system (matches C++ ParticleSystem::stop)
    pub fn stop(&mut self) {
        self.is_stopped = true;
    }

    /// Destroy the particle system (matches C++ ParticleSystem::destroy)
    /// In C++, this cascades to slave via m_slaveSystem->destroy().
    /// In Rust, the manager handles cascading via slave_system ID.
    pub fn destroy(&mut self) {
        self.is_stopped = true;
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
        self.particle_count
    }

    /// Get system priority for C++-style particle budget culling.
    pub fn priority(&self) -> ParticlePriorityType {
        self.template.info().priority
    }

    /// Get personality counter (for creating particles externally, e.g., slave emissions)
    pub fn personality_counter(&self) -> u32 {
        self.personality_counter
    }

    /// Push a pre-created particle into the system (used by manager for slave emission)
    pub fn push_particle(&mut self, particle: Particle) {
        self.personality_counter += 1;
        self.particle_count += 1;
        self.particles.push_back(particle);
    }

    /// Remove oldest particles from the front of this system's queue.
    pub fn remove_oldest_particles(&mut self, count: usize) -> usize {
        let remove_count = count.min(self.particle_count).min(self.particles.len());
        for _ in 0..remove_count {
            self.particles.pop_front();
        }
        self.particle_count -= remove_count;
        remove_count
    }

    /// Get wind angle
    pub fn wind_angle(&self) -> f32 {
        self.wind_angle
    }

    /// Update particles (matches C++ ParticleSystem::update particle loop)
    fn update_particles(&mut self, current_frame: u32) {
        let mut i = 0;

        // Cache values from template
        let gravity = self.template.info().gravity;
        let drift_velocity = self.template.info().drift_velocity;
        let wind_angle = self.wind_angle();
        let wind_motion = self.template.info().wind_motion;
        let shader_type = self.template.info().shader_type;
        let system_pos = self.position;

        while i < self.particles.len() {
            let mut remove_particle = false;

            {
                let particle = &mut self.particles[i];

                // Apply gravity as force (C++ update loop lines 2144-2149)
                // Note: C++ uses positive gravity for downward force (z decreases)
                if gravity != 0.0 {
                    particle.apply_force(Vector3::new(0.0, 0.0, gravity));
                }

                // Do wind motion if enabled (applied directly to position, not as force)
                // C++ handles this in Particle::doWindMotion, called from Particle::update
                if wind_motion != WindMotion::NotUsed {
                    particle.do_wind_motion(wind_angle, system_pos);
                }

                // Update particle with correct parameters matching C++ signature
                if !particle.update(drift_velocity, current_frame, shader_type) {
                    remove_particle = true;
                }
            }

            if remove_particle {
                self.particles.remove(i);
                self.particle_count = self.particle_count.saturating_sub(1);
            } else {
                i += 1;
            }
        }
    }

    /// Emit particles (matches C++ ParticleSystem::update emission logic)
    fn emit_particles(&mut self, current_frame: u32) {
        // Check initial delay (C++ lines 1980-1989)
        if self.delay_left > 0 {
            self.delay_left -= 1;
            // When delay finishes, set start timestamp (C++ line 1988)
            if self.delay_left == 0 {
                self.start_timestamp = current_frame;
            }
            return;
        }

        // Check burst delay (C++ lines 2037-2040)
        if self.burst_delay_left > 0 {
            self.burst_delay_left -= 1;
            return;
        }

        // Time to emit particles (C++ lines 1992-2034)
        let info = self.template.info();
        let burst_count = (info.burst_count.sample() * self.count_coeff) as u32;

        // Reset slave emission count for this burst
        self.slave_emission_count = 0;

        for i in 0..burst_count {
            if let Some(_particle_info) = self.generate_particle_info(i, burst_count) {
                // Create particle with current frame as creation timestamp (C++ line 287)
                let particle =
                    Particle::new(&_particle_info, self.personality_counter, current_frame);
                self.particles.push_back(particle);
                self.particle_count += 1;
                self.personality_counter += 1;

                // Track slave emission count (C++ lines 2004-2009)
                // The actual slave particle creation is handled by the manager
                // because we can't mutably borrow two systems simultaneously.
                if self.slave_system.is_some() {
                    self.slave_emission_count += 1;
                }
            }
        }

        // Reset burst delay (C++ lines 2036-2037)
        self.burst_delay_left = (info.burst_delay.sample() * self.delay_coeff) as u32;

        // Update accumulated size bonus (C++ line 1800)
        self.accumulated_size_bonus += info.start_size_rate.sample();

        // Clamp accumulated bonus (C++ lines 1801-1802)
        const MAX_SIZE_BONUS: f32 = 50.0;
        if self.accumulated_size_bonus > MAX_SIZE_BONUS {
            self.accumulated_size_bonus = MAX_SIZE_BONUS;
        }

        // Check if one-shot (C++ doesn't auto-stop on one-shot, but stops emitting)
        // The is_stopped flag prevents further emission
        if info.is_one_shot {
            self.is_stopped = true;
        }
    }

    /// Generate particle info (matches C++ ParticleSystem::generateParticleInfo)
    /// pub(crate) so the manager can call it for slave merge processing.
    pub(crate) fn generate_particle_info(
        &self,
        particle_num: u32,
        particle_count: u32,
    ) -> Option<ParticleInfo> {
        let info = self.template.info();

        let mut particle_info = ParticleInfo::default();

        // Position
        particle_info.position = self.compute_particle_position();
        particle_info.emitter_position = self.position;

        // C++ parity: ParticleSys.cpp lines 1518-1520
        // C++: newVel *= m_velCoeff * (0.5f + m_particleScale/2.0f)
        particle_info.velocity = self.compute_particle_velocity(&particle_info.position);
        let vel_scale = 0.5 + self.particle_scale / 2.0;
        particle_info.velocity.component_mul_assign(&self.vel_coeff);
        particle_info.velocity *= vel_scale;

        // Lifetime
        particle_info.lifetime = info.lifetime.sample() as u32;

        // C++ parity: ParticleSys.cpp lines 1782-1783
        // C++: m_size = m_startSize*m_sizeCoeff*TheGlobalData->m_particleScale
        // C++: m_sizeRate = m_sizeRate*m_sizeCoeff*TheGlobalData->m_particleScale
        particle_info.size = (info.start_size.sample() + self.accumulated_size_bonus)
            * self.size_coeff
            * self.particle_scale;
        particle_info.size_rate = info.size_rate.sample() * self.size_coeff * self.particle_scale;
        particle_info.size_rate_damping = info.size_rate_damping.sample();

        // Angles
        particle_info.angle_z = info.angle_z.sample();
        particle_info.angular_rate_z = info.angular_rate_z.sample();
        particle_info.angular_damping = info.angular_damping.sample();

        // Damping
        particle_info.vel_damping = info.vel_damping.sample();
        let mut rng = thread_rng();

        // Copy keyframes (sample random alpha keyframes into fixed runtime values).
        for (dst, src) in particle_info
            .alpha_keys
            .iter_mut()
            .zip(info.alpha_keys.iter())
        {
            dst.value = rng.gen_range(src.min_value..=src.max_value);
            dst.frame = src.frame;
        }
        particle_info.color_keys = info.color_keys;

        // Color scale
        particle_info.color_scale = info.color_scale.sample();

        // Wind randomness
        particle_info.wind_randomness = rng.gen_range(0.5..=1.5);

        // Particle up towards emitter
        particle_info.particle_up_towards_emitter = info.is_particle_up_towards_emitter;

        Some(particle_info)
    }

    /// Compute particle position based on emission volume (matches C++ ParticleSystem::computeParticlePosition)
    fn compute_particle_position(&self) -> Point3<f32> {
        let info = self.template.info();
        let mut rng = thread_rng();

        let emission_volume = self.effective_emission_volume();
        let local_pos = match emission_volume {
            EmissionVolume::Point => Vector3::zeros(),

            EmissionVolume::Line { start, end } => {
                let t = rng.gen::<f32>();
                (end - start) * t
            }

            EmissionVolume::Box { half_size } => {
                if info.is_emission_volume_hollow {
                    // Match C++ bug exactly: side % 3 == 1 uses halfSize.y for X (C++ line 1597)
                    let side = rng.gen_range(0..6);
                    if side % 3 == 0 {
                        // Bottom or top face (Z = -/+halfSize.z)
                        Vector3::new(
                            rng.gen_range(-half_size.x..=half_size.x),
                            rng.gen_range(-half_size.y..=half_size.y),
                            if side == 0 { -half_size.z } else { half_size.z },
                        )
                    } else if side % 3 == 1 {
                        // Left or right face (X = -/+halfSize.x)
                        // C++ bug: uses halfSize.y instead of halfSize.x for X coordinate
                        Vector3::new(
                            if side == 1 { -half_size.x } else { half_size.y },
                            rng.gen_range(-half_size.y..=half_size.y),
                            rng.gen_range(-half_size.z..=half_size.z),
                        )
                    } else {
                        // Front or back face (Y = -/+halfSize.y)
                        Vector3::new(
                            rng.gen_range(-half_size.x..=half_size.x),
                            if side == 2 { -half_size.y } else { half_size.y },
                            rng.gen_range(-half_size.z..=half_size.z),
                        )
                    }
                } else {
                    Vector3::new(
                        rng.gen_range(-half_size.x..=half_size.x),
                        rng.gen_range(-half_size.y..=half_size.y),
                        rng.gen_range(-half_size.z..=half_size.z),
                    )
                }
            }

            EmissionVolume::Sphere { radius } => {
                if info.is_emission_volume_hollow {
                    // On sphere surface
                    let theta = rng.gen::<f32>() * 2.0 * std::f32::consts::PI;
                    let phi = rng.gen::<f32>() * std::f32::consts::PI;
                    Vector3::new(
                        radius * phi.sin() * theta.cos(),
                        radius * phi.sin() * theta.sin(),
                        radius * phi.cos(),
                    )
                } else {
                    // Inside sphere
                    let r = rng.gen::<f32>().powf(1.0 / 3.0) * radius;
                    let theta = rng.gen::<f32>() * 2.0 * std::f32::consts::PI;
                    let phi = rng.gen::<f32>() * std::f32::consts::PI;
                    Vector3::new(
                        r * phi.sin() * theta.cos(),
                        r * phi.sin() * theta.sin(),
                        r * phi.cos(),
                    )
                }
            }

            EmissionVolume::Cylinder { radius, length } => {
                let half_length = length * 0.5;
                let z = rng.gen_range(-half_length..=half_length);

                if info.is_emission_volume_hollow {
                    // On cylinder surface
                    let theta = rng.gen::<f32>() * 2.0 * std::f32::consts::PI;
                    Vector3::new(radius * theta.cos(), radius * theta.sin(), z)
                } else {
                    // Inside cylinder
                    let r = rng.gen::<f32>().sqrt() * radius;
                    let theta = rng.gen::<f32>() * 2.0 * std::f32::consts::PI;
                    Vector3::new(r * theta.cos(), r * theta.sin(), z)
                }
            }
        };

        // C++ parity: ParticleSys.cpp lines 1644-1646
        // C++: newPos *= (0.5f + m_particleScale/2.0f)
        let pos_scale = 0.5 + self.particle_scale / 2.0;

        // Transform to world space
        self.position + self.transform * (local_pos * pos_scale)
    }

    fn effective_emission_volume(&self) -> EmissionVolume {
        self.emission_volume_override
            .unwrap_or(self.template.info().emission_volume)
    }

    /// Compute particle velocity based on emission properties (matches C++ ParticleSystem::computeParticleVelocity)
    fn compute_particle_velocity(&self, position: &Point3<f32>) -> Vector3<f32> {
        let info = self.template.info();
        let mut rng = thread_rng();

        match info.emission_velocity {
            EmissionVelocity::Ortho { x, y, z } => Vector3::new(x.sample(), y.sample(), z.sample()),

            EmissionVelocity::Spherical { speed } => {
                let speed_val = speed.sample();
                let theta = rng.gen::<f32>() * 2.0 * std::f32::consts::PI;
                let phi = rng.gen::<f32>() * std::f32::consts::PI;
                Vector3::new(
                    speed_val * phi.sin() * theta.cos(),
                    speed_val * phi.sin() * theta.sin(),
                    speed_val * phi.cos(),
                )
            }

            EmissionVelocity::Hemispherical { speed } => {
                let speed_val = speed.sample();
                let theta = rng.gen::<f32>() * 2.0 * std::f32::consts::PI;
                let phi = rng.gen::<f32>() * std::f32::consts::PI * 0.5; // Only upper hemisphere
                Vector3::new(
                    speed_val * phi.sin() * theta.cos(),
                    speed_val * phi.sin() * theta.sin(),
                    speed_val * phi.cos(),
                )
            }

            EmissionVelocity::Cylindrical { radial, normal } => {
                let radial_speed = radial.sample();
                let normal_speed = normal.sample();
                let theta = rng.gen::<f32>() * 2.0 * std::f32::consts::PI;
                Vector3::new(
                    radial_speed * theta.cos(),
                    radial_speed * theta.sin(),
                    normal_speed,
                )
            }

            EmissionVelocity::Outward { speed, other_speed } => {
                let speed_val = speed.sample();
                let other_speed_val = other_speed.sample();

                match info.emission_volume_type {
                    EmissionVolumeType::Cylinder => {
                        let dx = position.x - self.position.x;
                        let dy = position.y - self.position.y;
                        let len = (dx * dx + dy * dy).sqrt();
                        if len > 0.0 {
                            Vector3::new(
                                speed_val * dx / len,
                                speed_val * dy / len,
                                other_speed_val,
                            )
                        } else {
                            Vector3::new(speed_val, 0.0, other_speed_val)
                        }
                    }
                    EmissionVolumeType::Box | EmissionVolumeType::Sphere => {
                        let dx = position.x - self.position.x;
                        let dy = position.y - self.position.y;
                        let dz = position.z - self.position.z;
                        let len = (dx * dx + dy * dy + dz * dz).sqrt();
                        if len > 0.0 {
                            Vector3::new(
                                speed_val * dx / len,
                                speed_val * dy / len,
                                speed_val * dz / len,
                            )
                        } else {
                            let theta = rng.gen::<f32>() * 2.0 * std::f32::consts::PI;
                            let phi = rng.gen::<f32>() * std::f32::consts::PI;
                            Vector3::new(
                                speed_val * phi.sin() * theta.cos(),
                                speed_val * phi.sin() * theta.sin(),
                                speed_val * phi.cos(),
                            )
                        }
                    }
                    EmissionVolumeType::Line => {
                        let vol = self.effective_emission_volume();
                        if let EmissionVolume::Line { start, end } = vol {
                            let along = (end - start).normalize();
                            let up = Vector3::new(0.0, 0.0, 1.0);
                            let perp = up.cross(&along).normalize();
                            let new_up = along.cross(&perp);
                            speed_val * perp + other_speed_val * new_up
                        } else {
                            Vector3::new(0.0, 0.0, other_speed_val)
                        }
                    }
                    EmissionVolumeType::Point => {
                        let theta = rng.gen::<f32>() * 2.0 * std::f32::consts::PI;
                        let phi = rng.gen::<f32>() * std::f32::consts::PI;
                        Vector3::new(
                            speed_val * phi.sin() * theta.cos(),
                            speed_val * phi.sin() * theta.sin(),
                            speed_val * phi.cos(),
                        )
                    }
                }
            }
        }
    }

    /// Update transform matrix
    fn update_transform(&mut self) {
        self.transform = self.local_transform;
        self.is_identity = self.is_local_identity;
    }

    // C++ parity: ParticleSys.cpp lines 1910-1946
    fn update_transform_from_parent(&mut self) {
        if let Some(ref parent_xfrm) = self.parent_transform {
            if self.skip_parent_transform {
                self.transform = self.local_transform;
            } else if !self.is_local_identity {
                self.transform = parent_xfrm * self.local_transform;
            } else {
                self.transform = *parent_xfrm;
            }
            self.is_identity = false;
        } else {
            if !self.is_local_identity {
                self.transform = self.local_transform;
                self.is_identity = false;
            } else {
                self.transform = Matrix3::identity();
                self.is_identity = true;
            }
        }
    }

    /// Initialize wind motion
    fn initialize_wind_motion(&mut self) {
        let info = self.template.info();
        let mut rng = thread_rng();

        match info.wind_motion {
            WindMotion::PingPong => {
                self.wind_angle = rng
                    .gen_range(info.wind_motion_start_angle_min..=info.wind_motion_start_angle_max);
                self.wind_angle_change =
                    rng.gen_range(info.wind_angle_change_min..=info.wind_angle_change_max);
                self.wind_motion_moving_to_end_angle = false;
            }
            WindMotion::Circular => {
                self.wind_angle_change =
                    rng.gen_range(info.wind_angle_change_min..=info.wind_angle_change_max);
            }
            WindMotion::NotUsed => {
                // No wind motion
            }
        }
    }

    /// Update wind motion (matches C++ ParticleSys.cpp lines 2085-2180)
    fn update_wind_motion(&mut self) {
        let info = self.template.info();
        let mut rng = thread_rng();

        match info.wind_motion {
            WindMotion::PingPong => {
                let start_angle = self.wind_motion_start_angle;
                let end_angle = self.wind_motion_end_angle;

                let total_span = end_angle - start_angle;
                let half_span = total_span / 2.0;
                let diff_from_center = (half_span - self.wind_angle + start_angle).abs();

                const MINIMUM_CHANGE: f32 = 0.005;
                let mut change = (1.0 - (diff_from_center / half_span)) * self.wind_angle_change;
                if change < MINIMUM_CHANGE {
                    change = MINIMUM_CHANGE;
                }

                if self.wind_motion_moving_to_end_angle {
                    self.wind_angle += change;
                    if self.wind_angle >= end_angle {
                        self.wind_motion_moving_to_end_angle = false;
                        self.wind_angle_change =
                            rng.gen_range(info.wind_angle_change_min..=info.wind_angle_change_max);
                        self.wind_motion_start_angle = rng.gen_range(
                            info.wind_motion_start_angle_min..=info.wind_motion_start_angle_max,
                        );
                        self.wind_motion_end_angle = rng.gen_range(
                            info.wind_motion_end_angle_min..=info.wind_motion_end_angle_max,
                        );
                    }
                } else {
                    self.wind_angle -= change;
                    if self.wind_angle <= start_angle {
                        self.wind_motion_moving_to_end_angle = true;
                        self.wind_angle_change =
                            rng.gen_range(info.wind_angle_change_min..=info.wind_angle_change_max);
                        self.wind_motion_start_angle = rng.gen_range(
                            info.wind_motion_start_angle_min..=info.wind_motion_start_angle_max,
                        );
                        self.wind_motion_end_angle = rng.gen_range(
                            info.wind_motion_end_angle_min..=info.wind_motion_end_angle_max,
                        );
                    }
                }
            }
            WindMotion::Circular => {
                self.wind_angle += self.wind_angle_change;
                // Keep angle in 0-2π range
                while self.wind_angle > 2.0 * std::f32::consts::PI {
                    self.wind_angle -= 2.0 * std::f32::consts::PI;
                }
            }
            WindMotion::NotUsed => {
                // No wind motion
            }
        }
    }

    /// Get system ID
    pub fn system_id(&self) -> ParticleSystemId {
        self.system_id
    }

    /// Get template
    pub fn template(&self) -> &Arc<ParticleSystemTemplate> {
        &self.template
    }

    /// Tint all color keys on this system's private template copy.
    pub fn tint_all_colors(&mut self, tint_color: [f32; 3]) {
        Arc::make_mut(&mut self.template)
            .info_mut()
            .tint_all_colors(tint_color);
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
        assert_eq!(particle.alpha, 1.0);
    }

    #[test]
    fn particle_system_tint_all_colors_keeps_template_source_unchanged() {
        let mut template = ParticleSystemTemplate::new("TintSource".to_string());
        template.info_mut().color_keys[0] = RGBColorKeyframe {
            color: [1.0, 0.5, 0.25],
            frame: 0,
        };
        let template = Arc::new(template);
        let mut system = ParticleSystem::new(template.clone(), 1, false);

        system.tint_all_colors([0.25, 0.5, 1.0]);

        assert_eq!(
            system.template().info().color_keys[0].color,
            [0.25, 0.25, 0.25]
        );
        assert_eq!(template.info().color_keys[0].color, [1.0, 0.5, 0.25]);
    }

    #[test]
    fn test_particle_xfer_load_applies_particle_info_base() {
        use game_engine::common::system::xfer_load::XferLoad;
        use game_engine::common::system::xfer_save::XferSave;
        use std::io::Cursor;

        let mut info = ParticleInfo::default();
        info.velocity = Vector3::new(1.0, 2.0, 3.0);
        info.position = Point3::new(4.0, 5.0, 6.0);
        info.emitter_position = Point3::new(7.0, 8.0, 9.0);
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
        saved.acceleration = Vector3::new(16.0, 17.0, 18.0);
        saved.last_position = Point3::new(19.0, 20.0, 21.0);
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

        assert_eq!(loaded.velocity, Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(loaded.position, Point3::new(4.0, 5.0, 6.0));
        assert_eq!(loaded.emitter_position, Point3::new(7.0, 8.0, 9.0));
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
        assert_eq!(loaded.acceleration, Vector3::new(16.0, 17.0, 18.0));
        assert_eq!(loaded.last_position, Point3::new(19.0, 20.0, 21.0));
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
        let system = ParticleSystem::new(template, 1, false);

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
            info.drift_velocity = Vector3::new(15.0, 16.0, 17.0);
            info.gravity = -0.25;
            info.slave_system_name = "SlaveSystem".to_string();
            info.slave_pos_offset = Vector3::new(18.0, 19.0, 20.0);
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
        saved.local_transform = Matrix3::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        saved.transform = Matrix3::new(9.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0);
        saved.is_local_identity = false;
        saved.is_identity = false;
        saved.wind_angle = 0.33;
        saved.wind_angle_change = 0.44;
        saved.wind_motion_moving_to_end_angle = true;

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

        let loaded_info = loaded.template.info();
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
        assert_eq!(loaded_info.drift_velocity, Vector3::new(15.0, 16.0, 17.0));
        assert_eq!(loaded_info.gravity, -0.25);
        assert_eq!(loaded_info.slave_system_name, "SlaveSystem");
        assert_eq!(loaded_info.slave_pos_offset, Vector3::new(18.0, 19.0, 20.0));
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
        assert_eq!(loaded.wind_angle, 0.33);
        assert_eq!(loaded.wind_angle_change, 0.44);
        assert!(loaded.wind_motion_moving_to_end_angle);
        assert_eq!(loaded.system_id, 42);
        assert_eq!(loaded.local_transform, saved.local_transform);
        assert_eq!(loaded.transform, saved.transform);
    }

    #[test]
    fn test_emission_volumes() {
        use rand::prelude::*;
        let mut rng = thread_rng();

        // Test sphere emission
        let sphere = EmissionVolume::Sphere { radius: 5.0 };

        // This would require the actual computation logic
        // which is implemented in compute_particle_position
        let _ = (rng.gen::<f32>(), sphere);
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
    fn test_saveable_cascade_flag() {
        let template = Arc::new(ParticleSystemTemplate::new("Test".to_string()));
        let mut system = ParticleSystem::new(template, 1, false);

        assert!(system.is_saveable());
        system.set_saveable(false);
        assert!(!system.is_saveable());
    }
}
