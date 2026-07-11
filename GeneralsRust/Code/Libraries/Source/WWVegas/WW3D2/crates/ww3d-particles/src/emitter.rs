//! Particle Emitter Implementation
//!
//! This module implements the ParticleEmitterClass which creates and manages
//! particle effects, emitting particles into a ParticleBuffer.

use super::buffer::{FrameMode, NewParticle, ParticleBuffer, RenderMode};
use super::properties::*;
// use super::point_group::PointGroup;
// use super::streak::*;
// use super::line_renderer::*;
use glam::{Mat4, Quat, Vec3, Vec4Swizzles};
use ww3d_core::ww3d::WW3D;

/// Vec3 randomizer for position and velocity randomization
#[derive(Debug, Clone)]
pub struct Vec3Randomizer {
    pub min: Vec3,
    pub max: Vec3,
}

impl Vec3Randomizer {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn get_maximum_extent(&self) -> f32 {
        (self.max - self.min).length()
    }

    pub fn randomize(&self) -> Vec3 {
        Vec3::new(
            rand::random::<f32>() * (self.max.x - self.min.x) + self.min.x,
            rand::random::<f32>() * (self.max.y - self.min.y) + self.min.y,
            rand::random::<f32>() * (self.max.z - self.min.z) + self.min.z,
        )
    }
}

fn gcd_u32(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let tmp = b;
        b = a % b;
        a = tmp;
    }
    a
}

/// Particle emitter class - creates and manages particle effects
#[derive(Debug)]
pub struct ParticleEmitter {
    // Emission parameters
    pub emit_rate_ms: u32, // Milliseconds between emissions
    pub burst_size: usize,
    pub one_time_burst_size: usize,
    pub one_time_burst: bool,

    // Position and velocity
    pub position_randomizer: Option<Vec3Randomizer>,
    pub velocity_randomizer: Option<Vec3Randomizer>,
    pub base_velocity: Vec3,
    pub outward_velocity: f32,
    pub velocity_inherit_factor: f32,

    // Timing
    pub emit_remainder: u32,
    pub prev_quaternion: Quat,
    pub prev_origin: Vec3,
    pub active: bool,
    pub first_time: bool,
    pub particles_left: i32,
    pub max_particles: usize,

    // State
    pub is_complete: bool,
    pub name: String,
    pub remove_on_complete: bool,
    pub is_in_scene: bool,
    pub current_group_id: u8,

    // Buffer reference
    pub buffer: Option<ParticleBuffer>,

    // Timing
    pub current_time_ms: u32,

    // Transform
    pub current_transform: Mat4,
}

impl ParticleEmitter {
    /// Create a new particle emitter
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        emit_rate: f32, // particles per second
        burst_size: usize,
        pos_randomizer: Option<Vec3Randomizer>,
        base_vel: Vec3,
        vel_randomizer: Option<Vec3Randomizer>,
        out_vel: f32,
        vel_inherit_factor: f32,
        color_prop: ParticleColorProperty,
        opacity_prop: ParticleOpacityProperty,
        size_prop: ParticleSizeProperty,
        mut rotation_prop: ParticleRotationProperty,
        _orient_random: f32,
        frame_prop: ParticleFrameProperty,
        blur_time_prop: ParticleBlurTimeProperty,
        acceleration: Vec3,
        max_age: f32,      // in seconds
        future_start: f32, // in seconds
        render_mode: RenderMode,
        frame_mode: FrameMode,
        max_particles: usize,
        max_buffer_size: usize,
        _pingpong: bool,
    ) -> Self {
        let emit_rate_ms = if emit_rate > 0.0 {
            (1000.0 / emit_rate) as u32
        } else {
            1000
        };

        rotation_prop.set_orient_random(_orient_random);

        let buffer = Some(ParticleBuffer::new(
            max_buffer_size,
            color_prop,
            opacity_prop,
            size_prop,
            rotation_prop,
            frame_prop,
            blur_time_prop,
            acceleration,
            (max_age * 1000.0) as u32,
            (future_start * 1000.0) as u32,
            render_mode,
            frame_mode,
            None, // point_group will be created later
            None, // line_renderer will be created later
            None, // line_group_renderer will be created later
        ));

        Self {
            emit_rate_ms,
            burst_size: burst_size.max(1),
            one_time_burst_size: 0,
            one_time_burst: false,
            position_randomizer: pos_randomizer,
            velocity_randomizer: vel_randomizer,
            base_velocity: base_vel,
            outward_velocity: out_vel,
            velocity_inherit_factor: vel_inherit_factor,
            emit_remainder: 0,
            prev_quaternion: Quat::IDENTITY,
            prev_origin: Vec3::ZERO,
            active: true,
            first_time: true,
            particles_left: if max_particles > 0 {
                max_particles as i32
            } else {
                -1
            },
            max_particles,
            is_complete: false,
            name: String::new(),
            remove_on_complete: false,
            is_in_scene: false,
            current_group_id: 0,
            buffer,
            current_time_ms: 0,
            current_transform: Mat4::IDENTITY,
        }
    }

    /// Reset the emitter
    pub fn reset(&mut self) {
        self.emit_remainder = 0;
        self.prev_quaternion = Quat::IDENTITY;
        self.prev_origin = Vec3::ZERO;
        self.active = true;
        self.first_time = true;
        self.particles_left = if self.max_particles > 0 {
            self.max_particles as i32
        } else {
            -1
        };
        self.is_complete = false;
        self.current_group_id = 0;
        self.current_time_ms = 0;
        self.current_transform = Mat4::IDENTITY;
    }

    /// Start emitting particles
    pub fn start(&mut self) {
        self.active = true;
        self.current_group_id = self.current_group_id.wrapping_add(1);
        self.first_time = true;
    }

    /// Stop emitting particles
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Check if emitter is stopped
    pub fn is_stopped(&self) -> bool {
        !self.active
    }

    /// Set the current transform for the emitter
    pub fn set_transform(&mut self, transform: Mat4) {
        self.current_transform = transform;
    }

    /// Emit particles for this frame using the accumulated transform
    pub fn update(&mut self, delta_time_ms: u32) {
        if !self.active || self.is_complete {
            return;
        }

        let mut frame_time = delta_time_ms;
        let previous_time = self.current_time_ms;
        let sync_time = WW3D::sync_time();

        if sync_time != 0 {
            self.current_time_ms = sync_time;
            if previous_time != 0 {
                frame_time = sync_time.saturating_sub(previous_time);
            }
        } else {
            self.current_time_ms = self.current_time_ms.saturating_add(delta_time_ms);
        }
        let current_transform = self.current_transform;
        let current_quat = Quat::from_mat4(&current_transform);
        let current_origin = current_transform.w_axis.xyz();

        if self.first_time {
            self.prev_quaternion = current_quat;
            self.prev_origin = current_origin;
            self.first_time = false;
        }

        let prev_quat = self.prev_quaternion;
        let prev_origin = self.prev_origin;

        let emit_rate_ms = self.emit_rate_ms.max(1);

        // Prevent excessive loop iterations for very large frame times similar to the C++ behaviour.
        if frame_time > emit_rate_ms.saturating_mul(100) {
            if let Some(ref buffer) = self.buffer {
                let buffer_size = buffer.get_max_particles().max(1);
                let gcd = gcd_u32(buffer_size as u32, self.burst_size as u32).max(1);
                let bursts = (buffer_size as u32) / gcd;
                let cycle_time = emit_rate_ms.saturating_mul(bursts);
                frame_time = if cycle_time > 1 {
                    frame_time % cycle_time
                } else {
                    1
                };
            }
        }

        self.emit_remainder = self.emit_remainder.saturating_add(frame_time);

        let frame_time_f = if frame_time == 0 {
            1.0
        } else {
            frame_time as f32
        };

        let mut alpha = 1.0 - (self.emit_remainder as f32 / frame_time_f);
        let d_alpha = emit_rate_ms as f32 / frame_time_f;

        let emitter_inherited_velocity = if self.velocity_inherit_factor != 0.0 && frame_time > 0 {
            (current_origin - prev_origin) * (self.velocity_inherit_factor / frame_time_f)
        } else {
            Vec3::ZERO
        };

        // Handle one-time burst
        if self.one_time_burst && self.emit_remainder < emit_rate_ms {
            self.emit_remainder = emit_rate_ms;
        }

        let mut pending_one_time_burst = self.one_time_burst;
        while self.emit_remainder >= emit_rate_ms {
            self.emit_remainder -= emit_rate_ms;
            alpha += d_alpha;

            let lerp_factor = alpha.clamp(0.0, 1.0);
            let interp_quat = prev_quat.slerp(current_quat, lerp_factor);
            let interp_origin = prev_origin.lerp(current_origin, lerp_factor);
            let spawn_time = self.current_time_ms.saturating_sub(self.emit_remainder);

            let burst_size = if pending_one_time_burst {
                pending_one_time_burst = false;
                self.one_time_burst = false;
                self.one_time_burst_size.max(1)
            } else {
                self.burst_size
            };

            if self.emit_particles(
                burst_size,
                spawn_time,
                interp_quat,
                interp_origin,
                emitter_inherited_velocity,
            ) {
                break;
            }
        }

        self.prev_quaternion = current_quat;
        self.prev_origin = current_origin;
    }

    fn emit_particles(
        &mut self,
        desired_count: usize,
        spawn_time: u32,
        interp_quat: Quat,
        interp_origin: Vec3,
        emitter_velocity: Vec3,
    ) -> bool {
        let Some(ref mut buffer) = self.buffer else {
            return false;
        };

        if desired_count == 0 {
            return false;
        }

        let actual_count = if self.particles_left > 0 {
            desired_count.min(self.particles_left as usize)
        } else {
            desired_count
        };

        if actual_count == 0 {
            self.is_complete = true;
            self.active = false;
            return true;
        }

        for _ in 0..actual_count {
            let new_particle = Self::create_new_particle_static(
                interp_quat,
                interp_origin,
                self.base_velocity,
                self.outward_velocity,
                emitter_velocity,
                self.current_group_id,
                self.position_randomizer.as_ref(),
                self.velocity_randomizer.as_ref(),
                spawn_time,
            );
            buffer.add_new_particle(new_particle);
        }

        if self.particles_left > 0 {
            self.particles_left -= actual_count as i32;
            if self.particles_left <= 0 {
                self.is_complete = true;
                self.active = false;
                if let Some(ref mut buffer) = self.buffer {
                    buffer.emitter_is_dead();
                }
                return true;
            }
        }

        false
    }

    /// Create a single new particle (static method to avoid borrowing issues)
    fn create_new_particle_static(
        interp_quat: Quat,
        interp_origin: Vec3,
        base_velocity: Vec3,
        outward_velocity: f32,
        emitter_velocity: Vec3,
        current_group_id: u8,
        position_randomizer: Option<&Vec3Randomizer>,
        velocity_randomizer: Option<&Vec3Randomizer>,
        spawn_timestamp: u32,
    ) -> NewParticle {
        // Generate position with randomization
        let mut local_position = Vec3::ZERO;
        if let Some(pos_rand) = position_randomizer {
            local_position = pos_rand.randomize();
        }

        // Transform position to world space
        let position = interp_quat.mul_vec3(local_position) + interp_origin;

        // Generate velocity
        let mut velocity = base_velocity;

        // Add outward velocity if specified
        if outward_velocity != 0.0 {
            let outward_dir = if local_position.length_squared() > 0.0 {
                local_position.normalize()
            } else {
                Vec3::X
            };
            velocity += outward_dir * outward_velocity;
        }

        // Add velocity randomization
        if let Some(vel_rand) = velocity_randomizer {
            velocity += vel_rand.randomize();
        }

        // Transform velocity to world space
        let mut velocity = interp_quat.mul_vec3(velocity);

        // Add velocity inheritance from emitter movement
        velocity += emitter_velocity;

        NewParticle {
            position,
            velocity,
            timestamp: spawn_timestamp,
            group_id: current_group_id,
        }
    }

    /// Set position randomizer
    pub fn set_position_randomizer(&mut self, randomizer: Option<Vec3Randomizer>) {
        self.position_randomizer = randomizer;
    }

    /// Set velocity randomizer
    pub fn set_velocity_randomizer(&mut self, randomizer: Option<Vec3Randomizer>) {
        self.velocity_randomizer = randomizer;
    }

    /// Set base velocity
    pub fn set_base_velocity(&mut self, velocity: Vec3) {
        self.base_velocity = velocity;
    }

    /// Set outward velocity
    pub fn set_outwards_velocity(&mut self, velocity: f32) {
        self.outward_velocity = velocity;
    }

    /// Set velocity inheritance factor
    pub fn set_velocity_inheritance_factor(&mut self, factor: f32) {
        self.velocity_inherit_factor = factor;
    }

    /// Set emission rate in particles per second
    pub fn set_emission_rate(&mut self, rate: f32) {
        self.emit_rate_ms = if rate > 0.0 {
            (1000.0 / rate) as u32
        } else {
            1000
        };
    }

    /// Set burst size
    pub fn set_burst_size(&mut self, size: usize) {
        self.burst_size = size.max(1);
    }

    /// Set one-time burst
    pub fn set_one_time_burst(&mut self, size: usize) {
        self.one_time_burst_size = size.max(1);
        self.one_time_burst = true;
    }

    /// Get emission rate in particles per second
    pub fn get_emission_rate(&self) -> f32 {
        1000.0 / self.emit_rate_ms as f32
    }

    /// Get burst size
    pub fn get_burst_size(&self) -> usize {
        self.burst_size
    }

    /// Get max particles
    pub fn get_max_particles(&self) -> usize {
        self.max_particles
    }

    /// Get start velocity
    pub fn get_start_velocity(&self) -> Vec3 {
        self.base_velocity * 1000.0 // Convert to units per second
    }

    /// Get outward velocity
    pub fn get_outwards_velocity(&self) -> f32 {
        self.outward_velocity * 1000.0 // Convert to units per second
    }

    /// Get velocity inheritance factor
    pub fn get_velocity_inheritance_factor(&self) -> f32 {
        self.velocity_inherit_factor
    }

    /// Check if emitter is complete
    pub fn is_complete(&self) -> bool {
        self.is_complete
    }

    /// Enable/disable auto removal on complete
    pub fn enable_remove_on_complete(&mut self, enable: bool) {
        self.remove_on_complete = enable;
    }

    /// Check if remove on complete is enabled
    pub fn is_remove_on_complete_enabled(&self) -> bool {
        self.remove_on_complete
    }

    /// Get buffer reference
    pub fn get_buffer(&self) -> Option<&ParticleBuffer> {
        self.buffer.as_ref()
    }

    /// Get mutable buffer reference
    pub fn get_buffer_mut(&mut self) -> Option<&mut ParticleBuffer> {
        self.buffer.as_mut()
    }

    /// Buffer scene needed flag
    pub fn buffer_scene_needed(&mut self, needed: bool) {
        // This would be used in the scene management system
        self.first_time = !needed;
    }

    /// Remove buffer from scene
    pub fn remove_buffer_from_scene(&mut self) {
        self.first_time = true;
        // Scene management would handle the actual removal
    }
}

/// Default remove on complete setting - thread-safe using AtomicBool
///
/// Matches C++ part_emt.cpp line 68: Initialized to true (for editing purposes)
///
/// SAFETY NOTE: C++ implementation uses plain `static bool` without synchronization.
/// Rust implementation uses AtomicBool for thread safety, which is a safety improvement
/// over the C++ original. This allows safe concurrent access if the game engine
/// spawns particle emitters from multiple threads. The atomic load/store with
/// Relaxed ordering has minimal performance overhead while eliminating race conditions.
use std::sync::atomic::{AtomicBool, Ordering};
static DEFAULT_REMOVE_ON_COMPLETE: AtomicBool = AtomicBool::new(true);

impl ParticleEmitter {
    /// Get default remove on complete setting
    pub fn default_remove_on_complete() -> bool {
        DEFAULT_REMOVE_ON_COMPLETE.load(Ordering::Relaxed)
    }

    /// Set default remove on complete setting
    pub fn set_default_remove_on_complete(onoff: bool) {
        DEFAULT_REMOVE_ON_COMPLETE.store(onoff, Ordering::Relaxed);
    }
}
