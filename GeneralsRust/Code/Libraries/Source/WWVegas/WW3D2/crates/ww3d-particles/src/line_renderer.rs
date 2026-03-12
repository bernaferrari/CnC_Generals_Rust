//! Line Renderer Implementation
//!
//! This module implements the SegmentedLineRendererClass for rendering
//! thick segmented lines with various rendering options.

use glam::{Vec2, Vec3, Vec4};
use std::sync::Arc;
use wgpu::{util::DeviceExt, Device, Queue, RenderPass};

/// Segmented line renderer class
#[allow(dead_code)]
pub struct SegLineRendererClass {
    // Line properties
    pub width: f32,
    pub color: Vec3,
    pub opacity: f32,
    pub noise_amplitude: f32,
    pub merge_abort_factor: f32,
    pub texture_mapping_mode: TextureMappingMode,
    pub texture_tile_factor: f32,
    pub uv_offset_rate: Vec2,
    pub subdivision_levels: u32,
    pub merge_intersections: bool,
    pub freeze_random: bool,
    pub sorting_disabled: bool,
    pub end_caps_enabled: bool,

    // Texture and shader
    pub texture: Option<wgpu::Texture>,
    pub shader: wgpu::ShaderModule,

    // GPU resources
    pub vertex_buffer: Option<wgpu::Buffer>,
    pub index_buffer: Option<wgpu::Buffer>,
    pub bind_group: Option<wgpu::BindGroup>,
    pub render_pipeline: Option<wgpu::RenderPipeline>,

    // Device references
    device: Arc<Device>,
    _queue: Arc<Queue>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextureMappingMode {
    Linear = 0,
    Tiled = 1,
    Perspective = 2,
    Distance = 3,
}

impl SegLineRendererClass {
    /// Create a new segmented line renderer
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self {
            width: 1.0,
            color: Vec3::ONE,
            opacity: 1.0,
            noise_amplitude: 0.0,
            merge_abort_factor: 0.0,
            texture_mapping_mode: TextureMappingMode::Linear,
            texture_tile_factor: 1.0,
            uv_offset_rate: Vec2::ZERO,
            subdivision_levels: 0,
            merge_intersections: false,
            freeze_random: false,
            sorting_disabled: false,
            end_caps_enabled: true,
            texture: None,
            shader: Self::create_default_shader(&device),
            vertex_buffer: None,
            index_buffer: None,
            bind_group: None,
            render_pipeline: None,
            device,
            _queue: queue,
        }
    }

    /// Set width
    pub fn set_width(&mut self, width: f32) {
        self.width = width;
    }

    /// Get width
    pub fn get_width(&self) -> f32 {
        self.width
    }

    /// Set color
    pub fn set_color(&mut self, color: Vec3) {
        self.color = color;
    }

    /// Get color
    pub fn get_color(&mut self) -> Vec3 {
        self.color
    }

    /// Set opacity
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity;
    }

    /// Get opacity
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

    /// Set subdivision levels
    pub fn set_subdivision_levels(&mut self, levels: u32) {
        self.subdivision_levels = levels;
    }

    /// Get subdivision levels
    pub fn get_subdivision_levels(&self) -> u32 {
        self.subdivision_levels
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

    /// Set sorting disabled
    pub fn set_sorting_disabled(&mut self, onoff: bool) {
        self.sorting_disabled = onoff;
    }

    /// Get sorting disabled
    pub fn get_sorting_disabled(&self) -> bool {
        self.sorting_disabled
    }

    /// Set end caps enabled
    pub fn set_end_caps_enabled(&mut self, onoff: bool) {
        self.end_caps_enabled = onoff;
    }

    /// Get end caps enabled
    pub fn get_end_caps_enabled(&self) -> bool {
        self.end_caps_enabled
    }

    /// Set texture
    pub fn set_texture(&mut self, texture: wgpu::Texture) {
        self.texture = Some(texture);
        self.update_bind_group();
    }

    /// Get texture
    pub fn get_texture(&self) -> Option<&wgpu::Texture> {
        self.texture.as_ref()
    }

    /// Set shader
    pub fn set_shader(&mut self, shader: wgpu::ShaderModule) {
        self.shader = shader;
        self.update_render_pipeline();
    }

    /// Get shader
    pub fn get_shader(&self) -> &wgpu::ShaderModule {
        &self.shader
    }

    /// Render lines
    pub fn render(
        &mut self,
        render_pass: &mut RenderPass<'_>,
        points: &[Vec3],
        colors: Option<&[Vec4]>,
        widths: Option<&[f32]>,
    ) {
        if points.len() < 2 {
            return;
        }

        // Generate geometry for the line
        self.generate_line_geometry(points, colors, widths);

        if self.vertex_buffer.is_none() || self.index_buffer.is_none() {
            return;
        }

        // Set up render state
        render_pass.set_vertex_buffer(0, self.vertex_buffer.as_ref().unwrap().slice(..));
        render_pass.set_index_buffer(
            self.index_buffer.as_ref().unwrap().slice(..),
            wgpu::IndexFormat::Uint16,
        );

        // Draw the line
        let index_count = ((points.len() - 1) * 6) as u32; // 6 indices per segment (2 triangles)
        render_pass.draw_indexed(0..index_count, 0, 0..1);
    }

    /// Generate line geometry from points
    fn generate_line_geometry(
        &mut self,
        points: &[Vec3],
        colors: Option<&[Vec4]>,
        widths: Option<&[f32]>,
    ) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for i in 0..(points.len() - 1) {
            let start = points[i];
            let end = points[i + 1];

            let start_color = colors.map(|c| c[i]).unwrap_or(Vec4::new(
                self.color.x,
                self.color.y,
                self.color.z,
                self.opacity,
            ));
            let end_color = colors.map(|c| c[i + 1]).unwrap_or(Vec4::new(
                self.color.x,
                self.color.y,
                self.color.z,
                self.opacity,
            ));

            let start_width = widths.map(|w| w[i]).unwrap_or(self.width);
            let end_width = widths.map(|w| w[i + 1]).unwrap_or(self.width);

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

        // Create GPU buffers
        self.vertex_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Line Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));

        self.index_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Line Index Buffer"),
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
    ) -> [LineVertex; 4] {
        let direction = (end - start).normalize();
        let perpendicular = Vec3::new(-direction.y, direction.x, direction.z);

        let half_start_width = start_width * 0.5;
        let half_end_width = end_width * 0.5;

        // Calculate UV coordinates based on texture mapping mode
        let (u_start, u_end) = match self.texture_mapping_mode {
            TextureMappingMode::Linear => (segment_index as f32, (segment_index + 1) as f32),
            TextureMappingMode::Tiled => (
                segment_index as f32 * self.texture_tile_factor,
                (segment_index + 1) as f32 * self.texture_tile_factor,
            ),
            TextureMappingMode::Perspective => {
                // Perspective-correct mapping would require more complex calculation
                (segment_index as f32, (segment_index + 1) as f32)
            }
            TextureMappingMode::Distance => {
                let distance = (end - start).length();
                (
                    segment_index as f32 * distance,
                    (segment_index + 1) as f32 * distance,
                )
            }
        };

        [
            LineVertex {
                position: start - perpendicular * half_start_width,
                color: start_color,
                uv: Vec2::new(u_start, 0.0),
            },
            LineVertex {
                position: start + perpendicular * half_start_width,
                color: start_color,
                uv: Vec2::new(u_start, 1.0),
            },
            LineVertex {
                position: end - perpendicular * half_end_width,
                color: end_color,
                uv: Vec2::new(u_end, 0.0),
            },
            LineVertex {
                position: end + perpendicular * half_end_width,
                color: end_color,
                uv: Vec2::new(u_end, 1.0),
            },
        ]
    }

    /// Update bind group with current resources (C++ linerender.cpp lines 150-200)
    fn update_bind_group(&mut self) {
        if self.texture.is_none() {
            return;
        }

        let texture = self.texture.as_ref().unwrap();

        // Create texture view
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create sampler
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Line Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
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
                    label: Some("Line Bind Group Layout"),
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
            label: Some("Line Bind Group"),
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

    /// Update render pipeline with current shader (C++ linerender.cpp lines 200-300)
    fn update_render_pipeline(&mut self) {
        // Create bind group layout
        let bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Line Pipeline Bind Group Layout"),
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
                label: Some("Line Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        // Create render pipeline
        let render_pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Line Render Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<LineVertex>() as wgpu::BufferAddress,
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

    /// Create default shader for line rendering
    fn create_default_shader(device: &Device) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Line Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/line.wgsl").into()),
        })
    }
}

/// Vertex data for line rendering
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LineVertex {
    pub position: Vec3,
    pub color: Vec4,
    pub uv: Vec2,
}

// Manual implementation of Pod and Zeroable
unsafe impl bytemuck::Pod for LineVertex {}
unsafe impl bytemuck::Zeroable for LineVertex {}

/// Line group renderer - renders groups of lines as 3D shapes (tetrahedrons or prisms)
/// Used for motion-blurred particle systems (lightning bolts, streaks, etc.)
#[derive(Debug)]
pub struct LineGroupRenderer {
    pub line_mode: LineMode,
    pub transform_enabled: bool,

    // Default rendering properties
    pub default_line_size: f32,
    pub default_line_color: Vec3,
    pub default_line_alpha: f32,
    pub default_line_ucoord: f32,
    pub default_tail_diffuse: Vec4,

    // Texture and shader
    pub texture: Option<wgpu::Texture>,
    pub shader: wgpu::ShaderModule,

    // GPU resources
    pub vertex_buffer: Option<wgpu::Buffer>,
    pub index_buffer: Option<wgpu::Buffer>,
    pub bind_group: Option<wgpu::BindGroup>,
    pub render_pipeline: Option<wgpu::RenderPipeline>,

    // Device references
    device: Arc<Device>,
    _queue: Arc<Queue>,
}

/// Line rendering mode for line groups
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineMode {
    /// Tetrahedron - 4 vertices, 4 triangles per line
    Tetrahedron = 0,
    /// Prism - 6 vertices, 8 triangles per line
    Prism = 1,
}

impl LineGroupRenderer {
    /// Create a new line group renderer
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self {
            line_mode: LineMode::Tetrahedron,
            transform_enabled: true,
            default_line_size: 1.0,
            default_line_color: Vec3::ONE,
            default_line_alpha: 1.0,
            default_line_ucoord: 0.0,
            default_tail_diffuse: Vec4::ZERO,
            texture: None,
            shader: Self::create_default_shader(&device),
            vertex_buffer: None,
            index_buffer: None,
            bind_group: None,
            render_pipeline: None,
            device,
            _queue: queue,
        }
    }

    /// Set line mode (Tetrahedron or Prism)
    pub fn set_line_mode(&mut self, mode: LineMode) {
        self.line_mode = mode;
    }

    /// Get line mode
    pub fn get_line_mode(&self) -> LineMode {
        self.line_mode
    }

    /// Set default line size
    pub fn set_line_size(&mut self, size: f32) {
        self.default_line_size = size;
    }

    /// Get default line size
    pub fn get_line_size(&self) -> f32 {
        self.default_line_size
    }

    /// Set default line color
    pub fn set_line_color(&mut self, color: Vec3) {
        self.default_line_color = color;
    }

    /// Get default line color
    pub fn get_line_color(&self) -> Vec3 {
        self.default_line_color
    }

    /// Set default line alpha
    pub fn set_line_alpha(&mut self, alpha: f32) {
        self.default_line_alpha = alpha;
    }

    /// Get default line alpha
    pub fn get_line_alpha(&self) -> f32 {
        self.default_line_alpha
    }

    /// Set default tail diffuse color
    pub fn set_tail_diffuse(&mut self, diffuse: Vec4) {
        self.default_tail_diffuse = diffuse;
    }

    /// Get default tail diffuse color
    pub fn get_tail_diffuse(&self) -> Vec4 {
        self.default_tail_diffuse
    }

    /// Set texture
    pub fn set_texture(&mut self, texture: wgpu::Texture) {
        self.texture = Some(texture);
    }

    /// Render line groups
    /// start_locs: Start positions of lines
    /// end_locs: End positions of lines (apex for tetrahedron, end cap for prism)
    /// diffuse: Optional per-line diffuse colors (head)
    /// tail_diffuse: Optional per-line tail diffuse colors
    /// sizes: Optional per-line sizes
    /// ucoords: Optional per-line U coordinates
    pub fn render(
        &mut self,
        render_pass: &mut RenderPass<'_>,
        start_locs: &[Vec3],
        end_locs: &[Vec3],
        diffuse: Option<&[Vec4]>,
        tail_diffuse: Option<&[Vec4]>,
        sizes: Option<&[f32]>,
        ucoords: Option<&[f32]>,
        camera_up: Vec3,
        camera_right: Vec3,
    ) {
        if start_locs.len() != end_locs.len() || start_locs.is_empty() {
            return;
        }

        let _line_count = start_locs.len();

        // Generate geometry based on mode
        let (vertices, indices) = match self.line_mode {
            LineMode::Tetrahedron => self.generate_tetrahedron_geometry(
                start_locs,
                end_locs,
                diffuse,
                tail_diffuse,
                sizes,
                ucoords,
                camera_up,
                camera_right,
            ),
            LineMode::Prism => self.generate_prism_geometry(
                start_locs,
                end_locs,
                diffuse,
                tail_diffuse,
                sizes,
                ucoords,
                camera_up,
                camera_right,
            ),
        };

        if vertices.is_empty() || indices.is_empty() {
            return;
        }

        // Create GPU buffers
        self.vertex_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("LineGroup Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));

        self.index_buffer = Some(self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("LineGroup Index Buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            },
        ));

        // Set up render state and draw
        if let (Some(vb), Some(ib)) = (&self.vertex_buffer, &self.index_buffer) {
            render_pass.set_vertex_buffer(0, vb.slice(..));
            render_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }
    }

    /// Generate tetrahedron geometry
    /// Each line becomes a tetrahedron with apex at end_loc and base at start_loc
    fn generate_tetrahedron_geometry(
        &self,
        start_locs: &[Vec3],
        end_locs: &[Vec3],
        diffuse: Option<&[Vec4]>,
        tail_diffuse: Option<&[Vec4]>,
        sizes: Option<&[f32]>,
        ucoords: Option<&[f32]>,
        camera_up: Vec3,
        camera_right: Vec3,
    ) -> (Vec<LineGroupVertex>, Vec<u16>) {
        let line_count = start_locs.len();
        let mut vertices = Vec::with_capacity(line_count * 4);
        let mut indices = Vec::with_capacity(line_count * 12); // 4 triangles * 3 indices

        // Calculate offset vectors for base triangle (120 degrees apart)
        let offset_a = camera_up;
        let offset_b = camera_up * -0.5 + camera_right * (3.0_f32.sqrt() / 2.0);
        let offset_c = camera_up * -0.5 - camera_right * (3.0_f32.sqrt() / 2.0);

        for i in 0..line_count {
            let start = start_locs[i];
            let end = end_locs[i];
            let size = sizes.map(|s| s[i]).unwrap_or(self.default_line_size);
            let head_color = diffuse.map(|d| d[i]).unwrap_or(Vec4::new(
                self.default_line_color.x,
                self.default_line_color.y,
                self.default_line_color.z,
                self.default_line_alpha,
            ));
            let tail_color = tail_diffuse
                .map(|td| td[i])
                .unwrap_or(self.default_tail_diffuse);
            let ucoord = ucoords.map(|u| u[i]).unwrap_or(self.default_line_ucoord);

            let base_idx = vertices.len() as u16;

            // Apex vertex (at end position)
            vertices.push(LineGroupVertex {
                position: end,
                color: tail_color,
                uv: Vec2::new(ucoord, 1.0),
            });

            // Base triangle vertices
            vertices.push(LineGroupVertex {
                position: start + offset_a * size,
                color: head_color,
                uv: Vec2::new(ucoord, 0.0),
            });
            vertices.push(LineGroupVertex {
                position: start + offset_b * size,
                color: head_color,
                uv: Vec2::new(ucoord, 0.0),
            });
            vertices.push(LineGroupVertex {
                position: start + offset_c * size,
                color: head_color,
                uv: Vec2::new(ucoord, 0.0),
            });

            // Triangle indices (4 triangles forming tetrahedron)
            // apex, offset[1], offset[0]
            indices.extend_from_slice(&[base_idx, base_idx + 2, base_idx + 1]);
            // apex, offset[2], offset[1]
            indices.extend_from_slice(&[base_idx, base_idx + 3, base_idx + 2]);
            // apex, offset[0], offset[2]
            indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 3]);
            // Base triangle: offset[0], offset[1], offset[2]
            indices.extend_from_slice(&[base_idx + 1, base_idx + 2, base_idx + 3]);
        }

        (vertices, indices)
    }

    /// Generate prism geometry
    /// Each line becomes a triangular prism from start_loc to end_loc
    fn generate_prism_geometry(
        &self,
        start_locs: &[Vec3],
        end_locs: &[Vec3],
        diffuse: Option<&[Vec4]>,
        tail_diffuse: Option<&[Vec4]>,
        sizes: Option<&[f32]>,
        ucoords: Option<&[f32]>,
        camera_up: Vec3,
        camera_right: Vec3,
    ) -> (Vec<LineGroupVertex>, Vec<u16>) {
        let line_count = start_locs.len();
        let mut vertices = Vec::with_capacity(line_count * 6);
        let mut indices = Vec::with_capacity(line_count * 24); // 8 triangles * 3 indices

        // Calculate offset vectors for triangle cross-section (120 degrees apart)
        let offset_a = camera_up;
        let offset_b = camera_up * -0.5 + camera_right * (3.0_f32.sqrt() / 2.0);
        let offset_c = camera_up * -0.5 - camera_right * (3.0_f32.sqrt() / 2.0);

        for i in 0..line_count {
            let start = start_locs[i];
            let end = end_locs[i];
            let size = sizes.map(|s| s[i]).unwrap_or(self.default_line_size);
            let head_color = diffuse.map(|d| d[i]).unwrap_or(Vec4::new(
                self.default_line_color.x,
                self.default_line_color.y,
                self.default_line_color.z,
                self.default_line_alpha,
            ));
            let tail_color = tail_diffuse
                .map(|td| td[i])
                .unwrap_or(self.default_tail_diffuse);
            let ucoord = ucoords.map(|u| u[i]).unwrap_or(self.default_line_ucoord);

            let base_idx = vertices.len() as u16;

            // Start cap triangle
            vertices.push(LineGroupVertex {
                position: start + offset_a * size,
                color: head_color,
                uv: Vec2::new(ucoord, 0.0),
            });
            vertices.push(LineGroupVertex {
                position: start + offset_b * size,
                color: head_color,
                uv: Vec2::new(ucoord, 0.0),
            });
            vertices.push(LineGroupVertex {
                position: start + offset_c * size,
                color: head_color,
                uv: Vec2::new(ucoord, 0.0),
            });

            // End cap triangle
            vertices.push(LineGroupVertex {
                position: end + offset_a * size,
                color: tail_color,
                uv: Vec2::new(ucoord, 1.0),
            });
            vertices.push(LineGroupVertex {
                position: end + offset_b * size,
                color: tail_color,
                uv: Vec2::new(ucoord, 1.0),
            });
            vertices.push(LineGroupVertex {
                position: end + offset_c * size,
                color: tail_color,
                uv: Vec2::new(ucoord, 1.0),
            });

            // Triangle indices (8 triangles forming prism)
            // Start cap
            indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2]);
            // Left side (2 triangles)
            indices.extend_from_slice(&[base_idx, base_idx + 3, base_idx + 1]);
            indices.extend_from_slice(&[base_idx + 1, base_idx + 3, base_idx + 4]);
            // Bottom side (2 triangles)
            indices.extend_from_slice(&[base_idx + 1, base_idx + 4, base_idx + 5]);
            indices.extend_from_slice(&[base_idx + 1, base_idx + 5, base_idx + 2]);
            // Right side (2 triangles)
            indices.extend_from_slice(&[base_idx, base_idx + 2, base_idx + 5]);
            indices.extend_from_slice(&[base_idx, base_idx + 5, base_idx + 3]);
            // End cap
            indices.extend_from_slice(&[base_idx + 3, base_idx + 5, base_idx + 4]);
        }

        (vertices, indices)
    }

    /// Get polygon count for current line mode
    pub fn get_polygon_count(&self, line_count: usize) -> usize {
        match self.line_mode {
            LineMode::Tetrahedron => line_count * 4,
            LineMode::Prism => line_count * 8,
        }
    }

    /// Create default shader for line group rendering
    fn create_default_shader(device: &Device) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("LineGroup Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/line_group.wgsl").into()),
        })
    }
}

/// Vertex data for line group rendering
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LineGroupVertex {
    pub position: Vec3,
    pub color: Vec4,
    pub uv: Vec2,
}

// Manual implementation of Pod and Zeroable
unsafe impl bytemuck::Pod for LineGroupVertex {}
unsafe impl bytemuck::Zeroable for LineGroupVertex {}
