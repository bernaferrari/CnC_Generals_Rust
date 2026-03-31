//! Particle Emitter Class - Core particle emission and management
//!
//! This module implements the ParticleEmitterClass from the original C++ code,
//! providing comprehensive particle effects with WGPU integration.

use glam::{Mat4, Vec3, Vec4};
use std::sync::Arc;
use ww3d_core::wwstring::StringClass;
use ww3d_renderer_3d::core::error::RendererResult;
use ww3d_renderer_3d::math_utilities::Matrix4;
use ww3d_renderer_3d::render_object_system::{
    AABoxClass, AABoxCollisionTestClass, AABoxIntersectionTestClass, DecalGeneratorClass,
    MaterialInfoClass, OBBoxCollisionTestClass, OBBoxIntersectionTestClass, RayCollisionTestClass,
    RenderInfoClass, RenderObjClass, RenderObjClassId, SpecialRenderInfoClass, SphereClass,
};
use ww3d_renderer_3d::rendering::shader_system::shader::ShaderClass;
use ww3d_renderer_3d::rendering::texture_system::TextureClass;

/// Convert glam::Mat4 to math_utilities::Matrix4
#[allow(dead_code)] // C++ parity
fn mat4_to_matrix4(m: Mat4) -> Matrix4 {
    let cols = m.to_cols_array();
    Matrix4::from_cols_array(&cols)
}

// Matrix conversion functions removed - using Mat4 directly now

/// Convert glam::Vec3 to math_utilities::Vec3
#[allow(dead_code)] // C++ parity
fn vec3_to_vector3(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

/// Convert math_utilities::Vec3 to glam::Vec3
fn vector3_to_vec3(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

/// Particle property structure with keyframes
#[derive(Debug, Clone)]
pub struct ParticleProperty<T> {
    /// Starting value
    pub start: T,
    /// Random variation
    pub rand: T,
    /// Keyframe times
    pub key_times: Vec<f32>,
    /// Keyframe values
    pub values: Vec<T>,
}

impl<T> ParticleProperty<T>
where
    T: Clone + Default,
{
    /// Create new particle property
    pub fn new() -> Self {
        Self {
            start: T::default(),
            rand: T::default(),
            key_times: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Evaluate property at time
    pub fn evaluate(&self, time: f32, random_factor: f32) -> T
    where
        T: std::ops::Add<Output = T> + std::ops::Mul<f32, Output = T> + Copy,
    {
        if self.key_times.is_empty() {
            return self.start + self.rand * random_factor;
        }

        // Find keyframes
        let mut idx = 0;
        for (i, &key_time) in self.key_times.iter().enumerate() {
            if time <= key_time {
                idx = i;
                break;
            }
            idx = i;
        }

        if idx >= self.values.len() {
            return self.values.last().cloned().unwrap_or(self.start);
        }

        let value = self.values[idx];
        value + self.rand * random_factor
    }

    /// Set keyframes
    pub fn set_keyframes(&mut self, times: Vec<f32>, values: Vec<T>) {
        self.key_times = times;
        self.values = values;
    }
}

/// Particle emitter render mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmitterRenderMode {
    TriParticles = 0,
    QuadParticles,
    Line,
    Tetrahedral,
}

/// Particle emitter frame mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmitterFrameMode {
    Mode1x1 = 0,
    Mode2x2,
    Mode4x4,
    Mode8x8,
    Mode16x16,
}

/// Particle data structure
#[derive(Debug, Clone)]
pub struct Particle {
    /// Position
    pub position: Vec3,
    /// Velocity
    pub velocity: Vec3,
    /// Age in seconds
    pub age: f32,
    /// Maximum age
    pub max_age: f32,
    /// Color
    pub color: Vec4,
    /// Size
    pub size: f32,
    /// Rotation
    pub rotation: f32,
    /// Whether particle is active
    pub active: bool,
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            age: 0.0,
            max_age: 1.0,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            size: 1.0,
            rotation: 0.0,
            active: false,
        }
    }
}

/// Particle Emitter Class - Core particle emission system
pub struct ParticleEmitterClass {
    /// Emitter name
    name: StringClass,
    /// Transform matrix
    transform: Mat4,
    /// Whether transform is identity
    transform_identity: bool,

    /// Emission rate (particles per second)
    emit_rate: f32,
    /// Burst size
    burst_size: u32,
    /// Time accumulator for emission
    emit_accumulator: f32,

    /// Base velocity
    base_velocity: Vec3,
    /// Velocity inheritance factor
    velocity_inheritance_factor: f32,
    /// Acceleration
    acceleration: Vec3,

    /// Position randomizer
    position_randomizer: Option<Box<dyn Fn() -> Vec3 + Send + Sync>>,
    /// Velocity randomizer
    velocity_randomizer: Option<Box<dyn Fn() -> Vec3 + Send + Sync>>,

    /// Color property
    color_property: ParticleProperty<Vec3>,
    /// Opacity property
    opacity_property: ParticleProperty<f32>,
    /// Size property
    size_property: ParticleProperty<f32>,
    /// Rotation property
    rotation_property: ParticleProperty<f32>,
    /// Frame property
    frame_property: ParticleProperty<f32>,
    /// Blur times property
    blur_times_property: ParticleProperty<f32>,

    /// Texture
    texture: Option<Arc<TextureClass>>,
    /// Shader
    shader: ShaderClass,

    /// Maximum age
    max_age: f32,
    /// Future start time
    future_start: f32,
    /// Orientation randomization
    orientation_randomization: f32,

    /// Render mode
    render_mode: EmitterRenderMode,
    /// Frame mode
    frame_mode: EmitterFrameMode,

    /// Maximum particles
    max_particles: usize,
    /// Maximum buffer size
    max_buffer_size: i32,

    /// Whether to pingpong
    pingpong: bool,
    /// Scale factor
    scale: f32,

    /// Particles
    particles: Vec<Particle>,
    /// Active particle count
    active_count: usize,

    /// Reference count
    ref_count: std::sync::atomic::AtomicU32,
}

impl std::fmt::Debug for ParticleEmitterClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParticleEmitterClass")
            .field("name", &self.name)
            .field("emit_rate", &self.emit_rate)
            .field("max_particles", &self.max_particles)
            .field("active_count", &self.active_count)
            .field(
                "position_randomizer",
                &self.position_randomizer.as_ref().map(|_| "Fn() -> Vec3"),
            )
            .field(
                "velocity_randomizer",
                &self.velocity_randomizer.as_ref().map(|_| "Fn() -> Vec3"),
            )
            .finish()
    }
}

impl ParticleEmitterClass {
    /// Create new particle emitter
    pub fn new(
        emit_rate: f32,
        burst_size: u32,
        max_particles: usize,
        texture: Option<Arc<TextureClass>>,
        shader: ShaderClass,
    ) -> Self {
        Self {
            name: StringClass::new(),
            transform: Mat4::IDENTITY,
            transform_identity: true,
            emit_rate,
            burst_size,
            emit_accumulator: 0.0,
            base_velocity: Vec3::ZERO,
            velocity_inheritance_factor: 0.0,
            acceleration: Vec3::ZERO,
            position_randomizer: None,
            velocity_randomizer: None,
            color_property: ParticleProperty::new(),
            opacity_property: ParticleProperty::new(),
            size_property: ParticleProperty::new(),
            rotation_property: ParticleProperty::new(),
            frame_property: ParticleProperty::new(),
            blur_times_property: ParticleProperty::new(),
            texture,
            shader,
            max_age: 1.0,
            future_start: 0.0,
            orientation_randomization: 0.0,
            render_mode: EmitterRenderMode::TriParticles,
            frame_mode: EmitterFrameMode::Mode1x1,
            max_particles,
            max_buffer_size: -1,
            pingpong: false,
            scale: 1.0,
            particles: vec![Particle::default(); max_particles],
            active_count: 0,
            ref_count: std::sync::atomic::AtomicU32::new(1),
        }
    }

    /// Update particles
    pub fn update(&mut self, delta_time: f32) {
        // Update existing particles
        for particle in &mut self.particles {
            if particle.active {
                particle.age += delta_time;
                particle.velocity += self.acceleration * delta_time;
                particle.position += particle.velocity * delta_time;

                if particle.age >= particle.max_age {
                    particle.active = false;
                    self.active_count = self.active_count.saturating_sub(1);
                }
            }
        }

        // Emit new particles
        self.emit_accumulator += delta_time;
        let particles_to_emit = (self.emit_rate * self.emit_accumulator) as usize;

        if particles_to_emit > 0 {
            self.emit_accumulator -= particles_to_emit as f32 / self.emit_rate;

            for _ in 0..particles_to_emit {
                if self.active_count < self.max_particles {
                    self.emit_particle();
                }
            }
        }
    }

    /// Emit a single particle
    fn emit_particle(&mut self) {
        // Find inactive particle slot
        for particle in &mut self.particles {
            if !particle.active {
                // Initialize particle
                particle.position = Vec3::ZERO; // Would use position randomizer
                particle.velocity = self.base_velocity; // Would use velocity randomizer
                particle.age = 0.0;
                particle.max_age = self.max_age;
                particle.active = true;
                self.active_count += 1;
                break;
            }
        }
    }

    /// Get active particles
    pub fn get_active_particles(&self) -> &[Particle] {
        &self.particles[..self.active_count]
    }

    /// Set emission rate
    pub fn set_emit_rate(&mut self, rate: f32) {
        self.emit_rate = rate;
    }

    /// Get emission rate
    pub fn get_emit_rate(&self) -> f32 {
        self.emit_rate
    }

    /// Set base velocity
    pub fn set_base_velocity(&mut self, velocity: Vec3) {
        self.base_velocity = velocity;
    }

    /// Get base velocity
    pub fn get_base_velocity(&self) -> Vec3 {
        self.base_velocity
    }

    /// Set acceleration
    pub fn set_acceleration(&mut self, acceleration: Vec3) {
        self.acceleration = acceleration;
    }

    /// Get acceleration
    pub fn get_acceleration(&self) -> Vec3 {
        self.acceleration
    }

    /// Set maximum age
    pub fn set_max_age(&mut self, age: f32) {
        self.max_age = age;
    }

    /// Get maximum age
    pub fn get_max_age(&self) -> f32 {
        self.max_age
    }

    /// Set texture
    pub fn set_texture(&mut self, texture: Option<Arc<TextureClass>>) {
        self.texture = texture;
    }

    /// Get texture
    pub fn get_texture(&self) -> Option<&Arc<TextureClass>> {
        self.texture.as_ref()
    }

    /// Set shader
    pub fn set_shader(&mut self, shader: ShaderClass) {
        self.shader = shader;
    }

    /// Get shader
    pub fn get_shader(&self) -> &ShaderClass {
        &self.shader
    }

    /// Scale emitter
    pub fn scale(&mut self, scale_factor: f32) {
        self.scale *= scale_factor;
        // Would scale velocities, sizes, etc.
    }

    /// Get scale
    pub fn get_scale(&self) -> f32 {
        self.scale
    }

    /// Restart emitter
    pub fn restart(&mut self) {
        for particle in &mut self.particles {
            particle.active = false;
        }
        self.active_count = 0;
        self.emit_accumulator = 0.0;
    }

    /// Get active particle count
    pub fn get_active_count(&self) -> usize {
        self.active_count
    }

    /// Get maximum particle count
    pub fn get_max_particles(&self) -> usize {
        self.max_particles
    }

    // Property setters

    /// Set color keyframes
    pub fn set_color_keyframes(&mut self, times: Vec<f32>, values: Vec<Vec3>) {
        self.color_property.set_keyframes(times, values);
    }

    /// Set opacity keyframes
    pub fn set_opacity_keyframes(&mut self, times: Vec<f32>, values: Vec<f32>) {
        self.opacity_property.set_keyframes(times, values);
    }

    /// Set size keyframes
    pub fn set_size_keyframes(&mut self, times: Vec<f32>, values: Vec<f32>) {
        self.size_property.set_keyframes(times, values);
    }

    /// Set rotation keyframes
    pub fn set_rotation_keyframes(&mut self, times: Vec<f32>, values: Vec<f32>) {
        self.rotation_property.set_keyframes(times, values);
    }

    /// Set render mode
    pub fn set_render_mode(&mut self, mode: EmitterRenderMode) {
        self.render_mode = mode;
    }

    /// Get render mode
    pub fn get_render_mode(&self) -> EmitterRenderMode {
        self.render_mode
    }

    /// Set frame mode
    pub fn set_frame_mode(&mut self, mode: EmitterFrameMode) {
        self.frame_mode = mode;
    }

    /// Get frame mode
    pub fn get_frame_mode(&self) -> EmitterFrameMode {
        self.frame_mode
    }

    // Bounding volume calculations

    /// Calculate bounding box
    pub fn calculate_bounding_box(&self) -> AABoxClass {
        if self.active_count == 0 {
            return AABoxClass::from_center_and_extent(Vec3::ZERO, Vec3::ZERO);
        }

        let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);

        for particle in &self.particles[..self.active_count] {
            if particle.active {
                min = min.min(particle.position);
                max = max.max(particle.position);
            }
        }

        let center = (min + max) * 0.5;
        let extent = (max - min) * 0.5;
        AABoxClass::from_center_and_extent(center, extent)
    }

    /// Calculate bounding sphere
    pub fn calculate_bounding_sphere(&self) -> SphereClass {
        let bbox = self.calculate_bounding_box();
        let center = (bbox.min() + bbox.max()) * 0.5;
        let radius = (bbox.max() - bbox.min()).length() * 0.5;

        SphereClass::new(center, radius)
    }
}

impl RenderObjClass for ParticleEmitterClass {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn class_id(&self) -> RenderObjClassId {
        RenderObjClassId::ParticleEmitter
    }

    fn clone_render_obj(&self) -> Box<dyn RenderObjClass> {
        Box::new(self.clone())
    }

    fn clone_obj(&self) -> Box<dyn RenderObjClass> {
        Box::new(self.clone())
    }

    fn get_num_polys(&self) -> usize {
        self.max_particles
    }

    fn render(&self, _rinfo: &RenderInfoClass) -> RendererResult<()> {
        // Particle rendering would be implemented here
        // This would involve setting up WGPU render passes for particles
        Ok(())
    }

    fn special_render(&self, _rinfo: &SpecialRenderInfoClass) -> RendererResult<()> {
        // Special rendering implementation
        Ok(())
    }

    fn cast_ray(&self, _raytest: &mut RayCollisionTestClass) -> bool {
        false // Particles don't typically collide with rays
    }

    fn cast_aabox(&self, _boxtest: &mut AABoxCollisionTestClass) -> bool {
        false // Placeholder
    }

    fn cast_obbox(&self, _boxtest: &mut OBBoxCollisionTestClass) -> bool {
        false // Placeholder
    }

    fn intersect_aabox(&self, _boxtest: &AABoxIntersectionTestClass) -> bool {
        false // Placeholder
    }

    fn intersect_obbox(&self, _boxtest: &OBBoxIntersectionTestClass) -> bool {
        false // Placeholder
    }

    fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        self.calculate_bounding_sphere()
    }

    fn get_obj_space_bounding_box(&self) -> AABoxClass {
        self.calculate_bounding_box()
    }

    fn scale(&mut self, scale: f32) {
        // Scale the emitter
        self.transform = self.transform * Mat4::from_scale(Vec3::splat(scale));
    }

    fn scale_xyz(&mut self, scalex: f32, scaley: f32, scalez: f32) {
        // Scale the emitter with separate axes
        self.transform = self.transform * Mat4::from_scale(Vec3::new(scalex, scaley, scalez));
    }

    fn get_material_info(&self) -> Option<&MaterialInfoClass> {
        None // Particles don't have material info in this simple implementation
    }

    fn get_sort_level(&self) -> i32 {
        0 // Default sort level
    }

    fn set_sort_level(&mut self, _level: i32) {
        // Placeholder implementation
    }

    fn create_decal(&mut self, _generator: &mut DecalGeneratorClass) {
        // Particles don't create decals
    }

    fn delete_decal(&mut self, _decal_id: u32) {
        // Particles don't have decals
    }

    fn transform(&self) -> &Mat4 {
        &self.transform
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        self.transform_identity = self.transform == Mat4::IDENTITY;
    }

    fn set_position(&mut self, position: Vec3) {
        let vec3_pos = vector3_to_vec3(position);
        self.transform.w_axis = Vec4::new(vec3_pos.x, vec3_pos.y, vec3_pos.z, 1.0);
        self.transform_identity = false;
    }

    fn get_transform(&self) -> &Mat4 {
        &self.transform
    }

    fn get_bounding_sphere(&self) -> SphereClass {
        self.calculate_bounding_sphere()
    }

    fn get_bounding_box(&self) -> AABoxClass {
        self.calculate_bounding_box()
    }

    fn get_name(&self) -> &str {
        self.name.as_str()
    }

    fn set_name(&mut self, name: &str) {
        self.name = StringClass::from(name);
    }

    fn add_engine_ref(&self) {
        self.ref_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn release_engine_ref(&self) {
        let old_count = self
            .ref_count
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        if old_count == 1 {
            // Emitter will be dropped when this reference goes out of scope
        }
    }

    fn engine_refs(&self) -> usize {
        self.ref_count.load(std::sync::atomic::Ordering::Relaxed) as usize
    }
}

impl Clone for ParticleEmitterClass {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            transform: self.transform,
            transform_identity: self.transform_identity,
            emit_rate: self.emit_rate,
            burst_size: self.burst_size,
            emit_accumulator: 0.0, // Reset accumulator for clone
            base_velocity: self.base_velocity,
            velocity_inheritance_factor: self.velocity_inheritance_factor,
            acceleration: self.acceleration,
            position_randomizer: None, // Would need to clone if present
            velocity_randomizer: None, // Would need to clone if present
            color_property: self.color_property.clone(),
            opacity_property: self.opacity_property.clone(),
            size_property: self.size_property.clone(),
            rotation_property: self.rotation_property.clone(),
            frame_property: self.frame_property.clone(),
            blur_times_property: self.blur_times_property.clone(),
            texture: self.texture.clone(),
            shader: self.shader.clone(),
            max_age: self.max_age,
            future_start: self.future_start,
            orientation_randomization: self.orientation_randomization,
            render_mode: self.render_mode,
            frame_mode: self.frame_mode,
            max_particles: self.max_particles,
            max_buffer_size: self.max_buffer_size,
            pingpong: self.pingpong,
            scale: self.scale,
            particles: vec![Particle::default(); self.max_particles],
            active_count: 0, // Reset active count for clone
            ref_count: std::sync::atomic::AtomicU32::new(1),
        }
    }
}

/// Particle emitter utilities
pub struct ParticleEmitterUtils;

impl ParticleEmitterUtils {
    /// Create explosion emitter
    pub fn create_explosion_emitter() -> ParticleEmitterClass {
        let mut emitter = ParticleEmitterClass::new(
            100.0,              // High emission rate
            50,                 // Burst size
            1000,               // Max particles
            None,               // No texture initially
            ShaderClass::new(), // Default shader
        );

        // Set explosion properties
        emitter.set_base_velocity(Vec3::ZERO);
        emitter.set_acceleration(Vec3::new(0.0, -9.81, 0.0)); // Gravity
        emitter.set_max_age(2.0);

        // Color gradient: orange to red to black
        emitter.set_color_keyframes(
            vec![0.0, 0.3, 1.0],
            vec![
                Vec3::new(1.0, 0.5, 0.0), // Orange
                Vec3::new(1.0, 0.0, 0.0), // Red
                Vec3::new(0.0, 0.0, 0.0), // Black
            ],
        );

        // Size gradient: small to large
        emitter.set_size_keyframes(vec![0.0, 0.5, 1.0], vec![0.1, 0.5, 0.1]);

        emitter
    }

    /// Create smoke emitter
    pub fn create_smoke_emitter() -> ParticleEmitterClass {
        let mut emitter = ParticleEmitterClass::new(
            20.0,               // Moderate emission rate
            10,                 // Burst size
            500,                // Max particles
            None,               // No texture initially
            ShaderClass::new(), // Default shader
        );

        // Set smoke properties
        emitter.set_base_velocity(Vec3::new(0.0, 1.0, 0.0)); // Upward
        emitter.set_acceleration(Vec3::new(0.0, 0.5, 0.0)); // Slight upward acceleration
        emitter.set_max_age(5.0);

        // Color gradient: dark gray to light gray to transparent
        emitter.set_color_keyframes(
            vec![0.0, 0.7, 1.0],
            vec![
                Vec3::new(0.2, 0.2, 0.2), // Dark gray
                Vec3::new(0.6, 0.6, 0.6), // Light gray
                Vec3::new(1.0, 1.0, 1.0), // White
            ],
        );

        // Opacity gradient: solid to transparent
        emitter.set_opacity_keyframes(vec![0.0, 0.8, 1.0], vec![0.8, 0.3, 0.0]);

        // Size gradient: small to large
        emitter.set_size_keyframes(vec![0.0, 1.0], vec![0.5, 2.0]);

        emitter
    }

    /// Create fire emitter
    pub fn create_fire_emitter() -> ParticleEmitterClass {
        let mut emitter = ParticleEmitterClass::new(
            50.0,               // High emission rate
            25,                 // Burst size
            750,                // Max particles
            None,               // No texture initially
            ShaderClass::new(), // Default shader
        );

        // Set fire properties
        emitter.set_base_velocity(Vec3::new(0.0, 2.0, 0.0)); // Upward
        emitter.set_acceleration(Vec3::new(0.0, 1.0, 0.0)); // Upward acceleration
        emitter.set_max_age(1.5);

        // Color gradient: red to orange to yellow
        emitter.set_color_keyframes(
            vec![0.0, 0.5, 1.0],
            vec![
                Vec3::new(1.0, 0.0, 0.0), // Red
                Vec3::new(1.0, 0.5, 0.0), // Orange
                Vec3::new(1.0, 1.0, 0.0), // Yellow
            ],
        );

        // Size gradient: small to medium
        emitter.set_size_keyframes(vec![0.0, 0.7, 1.0], vec![0.2, 0.8, 0.1]);

        emitter
    }
}
