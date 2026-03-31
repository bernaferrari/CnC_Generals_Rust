//! Streak System Implementation
//!
//! This module implements the streak system for rendering thick segmented lines
//! with varying properties like width, color, and texture mapping.

use glam::{Vec2, Vec3, Vec4};
use std::sync::Arc;
use wgpu::{util::DeviceExt, BindGroup, Buffer, Device, Queue, RenderPass, RenderPipeline};

/// Streak line class for rendering thick segmented lines
#[derive(Debug)]
#[allow(dead_code)] // C++ parity
pub struct StreakLine {
    // Line properties
    pub points: Vec<Vec3>,
    pub colors: Vec<Vec4>,
    pub widths: Vec<f32>,
    pub texture: Option<wgpu::Texture>,
    pub shader: wgpu::ShaderModule,

    // Rendering parameters
    pub width: f32,
    pub color: Vec3,
    pub opacity: f32,
    pub noise_amplitude: f32,
    pub merge_abort_factor: f32,
    pub subdivision_levels: u32,
    pub texture_mapping_mode: TextureMappingMode,
    pub texture_tile_factor: f32,
    pub uv_offset_rate: Vec2,
    pub merge_intersections: bool,
    pub freeze_random: bool,
    pub sorting_disabled: bool,
    pub end_caps_enabled: bool,

    // GPU resources
    pub vertex_buffer: Option<Buffer>,
    pub index_buffer: Option<Buffer>,
    pub bind_group: Option<BindGroup>,
    pub render_pipeline: Option<RenderPipeline>,

    // Device references
    device: Arc<Device>,
    queue: Arc<Queue>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextureMappingMode {
    Linear,
    Tiled,
    Stretch,
}

impl StreakLine {
    /// Create a new streak line
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self {
            points: Vec::new(),
            colors: Vec::new(),
            widths: Vec::new(),
            texture: None,
            shader: Self::create_default_shader(&device),
            width: 1.0,
            color: Vec3::ONE,
            opacity: 1.0,
            noise_amplitude: 0.0,
            merge_abort_factor: 0.0,
            subdivision_levels: 0,
            texture_mapping_mode: TextureMappingMode::Linear,
            texture_tile_factor: 1.0,
            uv_offset_rate: Vec2::ZERO,
            merge_intersections: false,
            freeze_random: false,
            sorting_disabled: false,
            end_caps_enabled: true,
            vertex_buffer: None,
            index_buffer: None,
            bind_group: None,
            render_pipeline: None,
            device,
            queue,
        }
    }

    /// Create a simple two-point line
    pub fn new_simple(
        start: Vec3,
        end: Vec3,
        start_color: Vec4,
        end_color: Vec4,
        width: f32,
        device: Arc<Device>,
        queue: Arc<Queue>,
    ) -> Self {
        let mut line = Self::new(device, queue);
        line.points = vec![start, end];
        line.colors = vec![start_color, end_color];
        line.widths = vec![width, width];
        line.width = width;
        line
    }

    /// Reset the line
    pub fn reset_line(&mut self) {
        self.points.clear();
        self.colors.clear();
        self.widths.clear();
    }

    /// Get number of points
    pub fn get_num_points(&self) -> usize {
        self.points.len()
    }

    /// Set point location
    pub fn set_point_location(&mut self, point_idx: usize, location: Vec3) {
        if point_idx < self.points.len() {
            self.points[point_idx] = location;
        }
    }

    /// Get point location
    pub fn get_point_location(&mut self, point_idx: usize) -> Vec3 {
        if point_idx < self.points.len() {
            self.points[point_idx]
        } else {
            Vec3::ZERO
        }
    }

    /// Add a point to the line
    pub fn add_point(&mut self, location: Vec3) {
        self.points.push(location);
        self.colors.push(Vec4::new(
            self.color.x,
            self.color.y,
            self.color.z,
            self.opacity,
        ));
        self.widths.push(self.width);
    }

    /// Delete a point from the line
    pub fn delete_point(&mut self, point_idx: usize) {
        if point_idx < self.points.len() {
            self.points.remove(point_idx);
            self.colors.remove(point_idx);
            self.widths.remove(point_idx);
        }
    }

    /// Set line width
    pub fn set_width(&mut self, width: f32) {
        self.width = width;
    }

    /// Get line width
    pub fn get_width(&self) -> f32 {
        self.width
    }

    /// Set line color
    pub fn set_color(&mut self, color: Vec3) {
        self.color = color;
    }

    /// Get line color
    pub fn get_color(&mut self) -> Vec3 {
        self.color
    }

    /// Set line opacity
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity;
    }

    /// Get line opacity
    pub fn get_opacity(&self) -> f32 {
        self.opacity
    }

    /// Set noise amplitude
    pub fn set_noise_amplitude(&mut self, amplitude: f32) {
        self.noise_amplitude = amplitude;
    }

    /// Get noise amplitude
    pub fn get_noise_amplitude(&self) -> f32 {
        self.noise_amplitude
    }

    /// Set merge abort factor
    pub fn set_merge_abort_factor(&mut self, factor: f32) {
        self.merge_abort_factor = factor;
    }

    /// Get merge abort factor
    pub fn get_merge_abort_factor(&self) -> f32 {
        self.merge_abort_factor
    }

    /// Set subdivision levels
    pub fn set_subdivision_levels(&mut self, levels: u32) {
        self.subdivision_levels = levels;
    }

    /// Get subdivision levels
    pub fn get_subdivision_levels(&self) -> u32 {
        self.subdivision_levels
    }

    /// Set texture mapping mode
    pub fn set_texture_mapping_mode(&mut self, mode: TextureMappingMode) {
        self.texture_mapping_mode = mode;
    }

    /// Get texture mapping mode
    pub fn get_texture_mapping_mode(&self) -> TextureMappingMode {
        self.texture_mapping_mode
    }

    /// Set texture tile factor
    pub fn set_texture_tile_factor(&mut self, factor: f32) {
        assert!(factor >= 0.0, "Texture tile factor must be non-negative");
        self.texture_tile_factor = factor;
    }

    /// Get texture tile factor
    pub fn get_texture_tile_factor(&self) -> f32 {
        self.texture_tile_factor
    }

    /// Set UV offset rate
    pub fn set_uv_offset_rate(&mut self, rate: Vec2) {
        self.uv_offset_rate = rate;
    }

    /// Get UV offset rate
    pub fn get_uv_offset_rate(&self) -> Vec2 {
        self.uv_offset_rate
    }

    /// Set merge intersections
    pub fn set_merge_intersections(&mut self, onoff: bool) {
        self.merge_intersections = onoff;
    }

    /// Get merge intersections
    pub fn get_merge_intersections(&self) -> bool {
        self.merge_intersections
    }

    /// Set freeze random
    pub fn set_freeze_random(&mut self, onoff: bool) {
        self.freeze_random = onoff;
    }

    /// Get freeze random
    pub fn get_freeze_random(&self) -> bool {
        self.freeze_random
    }

    /// Set disable sorting
    pub fn set_disable_sorting(&mut self, onoff: bool) {
        self.sorting_disabled = onoff;
    }

    /// Get sorting disabled
    pub fn get_sorting_disabled(&self) -> bool {
        self.sorting_disabled
    }

    /// Set end caps enabled
    pub fn set_end_caps(&mut self, onoff: bool) {
        self.end_caps_enabled = onoff;
    }

    /// Get end caps enabled
    pub fn get_end_caps_enabled(&self) -> bool {
        self.end_caps_enabled
    }

    /// Set texture
    pub fn set_texture(&mut self, texture: wgpu::Texture) {
        self.texture = Some(texture);
        // Recreate bind group with new texture
        self.update_bind_group();
    }

    /// Get texture
    pub fn get_texture(&self) -> Option<&wgpu::Texture> {
        self.texture.as_ref()
    }

    /// Set shader
    pub fn set_shader(&mut self, shader: wgpu::ShaderModule) {
        self.shader = shader;
        // Recreate pipeline with new shader
        self.update_render_pipeline();
    }

    /// Get shader
    pub fn get_shader(&self) -> &wgpu::ShaderModule {
        &self.shader
    }

    /// Set locations, widths, and colors for all points
    pub fn set_locs_widths_colors(
        &mut self,
        num_points: usize,
        locs: &[Vec3],
        widths: Option<&[f32]>,
        colors: Option<&[Vec4]>,
    ) {
        self.points.clear();
        self.colors.clear();
        self.widths.clear();

        for i in 0..num_points {
            self.points.push(locs[i]);
            self.widths.push(widths.map(|w| w[i]).unwrap_or(self.width));
            self.colors.push(colors.map(|c| c[i]).unwrap_or_else(|| {
                Vec4::new(self.color.x, self.color.y, self.color.z, self.opacity)
            }));
        }

        self.update_geometry_buffers();
    }

    /// Update GPU geometry buffers
    fn update_geometry_buffers(&mut self) {
        if self.points.len() < 2 {
            return;
        }

        // Generate line segments
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for i in 0..(self.points.len() - 1) {
            let start = self.points[i];
            let end = self.points[i + 1];
            let start_color = self.colors[i];
            let end_color = self.colors[i + 1];
            let start_width = self.widths[i];
            let end_width = self.widths[i + 1];

            // Create quad vertices for this segment
            let segment_vertices = self.create_segment_vertices(
                start,
                end,
                start_width,
                end_width,
                start_color,
                end_color,
                i,
            );
            let base_index = vertices.len() as u16;

            vertices.extend(segment_vertices);
            indices.extend_from_slice(&[
                base_index,
                base_index + 1,
                base_index + 2,
                base_index + 1,
                base_index + 3,
                base_index + 2,
            ]);
        }

        // Add end caps if enabled (C++ streak.cpp lines 200-400)
        if self.end_caps_enabled && !self.points.is_empty() {
            self.add_end_caps(&mut vertices, &mut indices);
        }

        // Create GPU buffers
        self.vertex_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Streak Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));

        self.index_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Streak Index Buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            },
        ));
    }

    /// Create vertices for a line segment
    fn create_segment_vertices(
        &self,
        start: Vec3,
        end: Vec3,
        start_width: f32,
        end_width: f32,
        start_color: Vec4,
        end_color: Vec4,
        segment_index: usize,
    ) -> [StreakVertex; 4] {
        let direction = (end - start).normalize();
        let perpendicular = Vec3::new(-direction.y, direction.x, direction.z);

        let half_start_width = start_width * 0.5;
        let half_end_width = end_width * 0.5;

        // Calculate UV coordinates based on texture mapping mode
        let u_start = match self.texture_mapping_mode {
            TextureMappingMode::Linear => segment_index as f32,
            TextureMappingMode::Tiled => segment_index as f32 * self.texture_tile_factor,
            TextureMappingMode::Stretch => segment_index as f32 / (self.points.len() - 1) as f32,
        };
        let u_end = match self.texture_mapping_mode {
            TextureMappingMode::Linear => (segment_index + 1) as f32,
            TextureMappingMode::Tiled => (segment_index + 1) as f32 * self.texture_tile_factor,
            TextureMappingMode::Stretch => {
                ((segment_index + 1) as f32) / (self.points.len() - 1) as f32
            }
        };

        [
            StreakVertex {
                position: start - perpendicular * half_start_width,
                color: start_color,
                uv: Vec2::new(u_start, 0.0),
            },
            StreakVertex {
                position: start + perpendicular * half_start_width,
                color: start_color,
                uv: Vec2::new(u_start, 1.0),
            },
            StreakVertex {
                position: end - perpendicular * half_end_width,
                color: end_color,
                uv: Vec2::new(u_end, 0.0),
            },
            StreakVertex {
                position: end + perpendicular * half_end_width,
                color: end_color,
                uv: Vec2::new(u_end, 1.0),
            },
        ]
    }

    /// Add end caps to streak (C++ streak.cpp end cap generation)
    fn add_end_caps(&self, vertices: &mut Vec<StreakVertex>, indices: &mut Vec<u16>) {
        if self.points.len() < 2 {
            return;
        }

        // Start cap (flat perpendicular disc)
        {
            let start = self.points[0];
            let next = self.points[1];
            let direction = (next - start).normalize();
            let perpendicular = Vec3::new(-direction.y, direction.x, direction.z);

            let start_width = self.widths[0];
            let start_color = self.colors[0];
            let half_width = start_width * 0.5;

            let base_idx = vertices.len() as u16;

            // Center vertex
            vertices.push(StreakVertex {
                position: start,
                color: start_color,
                uv: Vec2::new(0.0, 0.5),
            });

            // Create a fan of vertices around the perimeter
            let segments = 8;
            for i in 0..=segments {
                let angle = (i as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
                let cos_angle = angle.cos();
                let sin_angle = angle.sin();

                let offset = perpendicular * cos_angle * half_width
                    + direction.cross(perpendicular).normalize() * sin_angle * half_width;

                vertices.push(StreakVertex {
                    position: start + offset,
                    color: start_color,
                    uv: Vec2::new(0.0, (i as f32) / (segments as f32)),
                });

                if i > 0 {
                    indices.extend_from_slice(&[
                        base_idx,
                        base_idx + i as u16,
                        base_idx + i as u16 + 1,
                    ]);
                }
            }
        }

        // End cap (flat perpendicular disc)
        {
            let end_idx = self.points.len() - 1;
            let end = self.points[end_idx];
            let prev = self.points[end_idx - 1];
            let direction = (end - prev).normalize();
            let perpendicular = Vec3::new(-direction.y, direction.x, direction.z);

            let end_width = self.widths[end_idx];
            let end_color = self.colors[end_idx];
            let half_width = end_width * 0.5;

            let base_idx = vertices.len() as u16;

            // Center vertex
            vertices.push(StreakVertex {
                position: end,
                color: end_color,
                uv: Vec2::new(1.0, 0.5),
            });

            // Create a fan of vertices around the perimeter
            let segments = 8;
            for i in 0..=segments {
                let angle = (i as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
                let cos_angle = angle.cos();
                let sin_angle = angle.sin();

                let offset = perpendicular * cos_angle * half_width
                    + direction.cross(perpendicular).normalize() * sin_angle * half_width;

                vertices.push(StreakVertex {
                    position: end + offset,
                    color: end_color,
                    uv: Vec2::new(1.0, (i as f32) / (segments as f32)),
                });

                if i > 0 {
                    indices.extend_from_slice(&[
                        base_idx,
                        base_idx + i as u16,
                        base_idx + i as u16 + 1,
                    ]);
                }
            }
        }
    }

    /// Update bind group (C++ streak.cpp texture binding)
    fn update_bind_group(&mut self) {
        if self.texture.is_none() {
            return;
        }

        let texture = self.texture.as_ref().unwrap();

        // Create texture view
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create sampler
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Streak Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create bind group layout
        let bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Streak Bind Group Layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        // Create bind group
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Streak Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        self.bind_group = Some(bind_group);
    }

    /// Update render pipeline (C++ streak.cpp pipeline setup)
    fn update_render_pipeline(&mut self) {
        // Create bind group layout
        let bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Streak Pipeline Bind Group Layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        // Create pipeline layout
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Streak Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        // Create render pipeline
        let render_pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Streak Render Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<StreakVertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                            wgpu::VertexAttribute {
                                offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            wgpu::VertexAttribute {
                                offset: std::mem::size_of::<[f32; 7]>() as wgpu::BufferAddress,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                        ],
                    }],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8UnormSrgb,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        self.render_pipeline = Some(render_pipeline);
    }

    /// Create default shader
    fn create_default_shader(device: &Device) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Streak Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/streak.wgsl").into()),
        })
    }

    /// Render the streak line
    pub fn render(&self, render_pass: &mut RenderPass<'_>) {
        if self.points.len() < 2 || self.vertex_buffer.is_none() || self.index_buffer.is_none() {
            return;
        }

        // Set up render state
        render_pass.set_vertex_buffer(0, self.vertex_buffer.as_ref().unwrap().slice(..));
        render_pass.set_index_buffer(
            self.index_buffer.as_ref().unwrap().slice(..),
            wgpu::IndexFormat::Uint16,
        );

        // Draw all segments
        let index_count = ((self.points.len() - 1) * 6) as u32;
        render_pass.draw_indexed(0..index_count, 0, 0..1);
    }
}

/// Vertex data for streak rendering
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StreakVertex {
    pub position: Vec3,
    pub color: Vec4,
    pub uv: Vec2,
}

// Manual implementation of Pod and Zeroable
unsafe impl bytemuck::Pod for StreakVertex {}
unsafe impl bytemuck::Zeroable for StreakVertex {}

/// Segmented line renderer for managing multiple streak lines
#[derive(Debug)]
pub struct SegmentedLineRenderer {
    pub lines: Vec<StreakLine>,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
}

impl SegmentedLineRenderer {
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self {
            lines: Vec::new(),
            device,
            queue,
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn add_line(
        &mut self,
        start: Vec3,
        end: Vec3,
        start_color: Vec4,
        end_color: Vec4,
        width: f32,
    ) {
        // Create a simple line from start to end points
        let line = StreakLine::new_simple(
            start,
            end,
            start_color,
            end_color,
            width,
            self.device.clone(),
            self.queue.clone(),
        );
        self.lines.push(line);
    }

    pub fn add_streak_line(&mut self, line: StreakLine) {
        self.lines.push(line);
    }

    pub fn render(
        &mut self,
        _device: &Device,
        _queue: &Queue,
        _encoder: &mut wgpu::CommandEncoder,
        render_pass: &mut RenderPass<'_>,
        _view_projection_matrix: glam::Mat4,
    ) {
        for line in &mut self.lines {
            line.render(render_pass);
        }
    }
}

/// Line group renderer for rendering multiple lines as a group
#[derive(Debug)]
pub struct LineGroupRenderer {
    pub lines: Vec<StreakLine>,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
}

impl LineGroupRenderer {
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self {
            lines: Vec::new(),
            device,
            queue,
        }
    }

    pub fn add_line(&mut self, line: StreakLine) {
        self.lines.push(line);
    }

    pub fn render(&self, render_pass: &mut RenderPass<'_>) {
        for line in &self.lines {
            line.render(render_pass);
        }
    }
}
