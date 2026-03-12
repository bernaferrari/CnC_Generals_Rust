//! Particle System Manager
//!
//! This module provides a manager class that implements the ParticleSystem trait
//! for integration with the renderer scene system.

use super::buffer::{FrameMode, RenderMode};
use super::emitter::ParticleEmitter;
use super::properties::*;
use super::sorting_renderer::{BoundingSphere, SortingRenderer};
use glam::{Mat4, Vec3};
use std::sync::Arc;
use wgpu::{Device, Queue, RenderPass};

/// Particle system manager that implements the ParticleSystem trait
#[derive(Debug)]
pub struct ParticleSystemManager {
    pub emitters: Vec<ParticleEmitter>,
    pub sorting_renderer: SortingRenderer,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
}

impl ParticleSystemManager {
    /// Create a new particle system manager
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        let sorting_renderer = SortingRenderer::new(device.clone(), queue.clone());

        Self {
            emitters: Vec::new(),
            sorting_renderer,
            device,
            queue,
        }
    }

    /// Add an emitter to the system
    pub fn add_emitter(&mut self, emitter: ParticleEmitter) {
        self.emitters.push(emitter);
    }

    /// Remove an emitter by index
    pub fn remove_emitter(&mut self, index: usize) {
        if index < self.emitters.len() {
            self.emitters.remove(index);
        }
    }

    /// Get the number of emitters
    pub fn emitter_count(&self) -> usize {
        self.emitters.len()
    }

    /// Create a simple fire emitter
    pub fn create_fire_emitter() -> ParticleEmitter {
        let color_prop = ParticleColorProperty::with_keyframes(
            Vec3::new(1.0, 0.3, 0.0), // Orange start
            Vec3::new(0.2, 0.2, 0.2), // Random variation
            vec![0.0, 0.5, 1.0],      // Times
            vec![
                Vec3::new(1.0, 0.3, 0.0), // Start: Orange
                Vec3::new(1.0, 0.6, 0.0), // Middle: Yellow-orange
                Vec3::new(0.5, 0.5, 0.5), // End: Gray
            ],
        );

        let opacity_prop = ParticleOpacityProperty::with_keyframes(
            1.0,
            0.0,
            vec![0.0, 0.7, 1.0],
            vec![1.0, 0.8, 0.0],
        );

        let size_prop = ParticleSizeProperty::with_keyframes(
            0.5,
            0.2,
            vec![0.0, 0.5, 1.0],
            vec![0.5, 1.0, 0.1],
        );

        let rotation_prop = ParticleRotationProperty::with_start(0.0);
        let frame_prop = ParticleFrameProperty::with_start(0.0);
        let blur_time_prop = ParticleBlurTimeProperty::with_start(0.0);

        ParticleEmitter::new(
            50.0, // 50 particles per second
            2,    // burst size
            Some(super::emitter::Vec3Randomizer::new(
                Vec3::new(-0.5, -0.5, -0.5),
                Vec3::new(0.5, 0.5, 0.5),
            )), // position randomizer
            Vec3::new(0.0, 2.0, 0.0), // velocity up
            Some(super::emitter::Vec3Randomizer::new(
                Vec3::new(-0.5, -0.5, -0.5),
                Vec3::new(0.5, 0.5, 0.5),
            )), // velocity randomizer
            0.0,  // outward velocity
            0.0,  // velocity inherit factor
            color_prop,
            opacity_prop,
            size_prop,
            rotation_prop,
            0.5, // orient random
            frame_prop,
            blur_time_prop,
            Vec3::new(0.0, -1.0, 0.0), // gravity
            3.0,                       // lifetime
            0.0,                       // future start
            RenderMode::TriParticles,
            FrameMode::Frame1x1,
            500,   // max particles
            1000,  // max buffer size
            false, // pingpong
        )
    }

    /// Create a smoke emitter
    pub fn create_smoke_emitter() -> ParticleEmitter {
        let color_prop = ParticleColorProperty::with_keyframes(
            Vec3::new(0.5, 0.5, 0.5), // Gray start
            Vec3::new(0.1, 0.1, 0.1), // Random variation
            vec![0.0, 0.8, 1.0],      // Times
            vec![
                Vec3::new(0.5, 0.5, 0.5), // Gray
                Vec3::new(0.7, 0.7, 0.7), // Light gray
                Vec3::new(0.9, 0.9, 0.9), // White
            ],
        );

        let opacity_prop = ParticleOpacityProperty::with_keyframes(
            0.3,
            0.1,
            vec![0.0, 0.9, 1.0],
            vec![0.3, 0.2, 0.0],
        );

        let size_prop = ParticleSizeProperty::with_keyframes(
            2.0,
            0.5,
            vec![0.0, 0.5, 1.0],
            vec![2.0, 4.0, 1.0],
        );

        let rotation_prop = ParticleRotationProperty::with_start(0.0);
        let frame_prop = ParticleFrameProperty::with_start(0.0);
        let blur_time_prop = ParticleBlurTimeProperty::with_start(0.0);

        ParticleEmitter::new(
            20.0,                     // 20 particles per second
            1,                        // burst size
            None,                     // position randomizer
            Vec3::new(0.0, 3.0, 0.0), // velocity up
            None,                     // velocity randomizer
            0.0,                      // outward velocity
            0.0,                      // velocity inherit factor
            color_prop,
            opacity_prop,
            size_prop,
            rotation_prop,
            0.0, // orient random
            frame_prop,
            blur_time_prop,
            Vec3::new(0.0, 0.5, 0.0), // light gravity
            5.0,                      // lifetime
            0.0,                      // future start
            RenderMode::TriParticles,
            FrameMode::Frame1x1,
            200,   // max particles
            500,   // max buffer size
            false, // pingpong
        )
    }
}

impl super::ParticleSystem for ParticleSystemManager {
    fn update(&mut self, delta_time_ms: u32) {
        // Update particle emitters and their buffers
        for emitter in &mut self.emitters {
            emitter.update(delta_time_ms);
            if let Some(buffer) = emitter.get_buffer_mut() {
                buffer.update(delta_time_ms);
            }
        }

        // Remove completed emitters if they have auto-remove enabled
        self.emitters.retain(|emitter| {
            if emitter.is_complete() && emitter.is_remove_on_complete_enabled() {
                false
            } else {
                true
            }
        });
    }

    fn emit(&mut self, transform: Mat4) {
        for emitter in &mut self.emitters {
            emitter.set_transform(transform);
        }
    }

    fn render<'pass>(&'pass mut self, _render_pass: &mut RenderPass<'pass>) {
        // Collect particle geometry for sorting
        self.collect_particle_geometry();

        // Flush sorted particle geometry
        self.sorting_renderer.flush();
    }

    fn active_particle_count(&self) -> usize {
        self.emitters
            .iter()
            .filter_map(|emitter| emitter.get_buffer().map(|buffer| buffer.get_active_count()))
            .sum()
    }

    fn sorting_enabled(&self) -> bool {
        self.sorting_renderer.is_triangle_draw_enabled()
    }

    fn enable_sorting(&mut self, enable: bool) {
        self.sorting_renderer.enable_triangle_draw(enable);
    }

    fn sorting_stats(&self) -> (usize, usize, usize) {
        (
            self.sorting_renderer.get_sorted_node_count(),
            self.sorting_renderer.get_total_polygon_count(),
            self.sorting_renderer.get_total_vertex_count(),
        )
    }
}

impl ParticleSystemManager {
    /// Collect particle geometry and submit to sorting renderer
    fn collect_particle_geometry(&mut self) {
        // For now, use identity matrix for view - in practice this would come from the camera
        let world_view_matrix = Mat4::IDENTITY;

        for emitter in &mut self.emitters {
            if let Some(buffer) = emitter.get_buffer() {
                if buffer.get_active_count() > 0 {
                    // Create a bounding sphere for the particle system
                    let bounding_sphere = BoundingSphere::new(
                        emitter.prev_origin,
                        10.0, // Conservative radius
                    );

                    // Insert triangles into sorting renderer
                    // This is a simplified version - in practice you'd need actual geometry data
                    self.sorting_renderer.insert_triangles(
                        bounding_sphere,
                        0,                                      // start_index
                        buffer.get_active_count() as u16,       // polygon_count (simplified)
                        0,                                      // min_vertex_index
                        (buffer.get_active_count() * 3) as u16, // vertex_count (3 verts per particle triangle)
                        &world_view_matrix,
                    );
                }
            }
        }
    }
}
