//! Integration with Main Renderer
//!
//! This module provides integration between the particle system and the main WW3D renderer,
//! ensuring proper render order, batching, and efficient GPU usage.

use super::buffer::ParticleBuffer;
use super::emitter::ParticleEmitter;
use super::manager::ParticleSystemManager;
use super::point_group::ParticleInstanceData;
use crate::ParticleSystem;
use glam::{Mat4, Vec3};
use std::sync::{Arc, Mutex};
use wgpu::{CommandEncoder, Device, Queue, RenderPass};
use ww3d_collision::SphereClass;

// Note: Particle system integration with scene/camera requires trait-based abstraction
// to avoid circular dependencies (particles -> renderer-3d -> scene -> particles).
// Solution: Define Camera/Scene traits in ww3d-core, implement in respective crates.
// C++ equivalent: Forward declarations and interface segregation

#[derive(Debug, Clone)]
pub struct SceneParticleBuffer {
    pub buffer: Arc<Mutex<ParticleBuffer>>,
    pub blend_mode: BlendMode,
}

#[derive(Debug, Default)]
pub struct SceneClass {
    particle_buffers: Vec<SceneParticleBuffer>,
}

impl SceneClass {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_particle_buffer(
        &mut self,
        buffer: Arc<Mutex<ParticleBuffer>>,
        blend_mode: BlendMode,
    ) {
        self.particle_buffers
            .push(SceneParticleBuffer { buffer, blend_mode });
    }

    pub fn clear_particle_buffers(&mut self) {
        self.particle_buffers.clear();
    }

    pub fn particle_buffers(&self) -> &[SceneParticleBuffer] {
        &self.particle_buffers
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CameraClass {
    position: Vec3,
}

impl CameraClass {
    pub fn new(position: Vec3) -> Self {
        Self { position }
    }

    pub fn get_position(&self) -> Vec3 {
        self.position
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
    }
}

/// Particle rendering pass manager
#[derive(Debug)]
pub struct ParticleRenderPass {
    /// Opaque particles (rendered after opaque geometry)
    pub opaque_buffers: Vec<Arc<Mutex<ParticleBuffer>>>,
    /// Transparent particles (rendered after transparent geometry)
    pub transparent_buffers: Vec<Arc<Mutex<ParticleBuffer>>>,
    /// Additive particles (rendered last)
    pub additive_buffers: Vec<Arc<Mutex<ParticleBuffer>>>,
    /// GPU instancing support
    pub instancing_enabled: bool,
    /// Maximum instances per batch
    pub max_instances_per_batch: usize,
}

impl ParticleRenderPass {
    fn lock_buffer_sphere(buffer: &Arc<Mutex<ParticleBuffer>>) -> SphereClass {
        buffer
            .lock()
            .map(|buf| buf.get_bounding_sphere())
            .unwrap_or_else(|_| SphereClass::empty())
    }

    /// Create a new particle render pass
    pub fn new() -> Self {
        Self {
            opaque_buffers: Vec::new(),
            transparent_buffers: Vec::new(),
            additive_buffers: Vec::new(),
            instancing_enabled: true,
            max_instances_per_batch: 1024,
        }
    }

    /// Add a particle buffer to the appropriate render queue
    pub fn add_buffer(&mut self, buffer: Arc<Mutex<ParticleBuffer>>, blend_mode: BlendMode) {
        match blend_mode {
            BlendMode::Opaque => self.opaque_buffers.push(buffer),
            BlendMode::Alpha => self.transparent_buffers.push(buffer),
            BlendMode::Additive => self.additive_buffers.push(buffer),
        }
    }

    /// Clear all render queues
    pub fn clear(&mut self) {
        self.opaque_buffers.clear();
        self.transparent_buffers.clear();
        self.additive_buffers.clear();
    }

    /// Populate render queues from a scene particle registry.
    pub fn collect_scene_buffers(&mut self, scene: &SceneClass) {
        self.clear();
        for entry in scene.particle_buffers() {
            self.add_buffer(entry.buffer.clone(), entry.blend_mode);
        }
    }

    /// Sort transparent particles by depth
    pub fn sort_transparent_particles(&mut self, camera_position: Vec3) {
        // Sort transparent buffers by distance from camera
        self.transparent_buffers.sort_by(|a, b| {
            let sphere_a = Self::lock_buffer_sphere(a);
            let sphere_b = Self::lock_buffer_sphere(b);
            let dist_a = (sphere_a.center - camera_position).length_squared();
            let dist_b = (sphere_b.center - camera_position).length_squared();
            dist_b
                .partial_cmp(&dist_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Render all particle buffers in proper order
    /// Note: Rendering Arc<ParticleBuffer> requires thread-safe mutability (RwLock or Mutex)
    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        render_pass: &mut RenderPass,
        view_projection_matrix: Mat4,
        _camera_position: Vec3,
    ) {
        for buffer in &self.opaque_buffers {
            if let Ok(mut buffer) = buffer.lock() {
                buffer.render(device, queue, encoder, render_pass, view_projection_matrix);
            }
        }

        for buffer in &self.transparent_buffers {
            if let Ok(mut buffer) = buffer.lock() {
                buffer.render(device, queue, encoder, render_pass, view_projection_matrix);
            }
        }

        for buffer in &self.additive_buffers {
            if let Ok(mut buffer) = buffer.lock() {
                buffer.render(device, queue, encoder, render_pass, view_projection_matrix);
            }
        }
    }

    /// Batch similar particles for efficient rendering
    pub fn batch_particles(&mut self) {
        if !self.instancing_enabled {
            return;
        }

        self.opaque_buffers.retain(buffer_has_active_particles);
        self.transparent_buffers.retain(buffer_has_active_particles);
        self.additive_buffers.retain(buffer_has_active_particles);
    }

    /// Get statistics for the current frame
    pub fn get_stats(&self) -> ParticleRenderStats {
        let total_buffers = self.opaque_buffers.len()
            + self.transparent_buffers.len()
            + self.additive_buffers.len();

        let mut total_particles = 0;
        for buffer in &self.opaque_buffers {
            if let Ok(buffer) = buffer.lock() {
                total_particles += buffer.get_active_count();
            }
        }
        for buffer in &self.transparent_buffers {
            if let Ok(buffer) = buffer.lock() {
                total_particles += buffer.get_active_count();
            }
        }
        for buffer in &self.additive_buffers {
            if let Ok(buffer) = buffer.lock() {
                total_particles += buffer.get_active_count();
            }
        }

        ParticleRenderStats {
            total_buffers,
            total_particles,
            opaque_buffers: self.opaque_buffers.len(),
            transparent_buffers: self.transparent_buffers.len(),
            additive_buffers: self.additive_buffers.len(),
            batches_rendered: total_buffers, // Simplified
        }
    }
}

fn buffer_has_active_particles(buffer: &Arc<Mutex<ParticleBuffer>>) -> bool {
    buffer
        .lock()
        .map(|buffer| buffer.get_active_count() > 0)
        .unwrap_or(false)
}

/// Particle blend modes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlendMode {
    Opaque,
    Alpha,
    Additive,
}

/// Particle render statistics
#[derive(Debug, Clone)]
pub struct ParticleRenderStats {
    pub total_buffers: usize,
    pub total_particles: usize,
    pub opaque_buffers: usize,
    pub transparent_buffers: usize,
    pub additive_buffers: usize,
    pub batches_rendered: usize,
}

/// Main particle system integration
#[derive(Debug)]
pub struct ParticleSystemIntegration {
    /// Particle manager
    pub particle_manager: ParticleSystemManager,
    /// Render pass manager
    pub render_pass: ParticleRenderPass,
    /// Frame statistics
    pub stats: ParticleRenderStats,
}

impl ParticleSystemIntegration {
    /// Create a new particle system integration
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self {
            particle_manager: ParticleSystemManager::new(device, queue),
            render_pass: ParticleRenderPass::new(),
            stats: ParticleRenderStats {
                total_buffers: 0,
                total_particles: 0,
                opaque_buffers: 0,
                transparent_buffers: 0,
                additive_buffers: 0,
                batches_rendered: 0,
            },
        }
    }

    /// Update the particle system
    pub fn update(&mut self, delta_time: f32, scene: &SceneClass) {
        // Update particle manager
        self.particle_manager.update((delta_time * 1000.0) as u32);

        // Clear render pass
        self.render_pass.collect_scene_buffers(scene);
    }

    /// Render the particle system
    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        render_pass: &mut RenderPass,
        view_projection_matrix: Mat4,
        camera: &CameraClass,
    ) {
        let camera_position = camera.get_position();
        self.particle_manager
            .set_world_view_matrix(view_projection_matrix);
        self.render_pass.sort_transparent_particles(camera_position);

        // Batch particles for efficient rendering
        self.render_pass.batch_particles();

        // Render all particles
        self.render_pass.render(
            device,
            queue,
            encoder,
            render_pass,
            view_projection_matrix,
            camera_position,
        );

        // Update statistics
        self.stats = self.render_pass.get_stats();
    }

    /// Get frame statistics
    pub fn get_stats(&self) -> &ParticleRenderStats {
        &self.stats
    }

    /// Enable/disable GPU instancing
    pub fn set_instancing_enabled(&mut self, enabled: bool) {
        self.render_pass.instancing_enabled = enabled;
    }

    /// Set maximum instances per batch
    pub fn set_max_instances_per_batch(&mut self, max_instances: usize) {
        self.render_pass.max_instances_per_batch = max_instances;
    }
}

/// Integration with scene system
pub trait ParticleSceneIntegration {
    /// Add particle emitter to scene
    fn add_particle_emitter(&mut self, emitter: ParticleEmitter);

    /// Remove particle emitter from scene
    fn remove_particle_emitter(&mut self, emitter_id: u32);

    /// Get all particle emitters in scene
    fn get_particle_emitters(&self) -> Vec<&ParticleEmitter>;

    /// Update all particle systems in scene
    fn update_particle_systems(&mut self, delta_time: f32);
}

/// GPU instancing data for batched particle rendering
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BatchedParticleInstance {
    /// World transform matrix (4x4)
    pub transform: [[f32; 4]; 4],
    /// Color and alpha
    pub color: [f32; 4],
    /// Size and rotation
    pub size_rotation: [f32; 2],
    /// Texture frame and animation data
    pub texture_data: [f32; 2],
}

impl BatchedParticleInstance {
    /// Create from particle instance data
    pub fn from_particle_data(data: &ParticleInstanceData, transform: Mat4) -> Self {
        Self {
            transform: transform.to_cols_array_2d(),
            color: data.color,
            size_rotation: [data.rotation_frame[0], data.rotation_frame[1]],
            texture_data: [data.rotation_frame[1], 0.0], // frame, unused
        }
    }
}

/// Culling support for particles
pub fn cull_particle_systems(
    emitters: &[ParticleEmitter],
    _camera: &CameraClass,
    frustum_planes: &[glam::Vec4; 6],
) -> Vec<usize> {
    let mut visible_indices = Vec::new();

    for (index, emitter) in emitters.iter().enumerate() {
        if let Some(ref buffer) = emitter.buffer {
            let sphere = buffer.get_bounding_sphere();

            // Simple sphere-frustum intersection test
            let mut visible = true;
            for plane in frustum_planes {
                let distance = plane.x * sphere.center.x
                    + plane.y * sphere.center.y
                    + plane.z * sphere.center.z
                    + plane.w;
                if distance < -sphere.radius {
                    visible = false;
                    break;
                }
            }

            if visible {
                visible_indices.push(index);
            }
        }
    }

    visible_indices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::{FrameMode, NewParticle, RenderMode};
    use crate::properties::{
        ParticleBlurTimeProperty, ParticleColorProperty, ParticleFrameProperty,
        ParticleOpacityProperty, ParticleRotationProperty, ParticleSizeProperty,
    };

    fn empty_test_buffer() -> ParticleBuffer {
        ParticleBuffer::new(
            4,
            ParticleColorProperty::with_start(Vec3::ONE),
            ParticleOpacityProperty::with_start(1.0),
            ParticleSizeProperty::with_start(1.0),
            ParticleRotationProperty::with_start(0.0),
            ParticleFrameProperty::with_start(0.0),
            ParticleBlurTimeProperty::with_start(0.0),
            Vec3::ZERO,
            1000,
            0,
            RenderMode::TriParticles,
            FrameMode::Frame1x1,
            None,
            None,
            None,
        )
    }

    fn test_buffer_at(position: Vec3) -> Arc<Mutex<ParticleBuffer>> {
        let mut buffer = empty_test_buffer();
        buffer.add_new_particle(NewParticle {
            position,
            ..Default::default()
        });
        buffer.update(0);
        Arc::new(Mutex::new(buffer))
    }

    fn inactive_test_buffer() -> Arc<Mutex<ParticleBuffer>> {
        Arc::new(Mutex::new(empty_test_buffer()))
    }

    #[test]
    fn batch_particles_prunes_empty_buffers_when_instancing_enabled() {
        let inactive = inactive_test_buffer();
        let active = test_buffer_at(Vec3::ZERO);

        let mut render_pass = ParticleRenderPass::new();
        render_pass.add_buffer(inactive, BlendMode::Alpha);
        render_pass.add_buffer(active.clone(), BlendMode::Alpha);

        render_pass.batch_particles();

        assert_eq!(render_pass.transparent_buffers.len(), 1);
        assert!(Arc::ptr_eq(&render_pass.transparent_buffers[0], &active));
    }

    #[test]
    fn batch_particles_preserves_buffers_when_instancing_disabled() {
        let inactive = inactive_test_buffer();
        let active = test_buffer_at(Vec3::ZERO);

        let mut render_pass = ParticleRenderPass::new();
        render_pass.instancing_enabled = false;
        render_pass.add_buffer(inactive.clone(), BlendMode::Opaque);
        render_pass.add_buffer(active.clone(), BlendMode::Opaque);

        render_pass.batch_particles();

        assert_eq!(render_pass.opaque_buffers.len(), 2);
        assert!(Arc::ptr_eq(&render_pass.opaque_buffers[0], &inactive));
        assert!(Arc::ptr_eq(&render_pass.opaque_buffers[1], &active));
    }

    #[test]
    fn test_particle_render_pass_creation() {
        let render_pass = ParticleRenderPass::new();
        assert_eq!(render_pass.opaque_buffers.len(), 0);
        assert_eq!(render_pass.transparent_buffers.len(), 0);
        assert_eq!(render_pass.additive_buffers.len(), 0);
        assert!(render_pass.instancing_enabled);
    }

    #[test]
    fn test_blend_mode_categorization() {
        let render_pass = ParticleRenderPass::new();

        // Would need actual particle buffer for full test
        // This is just a structure test
        assert_eq!(render_pass.opaque_buffers.len(), 0);
    }

    #[test]
    fn scene_collects_particle_buffers_by_blend_mode() {
        let opaque = test_buffer_at(Vec3::ZERO);
        let alpha = test_buffer_at(Vec3::X);
        let additive = test_buffer_at(Vec3::Y);
        let mut scene = SceneClass::new();
        scene.add_particle_buffer(opaque.clone(), BlendMode::Opaque);
        scene.add_particle_buffer(alpha.clone(), BlendMode::Alpha);
        scene.add_particle_buffer(additive.clone(), BlendMode::Additive);

        let mut render_pass = ParticleRenderPass::new();
        render_pass.collect_scene_buffers(&scene);

        assert_eq!(render_pass.opaque_buffers.len(), 1);
        assert_eq!(render_pass.transparent_buffers.len(), 1);
        assert_eq!(render_pass.additive_buffers.len(), 1);
        assert!(Arc::ptr_eq(&render_pass.opaque_buffers[0], &opaque));
        assert!(Arc::ptr_eq(&render_pass.transparent_buffers[0], &alpha));
        assert!(Arc::ptr_eq(&render_pass.additive_buffers[0], &additive));
    }

    #[test]
    fn transparent_scene_buffers_sort_back_to_front_from_camera() {
        let near = test_buffer_at(Vec3::new(1.0, 0.0, 0.0));
        let far = test_buffer_at(Vec3::new(5.0, 0.0, 0.0));
        let mut scene = SceneClass::new();
        scene.add_particle_buffer(near.clone(), BlendMode::Alpha);
        scene.add_particle_buffer(far.clone(), BlendMode::Alpha);

        let mut render_pass = ParticleRenderPass::new();
        render_pass.collect_scene_buffers(&scene);
        render_pass.sort_transparent_particles(Vec3::ZERO);

        assert!(Arc::ptr_eq(&render_pass.transparent_buffers[0], &far));
        assert!(Arc::ptr_eq(&render_pass.transparent_buffers[1], &near));
    }

    #[test]
    fn camera_position_is_stateful() {
        let mut camera = CameraClass::new(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(camera.get_position(), Vec3::new(1.0, 2.0, 3.0));
        camera.set_position(Vec3::new(4.0, 5.0, 6.0));
        assert_eq!(camera.get_position(), Vec3::new(4.0, 5.0, 6.0));
    }
}
