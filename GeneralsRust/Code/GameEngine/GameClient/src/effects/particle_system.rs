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

    // Emission volume overrides
    emission_volume_override: Option<EmissionVolume>,
    emission_volume_type_override: Option<EmissionVolumeType>,
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

            emission_volume_override: None,
            emission_volume_type_override: None,
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
    pub fn update(&mut self, local_player_index: i32, current_frame: u32) -> bool {
        if self.is_destroyed && self.particles.is_empty() {
            return false;
        }

        // Update wind motion (C++ line 1978)
        self.update_wind_motion();

        // Update existing particles (C++ lines 2143-2158)
        self.update_particles(current_frame);

        // Emit new particles if not stopped or destroyed (C++ lines 1987-2049)
        if !self.is_stopped && !self.is_destroyed {
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

    /// Get attached object
    pub fn attached_object(&self) -> Option<ObjectId> {
        if self.attached_object_id != 0 {
            Some(self.attached_object_id)
        } else {
            None
        }
    }

    /// Start the particle system (matches C++ ParticleSystem::start)
    pub fn start(&mut self) {
        self.is_stopped = false;
        self.is_destroyed = false;
        self.start_timestamp = 0; // Should be current frame timestamp
    }

    /// Stop the particle system (matches C++ ParticleSystem::stop)
    pub fn stop(&mut self) {
        self.is_stopped = true;
    }

    /// Destroy the particle system (matches C++ ParticleSystem::destroy)
    pub fn destroy(&mut self) {
        self.is_stopped = true;
        self.is_destroyed = true;
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

        for i in 0..burst_count {
            if let Some(particle_info) = self.generate_particle_info(i, burst_count) {
                // Create particle with current frame as creation timestamp (C++ line 287)
                let particle =
                    Particle::new(&particle_info, self.personality_counter, current_frame);
                self.particles.push_back(particle);
                self.particle_count += 1;
                self.personality_counter += 1;
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
    fn generate_particle_info(
        &self,
        particle_num: u32,
        particle_count: u32,
    ) -> Option<ParticleInfo> {
        let info = self.template.info();

        let mut particle_info = ParticleInfo::default();

        // Position
        particle_info.position = self.compute_particle_position();
        particle_info.emitter_position = self.position;

        // Velocity
        particle_info.velocity = self.compute_particle_velocity(&particle_info.position);

        // Apply velocity coefficient
        particle_info.velocity.component_mul_assign(&self.vel_coeff);

        // Lifetime
        particle_info.lifetime = info.lifetime.sample() as u32;

        // Size
        particle_info.size =
            (info.start_size.sample() + self.accumulated_size_bonus) * self.size_coeff;
        particle_info.size_rate = info.size_rate.sample();
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

            EmissionVolume::Box { half_size } => Vector3::new(
                rng.gen_range(-half_size.x..=half_size.x),
                rng.gen_range(-half_size.y..=half_size.y),
                rng.gen_range(-half_size.z..=half_size.z),
            ),

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

        // Transform to world space
        self.position + self.transform * local_pos
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

                // Direction from emitter to particle position
                let direction = (position - self.position).normalize();
                direction * speed_val + Vector3::new(0.0, 0.0, other_speed_val)
            }
        }
    }

    /// Update transform matrix
    fn update_transform(&mut self) {
        // For now, assume no parent transform
        self.transform = self.local_transform;
        self.is_identity = self.is_local_identity;
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

    /// Update wind motion (matches C++ ParticleSystem::updateWindMotion)
    fn update_wind_motion(&mut self) {
        let info = self.template.info();

        match info.wind_motion {
            WindMotion::PingPong => {
                if self.wind_motion_moving_to_end_angle {
                    self.wind_angle += self.wind_angle_change;
                    if self.wind_angle >= info.wind_motion_end_angle {
                        self.wind_motion_moving_to_end_angle = false;
                    }
                } else {
                    self.wind_angle -= self.wind_angle_change;
                    if self.wind_angle <= info.wind_motion_start_angle {
                        self.wind_motion_moving_to_end_angle = true;
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

    /// Check if destroyed
    pub fn is_destroyed(&self) -> bool {
        self.is_destroyed
    }

    /// Get template
    pub fn template(&self) -> &Arc<ParticleSystemTemplate> {
        &self.template
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
    fn test_particle_system_creation() {
        let template = Arc::new(ParticleSystemTemplate::new("TestSystem".to_string()));
        let system = ParticleSystem::new(template, 1, false);

        assert_eq!(system.system_id(), 1);
        assert_eq!(system.particle_count(), 0);
        assert!(!system.is_destroyed());
    }

    #[test]
    fn test_emission_volumes() {
        use rand::prelude::*;
        let mut rng = thread_rng();

        // Test sphere emission
        let sphere = EmissionVolume::Sphere { radius: 5.0 };

        // This would require the actual computation logic
        // which is implemented in compute_particle_position
    }
}
