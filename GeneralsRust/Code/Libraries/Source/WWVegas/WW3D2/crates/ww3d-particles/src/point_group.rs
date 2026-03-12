//! Point Group Implementation
//!
//! This module implements the PointGroupClass for rendering particles
//! as points, triangles, or quads with GPU instanced rendering.

use glam::{Mat4, Vec2, Vec3, Vec4};
use std::sync::Arc;
use wgpu::{util::DeviceExt, BindGroup, Buffer, Device, Queue, RenderPass, RenderPipeline};

/// Point group for rendering particles with GPU instancing
#[derive(Debug)]
pub struct PointGroup {
    // Rendering state
    pub point_mode: PointMode,
    pub default_point_size: f32,
    pub default_point_color: Vec3,
    pub default_point_alpha: f32,
    pub default_point_orientation: u8,
    pub default_point_frame: u8,
    pub frame_row_column_count_log2: u8,
    pub billboard: bool,

    // GPU resources
    pub vertex_buffer: Option<Buffer>,
    pub index_buffer: Option<Buffer>,
    pub instance_buffer: Option<Buffer>,
    pub bind_group: Option<BindGroup>,
    pub render_pipeline: Option<RenderPipeline>,

    // Particle data
    pub positions: Vec<Vec3>,
    pub colors: Vec<Vec4>,
    pub sizes: Vec<f32>,
    pub orientations: Vec<u8>,
    pub frames: Vec<u8>,
    pub active_count: usize,

    // Device references
    device: Arc<Device>,
    queue: Arc<Queue>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointMode {
    Triangles,
    Quads,
    Screenspace,
}

#[derive(Debug, Clone, Copy)]
pub enum PointFlags {
    Transform = 1,
}

/// Instance data for GPU instancing
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleInstanceData {
    pub position: [f32; 4],       // xyz + size
    pub color: [f32; 4],          // rgba
    pub rotation_frame: [f32; 4], // rotation, frame, unused, unused
}

impl Default for ParticleInstanceData {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
            rotation_frame: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// Sortable particle for depth-based rendering
#[derive(Debug, Clone)]
struct SortableParticle {
    instance_data: ParticleInstanceData,
    depth: f32,
}

impl PartialEq for SortableParticle {
    fn eq(&self, other: &Self) -> bool {
        self.depth.total_cmp(&other.depth).is_eq()
    }
}

impl PartialOrd for SortableParticle {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.depth.total_cmp(&other.depth))
    }
}

/// Vertex data for particle rendering
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleVertex {
    pub position: Vec2,
    pub tex_coord: Vec2,
}

impl Default for ParticleVertex {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            tex_coord: Vec2::ZERO,
        }
    }
}

impl PointGroup {
    /// Create a new point group
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self {
            point_mode: PointMode::Triangles,
            default_point_size: 1.0,
            default_point_color: Vec3::ONE,
            default_point_alpha: 1.0,
            default_point_orientation: 0,
            default_point_frame: 0,
            frame_row_column_count_log2: 0,
            billboard: true,
            vertex_buffer: None,
            index_buffer: None,
            instance_buffer: None,
            bind_group: None,
            render_pipeline: None,
            positions: Vec::new(),
            colors: Vec::new(),
            sizes: Vec::new(),
            orientations: Vec::new(),
            frames: Vec::new(),
            active_count: 0,
            device,
            queue,
        }
    }

    /// Update instance buffer with external data
    pub fn update_instance_buffer(
        &mut self,
        device: &Device,
        queue: &Queue,
        instance_data: &[ParticleInstanceData],
    ) {
        if instance_data.is_empty() {
            return;
        }

        // Create or update instance buffer
        let buffer_size = std::mem::size_of_val(instance_data);

        if let Some(ref instance_buffer) = self.instance_buffer {
            // Check if we need a larger buffer
            if instance_buffer.size() < buffer_size as u64 {
                self.instance_buffer = Some(device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor {
                        label: Some("Particle Instance Buffer"),
                        contents: bytemuck::cast_slice(instance_data),
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    },
                ));
            } else {
                // Update existing buffer
                queue.write_buffer(instance_buffer, 0, bytemuck::cast_slice(instance_data));
            }
        } else {
            // Create new buffer
            self.instance_buffer = Some(device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Particle Instance Buffer"),
                    contents: bytemuck::cast_slice(instance_data),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                },
            ));
        }

        self.active_count = instance_data.len();
    }

    /// Update instance buffer with sorted particle data for depth transparency
    pub fn update_instance_buffer_sorted(&mut self, view_matrix: &Mat4) {
        if self.positions.is_empty() {
            return;
        }

        // Transform all positions to view space once
        let view_space_positions: Vec<Vec3> = self
            .positions
            .iter()
            .map(|pos| {
                let pos_4d = *view_matrix * Vec4::new(pos.x, pos.y, pos.z, 1.0);
                Vec3::new(pos_4d.x, pos_4d.y, pos_4d.z)
            })
            .collect();

        // Create sortable particles using pre-transformed Z depth
        let mut sortable_particles: Vec<SortableParticle> = view_space_positions
            .iter()
            .zip(&self.positions)
            .zip(&self.colors)
            .zip(&self.sizes)
            .zip(&self.orientations)
            .zip(&self.frames)
            .enumerate()
            .map(
                |(_i, (((((view_pos, pos), color), size), orientation), frame))| {
                    let depth = view_pos.z;
                    SortableParticle {
                        instance_data: ParticleInstanceData {
                            position: [pos.x, pos.y, pos.z, *size],
                            color: [color.x, color.y, color.z, color.w],
                            rotation_frame: [*orientation as f32, *frame as f32, 0.0, depth],
                        },
                        depth,
                    }
                },
            )
            .collect();

        // Sort by depth (back to front for transparency)
        sortable_particles.sort_by(|a, b| {
            b.depth
                .partial_cmp(&a.depth)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Extract instance data
        let instance_data: Vec<ParticleInstanceData> = sortable_particles
            .into_iter()
            .map(|sp| sp.instance_data)
            .collect();

        // Update buffer
        let device = Arc::clone(&self.device);
        let queue = Arc::clone(&self.queue);
        self.update_instance_buffer(&device, &queue, &instance_data);
    }

    /// Update GPU buffers with current particle data
    fn update_gpu_buffers(&mut self) {
        if self.positions.is_empty() {
            return;
        }

        // Create instance data from particle vectors
        let instance_data: Vec<ParticleInstanceData> = self
            .positions
            .iter()
            .zip(&self.colors)
            .zip(&self.sizes)
            .zip(&self.orientations)
            .zip(&self.frames)
            .map(
                |((((pos, color), size), orientation), frame)| ParticleInstanceData {
                    position: [pos.x, pos.y, pos.z, *size],
                    color: [color.x, color.y, color.z, color.w],
                    rotation_frame: [*orientation as f32, *frame as f32, 0.0, 0.0],
                },
            )
            .collect();

        let device = Arc::clone(&self.device);
        let queue = Arc::clone(&self.queue);
        self.update_instance_buffer(&device, &queue, &instance_data);
    }

    /// Update vertex data based on point mode
    fn update_vertex_data(&mut self) {
        let (vertices, indices) = match self.point_mode {
            PointMode::Triangles => Self::create_triangle_vertices(),
            PointMode::Quads => Self::create_quad_vertices(),
            PointMode::Screenspace => Self::create_screenspace_vertices(),
        };

        // Create vertex buffer
        if !vertices.is_empty() {
            self.vertex_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Particle Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        }

        // Create index buffer
        if !indices.is_empty() {
            self.index_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Particle Index Buffer"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                },
            ));
        }
    }

    /// Create triangle vertices for point rendering
    fn create_triangle_vertices() -> (Vec<ParticleVertex>, Vec<u16>) {
        let vertices = vec![
            ParticleVertex {
                position: Vec2::new(-0.5, -0.5),
                tex_coord: Vec2::new(0.0, 1.0),
            },
            ParticleVertex {
                position: Vec2::new(0.5, -0.5),
                tex_coord: Vec2::new(1.0, 1.0),
            },
            ParticleVertex {
                position: Vec2::new(0.0, 0.5),
                tex_coord: Vec2::new(0.5, 0.0),
            },
        ];
        let indices = vec![0, 1, 2];
        (vertices, indices)
    }

    /// Create quad vertices for point rendering
    fn create_quad_vertices() -> (Vec<ParticleVertex>, Vec<u16>) {
        let vertices = vec![
            ParticleVertex {
                position: Vec2::new(-0.5, -0.5),
                tex_coord: Vec2::new(0.0, 1.0),
            },
            ParticleVertex {
                position: Vec2::new(0.5, -0.5),
                tex_coord: Vec2::new(1.0, 1.0),
            },
            ParticleVertex {
                position: Vec2::new(0.5, 0.5),
                tex_coord: Vec2::new(1.0, 0.0),
            },
            ParticleVertex {
                position: Vec2::new(-0.5, 0.5),
                tex_coord: Vec2::new(0.0, 0.0),
            },
        ];
        let indices = vec![0, 1, 2, 2, 3, 0];
        (vertices, indices)
    }

    /// Create screenspace vertices for point rendering
    fn create_screenspace_vertices() -> (Vec<ParticleVertex>, Vec<u16>) {
        // Simple point for screenspace rendering
        let vertices = vec![ParticleVertex {
            position: Vec2::ZERO,
            tex_coord: Vec2::new(0.5, 0.5),
        }];
        let indices = vec![0];
        (vertices, indices)
    }

    /// Render particles using GPU instancing
    pub fn render_instanced(&self, render_pass: &mut RenderPass<'_>) {
        if self.active_count == 0 || self.vertex_buffer.is_none() || self.instance_buffer.is_none()
        {
            return;
        }

        // Set pipeline
        if let Some(ref pipeline) = self.render_pipeline {
            render_pass.set_pipeline(pipeline);
        }

        // Bind resources
        if let Some(ref bind_group) = self.bind_group {
            render_pass.set_bind_group(0, bind_group, &[]);
        }

        // Set vertex buffer
        if let Some(ref vertex_buffer) = self.vertex_buffer {
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        }

        // Set instance buffer
        if let Some(ref instance_buffer) = self.instance_buffer {
            render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
        }

        // Draw instances
        match self.point_mode {
            PointMode::Triangles => {
                if let Some(ref index_buffer) = self.index_buffer {
                    render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                    render_pass.draw_indexed(0..3, 0, 0..self.active_count as u32);
                } else {
                    render_pass.draw(0..3, 0..self.active_count as u32);
                }
            }
            PointMode::Quads => {
                if let Some(ref index_buffer) = self.index_buffer {
                    render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                    render_pass.draw_indexed(0..6, 0, 0..self.active_count as u32);
                } else {
                    render_pass.draw(0..6, 0..self.active_count as u32);
                }
            }
            PointMode::Screenspace => {
                render_pass.draw(0..1, 0..self.active_count as u32);
            }
        }
    }

    /// Render particles (compatibility method)
    pub fn render(
        &self,
        _device: &Device,
        _queue: &Queue,
        _encoder: &mut wgpu::CommandEncoder,
        render_pass: &mut RenderPass<'_>,
        _view_projection_matrix: &Mat4,
    ) {
        self.render_instanced(render_pass);
    }

    /// Get polygon count for rendering
    pub fn get_polygon_count(&self) -> usize {
        match self.point_mode {
            PointMode::Triangles | PointMode::Screenspace => self.active_count,
            PointMode::Quads => self.active_count * 2,
        }
    }

    /// Add a single point to the particle system
    pub fn add_point(
        &mut self,
        position: Vec3,
        color: Vec4,
        size: f32,
        orientation: u8,
        frame: u8,
    ) {
        self.positions.push(position);
        self.colors.push(color);
        self.sizes.push(size);
        self.orientations.push(orientation);
        self.frames.push(frame);
        self.active_count += 1;
    }

    /// Clear all particles
    pub fn clear(&mut self) {
        self.positions.clear();
        self.colors.clear();
        self.sizes.clear();
        self.orientations.clear();
        self.frames.clear();
        self.active_count = 0;
    }

    /// Get the number of active particles
    pub fn particle_count(&self) -> usize {
        self.active_count
    }

    /// Initialize GPU resources for rendering
    pub fn init_gpu_resources(&mut self) {
        self.update_vertex_data();
        if !self.positions.is_empty() {
            self.update_gpu_buffers();
        }
    }
}
