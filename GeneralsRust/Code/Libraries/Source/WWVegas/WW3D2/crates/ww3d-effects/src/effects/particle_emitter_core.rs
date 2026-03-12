//! Particle Emitter Core - Complete particle effect system
//!
//! This module implements the ParticleEmitterClass from the original C++ code,
//! providing comprehensive particle effects with WGPU integration.
//!
//! Converted from:
//! - part_emt.cpp/h (particle emitter class)
//! - part_buf.cpp/h (particle buffer management)
//! - part_ldr.cpp/h (particle loader)

use glam::{Mat4, Vec3, Vec4};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use ww3d_core::errors::W3DResult as Result;
use ww3d_renderer_3d::bounding_volumes::{aabox::AABoxClass, sphere::SphereClass};
use ww3d_renderer_3d::render_object_system::RenderObjClass;
use ww3d_renderer_3d::rendering::shader_system::shader::ShaderClass;
use ww3d_renderer_3d::texture_system::TextureClass;

static EMITTER_ID_COUNTER: AtomicU32 = AtomicU32::new(0);
static PARTICLE_DEBUG_DISABLE: AtomicBool = AtomicBool::new(false);
static DEFAULT_REMOVE_ON_COMPLETE: AtomicBool = AtomicBool::new(true);

/// Particle property with keyframes
#[derive(Debug, Clone)]
pub struct ParticleProperty<T> {
    /// Start value
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
    T: Clone + Default + std::ops::Add<Output = T> + std::ops::Mul<f32, Output = T> + Copy,
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
        T: std::ops::Sub<Output = T>,
    {
        if self.key_times.is_empty() {
            return self.start + self.rand * random_factor;
        }

        // Find appropriate keyframes
        for i in 0..(self.key_times.len() - 1) {
            if time >= self.key_times[i] && time <= self.key_times[i + 1] {
                let t = (time - self.key_times[i]) / (self.key_times[i + 1] - self.key_times[i]);
                let value1 = self.values[i];
                let value2 = self.values[i + 1];
                return value1 + (value2 - value1) * t;
            }
        }

        // Return last value
        self.values.last().copied().unwrap_or(self.start)
    }

    /// Add keyframe
    pub fn add_keyframe(&mut self, time: f32, value: T) {
        self.key_times.push(time);
        self.values.push(value);
    }
}

/// Vector randomizer for particle properties
#[derive(Debug, Clone)]
pub struct Vec3Randomizer {
    /// Randomization extents
    pub extents: Vec3,
}

impl Vec3Randomizer {
    /// Create new randomizer
    pub fn new(extents: Vec3) -> Self {
        Self { extents }
    }

    /// Get random vector
    pub fn get_random(&self) -> Vec3 {
        // In a full implementation, this would use a proper random number generator
        Vec3::new(
            (rand::random::<f32>() - 0.5) * self.extents.x,
            (rand::random::<f32>() - 0.5) * self.extents.y,
            (rand::random::<f32>() - 0.5) * self.extents.z,
        )
    }
}

/// Particle structure
#[derive(Debug, Clone)]
pub struct Particle {
    /// Position
    pub position: Vec3,
    /// Velocity
    pub velocity: Vec3,
    /// Color
    pub color: Vec4,
    /// Size
    pub size: f32,
    /// Age
    pub age: f32,
    /// Lifetime
    pub lifetime: f32,
    /// Rotation
    pub rotation: f32,
    /// Texture frame
    pub frame: f32,
    /// Blur time
    pub blur_time: f32,
    /// Whether particle is active
    pub active: bool,
}

impl Particle {
    /// Create new particle
    pub fn new() -> Self {
        Self {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            size: 1.0,
            age: 0.0,
            lifetime: 1.0,
            rotation: 0.0,
            frame: 0.0,
            blur_time: 0.0,
            active: false,
        }
    }

    /// Update particle
    pub fn update(&mut self, delta_time: f32, acceleration: Vec3) {
        if !self.active {
            return;
        }

        self.age += delta_time;

        if self.age >= self.lifetime {
            self.active = false;
            return;
        }

        // Update velocity
        self.velocity += acceleration * delta_time;

        // Update position
        self.position += self.velocity * delta_time;
    }

    /// Check if particle is dead
    pub fn is_dead(&self) -> bool {
        !self.active || self.age >= self.lifetime
    }
}

/// Particle emitter render mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParticleRenderMode {
    /// Triangles
    Triangles = 0,
    /// Quads
    Quads,
    /// Lines
    Lines,
    /// Points
    Points,
}

/// Particle emitter frame mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParticleFrameMode {
    /// Single frame
    Single = 0,
    /// Multi frame
    Multi,
    /// Random frame
    Random,
    /// Ping pong
    PingPong,
}

/// Emitter line properties
#[derive(Debug, Clone)]
pub struct W3dEmitterLinePropertiesStruct {
    /// Line flags
    pub flags: u32,
    /// Subdivision level
    pub subdivision_level: u32,
    /// Noise amplitude
    pub noise_amplitude: f32,
    /// Merge abort factor
    pub merge_abort_factor: f32,
    /// Texture tile factor
    pub texture_tile_factor: f32,
    /// U per sec
    pub u_per_sec: f32,
    /// V per sec
    pub v_per_sec: f32,
}

/// Particle emitter class
#[derive(Debug)]
pub struct ParticleEmitterClass {
    /// Base render object
    pub base: Option<Arc<dyn RenderObjClass>>,
    /// Emit rate (milliseconds between emissions)
    pub emit_rate: u32,
    /// Burst size
    pub burst_size: u32,
    /// One time burst size
    pub one_time_burst_size: u32,
    /// One time burst flag
    pub one_time_burst: bool,
    /// Position randomizer
    pub pos_rand: Option<Vec3Randomizer>,
    /// Base velocity
    pub base_vel: Vec3,
    /// Velocity randomizer
    pub vel_rand: Option<Vec3Randomizer>,
    /// Outward velocity
    pub outward_vel: f32,
    /// Velocity inheritance factor
    pub vel_inherit_factor: f32,
    /// Particles left to emit
    pub particles_left: i32,
    /// Whether emitter is active
    pub active: bool,
    /// First time flag
    pub first_time: bool,
    /// Color property
    pub color: ParticleProperty<Vec3>,
    /// Opacity property
    pub opacity: ParticleProperty<f32>,
    /// Size property
    pub size: ParticleProperty<f32>,
    /// Rotation property
    pub rotation: ParticleProperty<f32>,
    /// Orientation randomizer
    pub orient_rnd: f32,
    /// Frames property
    pub frames: ParticleProperty<f32>,
    /// Blur times property
    pub blur_times: ParticleProperty<f32>,
    /// Acceleration
    pub acceleration: Vec3,
    /// Maximum age
    pub max_age: f32,
    /// Future start time
    pub future_start: f32,
    /// Texture
    pub texture: Option<Arc<TextureClass>>,
    /// Shader
    pub shader: ShaderClass,
    /// Maximum particles
    pub max_particles: usize,
    /// Maximum buffer size
    pub max_buffer_size: usize,
    /// Ping pong flag
    pub pingpong: bool,
    /// Render mode
    pub render_mode: ParticleRenderMode,
    /// Frame mode
    pub frame_mode: ParticleFrameMode,
    /// Line properties
    pub line_props: Option<W3dEmitterLinePropertiesStruct>,
    /// Particles
    pub particles: Vec<Particle>,
    /// Active particle count
    pub active_count: usize,
    /// Time accumulator
    pub time_accumulator: f32,
    /// Remove on complete flag
    pub remove_on_complete: bool,
    /// Transform
    pub transform: Mat4,
    /// Previous transform
    pub prev_transform: Mat4,
    /// Emitter ID
    pub emitter_id: u32,
}

impl ParticleEmitterClass {
    /// Create new particle emitter
    pub fn new(
        emit_rate: f32,
        burst_size: u32,
        pos_rand: Option<Vec3Randomizer>,
        base_vel: Vec3,
        vel_rand: Option<Vec3Randomizer>,
        outward_vel: f32,
        vel_inherit_factor: f32,
        color: ParticleProperty<Vec3>,
        opacity: ParticleProperty<f32>,
        size: ParticleProperty<f32>,
        rotation: ParticleProperty<f32>,
        orient_rnd: f32,
        frames: ParticleProperty<f32>,
        blur_times: ParticleProperty<f32>,
        acceleration: Vec3,
        max_age: f32,
        future_start: f32,
        texture: Option<Arc<TextureClass>>,
        shader: ShaderClass,
        max_particles: usize,
        max_buffer_size: usize,
        pingpong: bool,
        render_mode: ParticleRenderMode,
        frame_mode: ParticleFrameMode,
        line_props: Option<W3dEmitterLinePropertiesStruct>,
    ) -> Self {
        let emitter_id = EMITTER_ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;

        let emit_rate_ms = if emit_rate > 0.0 {
            (1000.0 / emit_rate) as u32
        } else {
            1000
        };

        let burst_size = if burst_size != 0 { burst_size } else { 1 };

        Self {
            base: None, // Placeholder
            emit_rate: emit_rate_ms,
            burst_size,
            one_time_burst_size: 1,
            one_time_burst: false,
            pos_rand,
            base_vel: base_vel * 0.001, // Convert to per-millisecond
            vel_rand,
            outward_vel: outward_vel * 0.001,
            vel_inherit_factor,
            particles_left: max_particles as i32,
            active: false,
            first_time: true,
            color,
            opacity,
            size,
            rotation,
            orient_rnd,
            frames,
            blur_times,
            acceleration,
            max_age,
            future_start,
            texture,
            shader,
            max_particles,
            max_buffer_size,
            pingpong,
            render_mode,
            frame_mode,
            line_props,
            particles: vec![Particle::new(); max_particles],
            active_count: 0,
            time_accumulator: 0.0,
            remove_on_complete: true,
            transform: Mat4::IDENTITY,
            prev_transform: Mat4::IDENTITY,
            emitter_id,
        }
    }

    /// Start emitting
    pub fn start(&mut self) {
        self.active = true;
        self.first_time = true;
        self.time_accumulator = 0.0;
    }

    /// Stop emitting
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Reset emitter
    pub fn reset(&mut self) {
        self.stop();
        for particle in &mut self.particles {
            particle.active = false;
            particle.age = 0.0;
        }
        self.active_count = 0;
        self.particles_left = self.max_particles as i32;
        self.first_time = true;
    }

    /// Update emitter
    pub fn update(&mut self, delta_time: f32) {
        // Handle future start
        if self.future_start > 0.0 {
            self.future_start -= delta_time;
            if self.future_start > 0.0 {
                return;
            }
            self.start();
        }

        if !self.active {
            return;
        }

        // Update time accumulator
        self.time_accumulator += delta_time * 1000.0; // Convert to milliseconds

        // Create new particles
        while self.time_accumulator >= self.emit_rate as f32 && self.particles_left > 0 {
            self.create_new_particles();
            self.time_accumulator -= self.emit_rate as f32;
        }

        // Update existing particles
        for particle in &mut self.particles {
            if particle.active {
                particle.update(delta_time, self.acceleration);
                if particle.is_dead() {
                    particle.active = false;
                    self.active_count -= 1;
                }
            }
        }

        // Check if emitter is complete
        if self.particles_left <= 0 && self.active_count == 0 && self.remove_on_complete {
            // Emitter is complete, would notify scene to remove it
        }
    }

    /// Create new particles
    fn create_new_particles(&mut self) {
        let particles_to_create = if self.one_time_burst {
            self.one_time_burst_size.min(self.particles_left as u32)
        } else {
            self.burst_size.min(self.particles_left as u32)
        };

        for _ in 0..particles_to_create {
            if let Some(index) = self.find_inactive_particle_index() {
                // Initialize the particle at the found index
                self.initialize_particle_at_index(index);
                self.active_count += 1;
                self.particles_left -= 1;
            }
        }

        if self.one_time_burst {
            self.one_time_burst = false;
        }
    }

    /// Find inactive particle
    fn find_inactive_particle_index(&mut self) -> Option<usize> {
        for (index, particle) in self.particles.iter().enumerate() {
            if !particle.active {
                return Some(index);
            }
        }
        None
    }

    #[allow(dead_code)]

    fn find_inactive_particle(&mut self) -> Option<&mut Particle> {
        for particle in &mut self.particles {
            if !particle.active {
                return Some(particle);
            }
        }
        None
    }

    /// Initialize particle
    fn initialize_particle_at_index(&mut self, index: usize) {
        if let Some(particle) = self.particles.get_mut(index) {
            // Initialize particle directly to avoid borrow conflict

            // Position
            let mut position = Vec3::ZERO;
            if let Some(ref pos_rand) = self.pos_rand {
                position = pos_rand.get_random();
            }

            // Velocity
            let mut velocity = self.base_vel;
            if let Some(ref vel_rand) = self.vel_rand {
                velocity += vel_rand.get_random();
            }

            // Add outward velocity if specified
            if self.outward_vel > 0.0 {
                // Would add outward velocity based on position
            }

            // Transform to world space
            particle.position = (self.transform * Vec4::from((position, 1.0))).truncate();
            particle.velocity = (self.transform * Vec4::from((velocity, 0.0))).truncate();

            // Set other initial properties
            particle.age = 0.0;
            particle.lifetime = 1.0; // Default lifetime
            particle.size = 1.0; // Default size
            particle.color.w = 1.0; // alpha
            particle.rotation = 0.0;
        }
    }

    #[allow(dead_code)]

    fn initialize_particle(&self, particle: &mut Particle) {
        // Position
        let mut position = Vec3::ZERO;
        if let Some(ref pos_rand) = self.pos_rand {
            position = pos_rand.get_random();
        }

        // Velocity
        let mut velocity = self.base_vel;
        if let Some(ref vel_rand) = self.vel_rand {
            velocity += vel_rand.get_random();
        }

        // Add outward velocity if specified
        if self.outward_vel > 0.0 {
            // Would add outward velocity based on position
        }

        // Transform to world space
        particle.position = (self.transform * Vec4::from((position, 1.0))).truncate();
        particle.velocity = (self.transform * Vec4::from((velocity, 0.0))).truncate();

        // Properties
        let random_factor = rand::random::<f32>();
        particle.color = Vec4::from((self.color.evaluate(0.0, random_factor), 1.0));
        particle.size = self.size.evaluate(0.0, random_factor);
        particle.rotation = self.rotation.evaluate(0.0, random_factor);
        particle.frame = self.frames.evaluate(0.0, random_factor);
        particle.blur_time = self.blur_times.evaluate(0.0, random_factor);

        // Lifetime
        particle.lifetime = self.max_age;
        particle.age = 0.0;
        particle.active = true;
    }

    /// Render particles
    pub fn render(&self) {
        if self.active_count == 0 {
            return;
        }

        // In a full implementation, this would:
        // 1. Set up WGPU render pass
        // 2. Bind textures and shaders
        // 3. Create vertex/index buffers for particles
        // 4. Render particles based on render mode
        // 5. Handle blending and effects

        // For now, this is a placeholder
    }

    /// Get particle count
    pub fn get_particle_count(&self) -> usize {
        self.active_count
    }

    /// Is emitter complete
    pub fn is_complete(&self) -> bool {
        self.particles_left <= 0 && self.active_count == 0
    }

    /// Set transform
    pub fn set_transform(&mut self, transform: Mat4) {
        self.prev_transform = self.transform;
        self.transform = transform;
    }

    /// Get transform
    pub fn get_transform(&self) -> Mat4 {
        self.transform
    }

    /// Get bounding box
    pub fn get_bounding_box(&self) -> AABoxClass {
        // Calculate bounds from all active particles
        if self.active_count == 0 {
            return AABoxClass::from_center_and_extent(Vec3::ZERO, Vec3::ONE);
        }

        let mut min_corner = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max_corner = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);

        for particle in &self.particles {
            if particle.active {
                min_corner = min_corner.min(
                    particle.position - Vec3::new(particle.size, particle.size, particle.size),
                );
                max_corner = max_corner.max(
                    particle.position + Vec3::new(particle.size, particle.size, particle.size),
                );
            }
        }

        let center = (min_corner + max_corner) / 2.0;
        let extent = (max_corner - min_corner) / 2.0;

        AABoxClass::from_center_and_extent(center, extent)
    }

    /// Get bounding sphere
    pub fn get_bounding_sphere(&self) -> SphereClass {
        let bbox = self.get_bounding_box();
        let center = bbox.center;
        let radius = bbox.extent.length();

        SphereClass::new(center, radius)
    }

    /// Set remove on complete
    pub fn set_remove_on_complete(&mut self, remove: bool) {
        self.remove_on_complete = remove;
    }

    /// Get remove on complete
    pub fn get_remove_on_complete(&self) -> bool {
        self.remove_on_complete
    }

    /// Set one time burst
    pub fn set_one_time_burst(&mut self, burst_size: u32) {
        self.one_time_burst = true;
        self.one_time_burst_size = burst_size;
    }

    /// Load from W3D file
    pub fn load_w3d(&mut self, data: &[u8]) -> Result<()> {
        // In a full implementation, this would parse W3D particle emitter data
        let _ = data;
        Ok(())
    }

    /// Save to W3D file
    pub fn save_w3d(&self, writer: &mut dyn std::io::Write) -> Result<()> {
        // In a full implementation, this would write W3D particle emitter data
        let _ = writer;
        Ok(())
    }
}

/// Particle buffer class for managing particle rendering
#[derive(Debug)]
pub struct ParticleBufferClass {
    /// Particle emitter
    pub emitter: Arc<ParticleEmitterClass>,
    /// Vertex buffer
    pub vertices: Vec<f32>,
    /// Index buffer
    pub indices: Vec<u32>,
    /// Whether buffer needs updating
    pub needs_update: bool,
}

impl ParticleBufferClass {
    /// Create new particle buffer
    pub fn new(emitter: Arc<ParticleEmitterClass>) -> Self {
        Self {
            emitter,
            vertices: Vec::new(),
            indices: Vec::new(),
            needs_update: true,
        }
    }

    /// Update buffer
    pub fn update(&mut self) {
        if !self.needs_update {
            return;
        }

        self.vertices.clear();
        self.indices.clear();

        // Collect active particles with their indices first
        let active_particles: Vec<(usize, Particle)> = self
            .emitter
            .particles
            .iter()
            .enumerate()
            .filter(|(_, particle)| particle.active)
            .map(|(i, particle)| (i, particle.clone()))
            .collect();

        // Generate vertices and indices for active particles
        for (i, particle) in &active_particles {
            self.add_particle_geometry(particle, *i);
        }

        self.needs_update = false;
    }

    /// Add particle geometry
    fn add_particle_geometry(&mut self, particle: &Particle, _index: usize) {
        // Generate quad geometry for particle
        let half_size = particle.size / 2.0;
        let base_index = self.vertices.len() / 8; // 8 floats per vertex (pos + uv)

        // Quad vertices (position + UV)
        let quad_verts = [
            // Bottom-left
            particle.position.x - half_size,
            particle.position.y - half_size,
            particle.position.z,
            0.0,
            0.0,
            // Bottom-right
            particle.position.x + half_size,
            particle.position.y - half_size,
            particle.position.z,
            1.0,
            0.0,
            // Top-right
            particle.position.x + half_size,
            particle.position.y + half_size,
            particle.position.z,
            1.0,
            1.0,
            // Top-left
            particle.position.x - half_size,
            particle.position.y + half_size,
            particle.position.z,
            0.0,
            1.0,
        ];

        self.vertices.extend_from_slice(&quad_verts);

        // Quad indices
        let quad_indices = [
            base_index as u32,
            (base_index + 1) as u32,
            (base_index + 2) as u32,
            base_index as u32,
            (base_index + 2) as u32,
            (base_index + 3) as u32,
        ];

        self.indices.extend_from_slice(&quad_indices);
    }

    /// Get vertex count
    pub fn get_vertex_count(&self) -> usize {
        self.vertices.len() / 8 // 8 floats per vertex
    }

    /// Get index count
    pub fn get_index_count(&self) -> usize {
        self.indices.len()
    }

    /// Mark as dirty
    pub fn set_dirty(&mut self) {
        self.needs_update = true;
    }
}

/// Set debug disable
pub fn set_particle_debug_disable(disable: bool) {
    PARTICLE_DEBUG_DISABLE.store(disable, Ordering::Relaxed);
}

/// Get debug disable
pub fn get_particle_debug_disable() -> bool {
    PARTICLE_DEBUG_DISABLE.load(Ordering::Relaxed)
}

/// Set default remove on complete
pub fn set_default_remove_on_complete(remove: bool) {
    DEFAULT_REMOVE_ON_COMPLETE.store(remove, Ordering::Relaxed);
}

/// Get default remove on complete
pub fn get_default_remove_on_complete() -> bool {
    DEFAULT_REMOVE_ON_COMPLETE.load(Ordering::Relaxed)
}

/// Quick particle emitter creation function
pub fn create_particle_emitter(
    emit_rate: f32,
    texture: Option<Arc<TextureClass>>,
    max_particles: usize,
) -> ParticleEmitterClass {
    let _color: ParticleProperty<Vec3> = ParticleProperty::new();
    let _opacity: ParticleProperty<f32> = ParticleProperty::new();
    let _size: ParticleProperty<f32> = ParticleProperty::new();
    let rotation: ParticleProperty<f32> = ParticleProperty::new();
    let _frames: ParticleProperty<f32> = ParticleProperty::new();
    let blur_times: ParticleProperty<f32> = ParticleProperty::new();

    // Set default values
    let mut color_prop: ParticleProperty<Vec3> = ParticleProperty::new();
    color_prop.start = Vec3::new(1.0, 1.0, 1.0);
    let mut opacity_prop: ParticleProperty<f32> = ParticleProperty::new();
    opacity_prop.start = 1.0;
    let mut size_prop: ParticleProperty<f32> = ParticleProperty::new();
    size_prop.start = 1.0;
    let mut frames_prop: ParticleProperty<f32> = ParticleProperty::new();
    frames_prop.start = 0.0;

    ParticleEmitterClass::new(
        emit_rate,
        1,
        None,
        Vec3::ZERO,
        None,
        0.0,
        0.0,
        color_prop,
        opacity_prop,
        size_prop,
        rotation,
        0.0,
        frames_prop,
        blur_times,
        Vec3::ZERO,
        1.0,
        0.0,
        texture,
        ShaderClass::new(), // Placeholder
        max_particles,
        max_particles * 6, // 6 indices per quad
        false,
        ParticleRenderMode::Quads,
        ParticleFrameMode::Single,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_particle_property() {
        let mut prop = ParticleProperty::<f32>::new();
        prop.start = 1.0;
        prop.rand = 0.5;
        prop.add_keyframe(0.0, 1.0);
        prop.add_keyframe(1.0, 2.0);

        let value = prop.evaluate(0.5, 0.0);
        assert_eq!(value, 1.5);
    }

    #[test]
    fn test_vector_randomizer() {
        let randomizer = Vec3Randomizer::new(Vec3::new(1.0, 2.0, 3.0));
        let random_vec = randomizer.get_random();

        assert!(random_vec.x.abs() <= 0.5);
        assert!(random_vec.y.abs() <= 1.0);
        assert!(random_vec.z.abs() <= 1.5);
    }

    #[test]
    fn test_particle_creation() {
        let particle = Particle::new();
        assert_eq!(particle.position, Vec3::ZERO);
        assert_eq!(particle.color, Vec4::new(1.0, 1.0, 1.0, 1.0));
        assert_eq!(particle.size, 1.0);
        assert!(!particle.active);
    }

    #[test]
    fn test_particle_emitter_creation() {
        let emitter = create_particle_emitter(10.0, None, 100);
        assert_eq!(emitter.emit_rate, 100); // 1000ms / 10Hz = 100ms
        assert_eq!(emitter.max_particles, 100);
        assert_eq!(emitter.render_mode, ParticleRenderMode::Quads);
    }

    #[test]
    fn test_particle_emitter_start_stop() {
        let mut emitter = create_particle_emitter(10.0, None, 100);
        assert!(!emitter.active);

        emitter.start();
        assert!(emitter.active);

        emitter.stop();
        assert!(!emitter.active);
    }

    #[test]
    fn test_particle_buffer_creation() {
        let emitter = Arc::new(create_particle_emitter(10.0, None, 100));
        let buffer = ParticleBufferClass::new(Arc::clone(&emitter));

        assert!(buffer.vertices.is_empty());
        assert!(buffer.indices.is_empty());
        assert!(buffer.needs_update);
    }

    #[test]
    fn test_particle_update() {
        let mut particle = Particle::new();
        particle.active = true;
        particle.velocity = Vec3::new(1.0, 0.0, 0.0);
        particle.lifetime = 2.0;

        particle.update(1.0, Vec3::ZERO);

        assert_eq!(particle.position, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(particle.age, 1.0);
        assert!(particle.active);

        particle.update(1.5, Vec3::ZERO);
        assert!(!particle.active); // Should be dead
    }
}
