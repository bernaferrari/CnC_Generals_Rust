//! Water Renderer Module
//!
//! Corresponds to C++ file:
//! - GameEngineDevice/Source/W3DDevice/GameClient/Water/W3DWater.cpp
//!
//! This module implements water surface rendering with:
//! - Wave animation and displacement
//! - Reflection rendering
//! - Refraction and distortion
//! - Normal mapping
//! - Specular highlights
//! - Fresnel effects

use super::water_config::*;
use std::f32::consts::PI;
use wgpu::util::DeviceExt;

/// Vertex format for water surface
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WaterVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl WaterVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x2,
        3 => Float32x4,
    ];

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<WaterVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Camera uniforms matching shader
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub view: [[f32; 4]; 4],
    pub projection: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub _padding: f32,
}

/// Water-specific uniforms matching shader
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WaterUniforms {
    pub world_transform: [[f32; 4]; 4],
    pub water_color: [f32; 4],
    pub water_level: f32,
    pub time: f32,
    pub wave_scale: f32,
    pub wave_speed: f32,
    pub bump_scale: f32,
    pub reflection_factor: f32,
    pub fresnel_bias: f32,
    pub fresnel_power: f32,
    pub uv_scroll: [f32; 2],
    pub grid_scale: [f32; 2],
}

/// Light uniforms matching shader
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniforms {
    pub direction: [f32; 3],
    pub _padding1: f32,
    pub ambient: [f32; 3],
    pub _padding2: f32,
    pub diffuse: [f32; 3],
    pub _padding3: f32,
    pub specular: [f32; 3],
    pub specular_power: f32,
}

/// Main water rendering system
/// Matches C++ WaterRenderObjClass from W3DWater.h
pub struct WaterRenderer {
    // WGPU resources
    device: wgpu::Device,
    queue: wgpu::Queue,

    // Rendering pipeline
    render_pipeline: wgpu::RenderPipeline,
    simple_pipeline: wgpu::RenderPipeline,

    // Buffers
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    camera_buffer: wgpu::Buffer,
    water_buffer: wgpu::Buffer,
    light_buffer: wgpu::Buffer,

    // Bind groups
    uniform_bind_group: wgpu::BindGroup,
    texture_bind_group: wgpu::BindGroup,

    // Textures
    water_texture: wgpu::Texture,
    normal_map: wgpu::Texture,
    reflection_texture: wgpu::Texture,
    caustics_texture: wgpu::Texture,

    // Water mesh data
    mesh_data: Vec<WaterMeshData>,
    grid_transform: GridTransform,

    // Settings
    water_type: WaterType,
    time_of_day: TimeOfDay,
    settings: [WaterSetting; TIME_OF_DAY_COUNT],
    transparency: WaterTransparencySetting,

    // Animation state
    water_level: f32,
    elapsed_time: f32,
    uv_offset: [f32; 2],
    mesh_in_motion: bool,
    river_v_origin: f32,
    river_x_offset: f32,
    river_y_offset: f32,
    bump_frame: usize,
    logic_time_accumulator: f32,

    // Performance
    use_high_quality: bool,
    enable_reflections: bool,
    enable_caustics: bool,
}

impl WaterRenderer {
    /// Create a new water renderer
    /// Matches C++ WaterRenderObjClass::init() from W3DWater.cpp
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        water_level: f32,
        dx: f32,
        dy: f32,
        water_type: WaterType,
    ) -> Self {
        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Water Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("water_shader.wgsl").into()),
        });

        // Create uniform buffers
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Uniform Buffer"),
            size: std::mem::size_of::<CameraUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let water_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Water Uniform Buffer"),
            size: std::mem::size_of::<WaterUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Uniform Buffer"),
            size: std::mem::size_of::<LightUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layouts
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Water Uniform Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Water Texture Bind Group Layout"),
                entries: &[
                    // Water texture
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
                    // Normal map
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Reflection texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Caustics texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Water Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Water Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[WaterVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: Some(wgpu::IndexFormat::Uint16),
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // Create simple pipeline (low detail)
        let simple_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Water Simple Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[WaterVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_simple",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: Some(wgpu::IndexFormat::Uint16),
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // Generate water mesh
        let (vertices, indices) = Self::generate_water_mesh(
            constants::PATCH_SIZE,
            constants::PATCH_SIZE,
            dx,
            dy,
            water_level,
        );
        let index_count = indices.len() as u32;

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Water Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Water Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Create placeholder textures (to be loaded from assets)
        let water_texture = Self::create_placeholder_texture(&device, &queue, 256, 256, "Water");
        let normal_map = Self::create_placeholder_texture(&device, &queue, 256, 256, "Normal");
        let caustics_texture =
            Self::create_placeholder_texture(&device, &queue, 256, 256, "Caustics");

        // Create reflection texture (render target)
        let reflection_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Reflection Texture"),
            size: wgpu::Extent3d {
                width: constants::SEA_REFLECTION_SIZE,
                height: constants::SEA_REFLECTION_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        // Create samplers
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Water Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create bind groups
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Water Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: water_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: light_buffer.as_entire_binding(),
                },
            ],
        });

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Water Texture Bind Group"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &water_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &normal_map.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(
                        &reflection_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(
                        &caustics_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Initialize water mesh data for grid-based water
        let grid_transform = GridTransform::default();
        let mesh_size = (grid_transform.cells_x + 2) * (grid_transform.cells_y + 2);
        let mesh_data = vec![WaterMeshData::default(); mesh_size];

        Self {
            device,
            queue,
            render_pipeline,
            simple_pipeline,
            vertex_buffer,
            index_buffer,
            index_count,
            camera_buffer,
            water_buffer,
            light_buffer,
            uniform_bind_group,
            texture_bind_group,
            water_texture,
            normal_map,
            reflection_texture,
            caustics_texture,
            mesh_data,
            grid_transform,
            water_type,
            time_of_day: TimeOfDay::Afternoon,
            settings: [
                WaterSetting::default(),
                WaterSetting::default(),
                WaterSetting::default(),
                WaterSetting::default(),
            ],
            transparency: WaterTransparencySetting::default(),
            water_level,
            elapsed_time: 0.0,
            uv_offset: [0.0, 0.0],
            mesh_in_motion: false,
            river_v_origin: 0.0,
            river_x_offset: 0.0,
            river_y_offset: 0.0,
            bump_frame: 0,
            logic_time_accumulator: 0.0,
            use_high_quality: true,
            enable_reflections: true,
            enable_caustics: true,
        }
    }

    /// Generate water mesh vertices and indices
    /// Matches C++ WaterRenderObjClass::generateVertexBuffer and generateIndexBuffer
    fn generate_water_mesh(
        size_x: usize,
        size_y: usize,
        dx: f32,
        dy: f32,
        water_level: f32,
    ) -> (Vec<WaterVertex>, Vec<u16>) {
        if size_x < 2 || size_y < 2 {
            return (Vec::new(), Vec::new());
        }

        let mut vertices = Vec::with_capacity(size_x * size_y);

        let step_x = dx / (size_x - 1) as f32;
        let step_y = dy / (size_y - 1) as f32;
        let start_x = -dx / 2.0;
        let start_y = -dy / 2.0;

        // Generate vertices
        for y in 0..size_y {
            for x in 0..size_x {
                let px = start_x + x as f32 * step_x;
                let py = start_y + y as f32 * step_y;

                let u = x as f32 / (size_x - 1) as f32;
                let v = y as f32 / (size_y - 1) as f32;

                vertices.push(WaterVertex {
                    position: [px, py, water_level],
                    normal: [0.0, 0.0, 1.0],
                    uv: [u * constants::PATCH_UV_SCALE, v * constants::PATCH_UV_SCALE],
                    color: [1.0, 1.0, 1.0, 0.8],
                });
            }
        }

        // Generate indices using the same triangle-strip-with-degenerates pattern as C++.
        let mut indices = Vec::with_capacity((size_y - 1) * (size_x * 2 + 2) - 2);

        for y in 0..(size_y - 1) {
            let row_start = y * size_x;
            let next_row_start = (y + 1) * size_x;

            for x in 0..size_x {
                indices.push((next_row_start + x) as u16);
                indices.push((row_start + x) as u16);
            }

            if y + 1 < size_y - 1 {
                indices.push((row_start + size_x - 1) as u16);
                indices.push(((y + 2) * size_x) as u16);
            }
        }

        (vertices, indices)
    }

    /// Create placeholder texture
    fn create_placeholder_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        label: &str,
    ) -> wgpu::Texture {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("{} Texture", label)),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Create checkerboard pattern
        let mut data = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;
                let checker = ((x / 16) + (y / 16)) % 2;
                let value = if checker == 0 { 100 } else { 150 };
                data[idx] = value;
                data[idx + 1] = value;
                data[idx + 2] = value + 50;
                data[idx + 3] = 255;
            }
        }

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        texture
    }

    /// Update water simulation
    /// Matches C++ WaterRenderObjClass::update()
    pub fn update(&mut self, delta_time: f32) {
        self.elapsed_time += delta_time;

        // C++ advances the scroll/bump animation every client frame.
        self.river_v_origin += 0.002;
        self.river_x_offset += 0.0125 * 33.0 / 5000.0;
        self.river_y_offset += 2.0 * 0.0125 * 33.0 / 5000.0;
        Self::wrap_unit_range(&mut self.river_x_offset);
        Self::wrap_unit_range(&mut self.river_y_offset);
        self.bump_frame = (self.bump_frame + 1) % constants::NUM_BUMP_FRAMES;
        self.uv_offset = [self.river_x_offset, self.river_y_offset];

        // Water grid simulation still runs on a logic-frame cadence to avoid oversampling.
        self.logic_time_accumulator += delta_time;
        const LOGIC_FRAME_SECONDS: f32 = 1.0 / 30.0;

        while self.logic_time_accumulator >= LOGIC_FRAME_SECONDS {
            self.logic_time_accumulator -= LOGIC_FRAME_SECONDS;
            self.advance_logic_frame();
        }
    }

    fn advance_logic_frame(&mut self) {
        if self.water_type == WaterType::GridMesh && self.mesh_in_motion {
            self.update_water_mesh();
        }
    }

    fn wrap_unit_range(value: &mut f32) {
        if *value > 1.0 {
            *value -= 1.0;
        } else if *value < -1.0 {
            *value += 1.0;
        }
    }

    /// Update 3D water mesh simulation
    /// Matches C++ WaterRenderObjClass::renderWaterMesh() physics
    fn update_water_mesh(&mut self) {
        let cells_x = self.grid_transform.cells_x;
        let cells_y = self.grid_transform.cells_y;

        // Clear the active flag first, mirroring the C++ logic.
        self.mesh_in_motion = false;

        // Approximate the legacy logic-frame behavior.
        let delta_time = 1.0 / 30.0;
        let damping = 0.93;
        let spring_constant = 0.025;
        let preferred_height_fudge = 1.0;
        let at_rest_velocity_fudge = 1.0;

        for y in 1..=cells_y {
            for x in 1..=cells_x {
                let idx = y * (cells_x + 2) + x;
                let left = self.mesh_data[y * (cells_x + 2) + (x - 1)].height;
                let right = self.mesh_data[y * (cells_x + 2) + (x + 1)].height;
                let up = self.mesh_data[(y - 1) * (cells_x + 2) + x].height;
                let down = self.mesh_data[(y + 1) * (cells_x + 2) + x].height;
                let preferred_height = self.mesh_data[idx].preferred_height as f32;
                let mesh_height = self.mesh_data[idx].height;

                let avg_neighbor = (left + right + up + down) * 0.25;
                let force = (avg_neighbor - mesh_height) * spring_constant;

                let mesh = &mut self.mesh_data[idx];

                // Update velocity and position
                mesh.velocity += force * delta_time;
                mesh.velocity *= damping;
                mesh.height += mesh.velocity * delta_time;

                // Clamp height
                mesh.height = mesh.height.clamp(
                    self.grid_transform.min_height,
                    self.grid_transform.max_height,
                );

                // Match the legacy "at rest" thresholds more closely.
                if (mesh.height - preferred_height).abs() < preferred_height_fudge
                    && mesh.velocity.abs() < at_rest_velocity_fudge
                {
                    mesh.status = mesh_status::AT_REST;
                    mesh.height = preferred_height;
                    mesh.velocity = 0.0;
                } else {
                    mesh.status = mesh_status::IN_MOTION;
                    self.mesh_in_motion = true;
                }
            }
        }
    }

    /// Add velocity to water mesh at world position
    /// Matches C++ WaterRenderObjClass::addVelocity()
    pub fn add_velocity(
        &mut self,
        world_x: f32,
        world_y: f32,
        z_velocity: f32,
        preferred_height: f32,
    ) {
        if let Some((grid_x, grid_y)) = self.world_to_grid_space(world_x, world_y) {
            let min_x = (grid_x - self.grid_transform.change_max_range)
                .floor()
                .max(0.0) as usize;
            let max_x = (grid_x + self.grid_transform.change_max_range)
                .ceil()
                .min(self.grid_transform.cells_x as f32) as usize;
            let min_y = (grid_y - self.grid_transform.change_max_range)
                .floor()
                .max(0.0) as usize;
            let max_y = (grid_y + self.grid_transform.change_max_range)
                .ceil()
                .min(self.grid_transform.cells_y as f32) as usize;

            let preferred_height = preferred_height.clamp(0.0, 255.0) as u8;

            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    let idx = (y + 1) * (self.grid_transform.cells_x + 2) + (x + 1);
                    let mesh_point = &mut self.mesh_data[idx];
                    mesh_point.velocity += z_velocity;
                    mesh_point.preferred_height = preferred_height;
                    mesh_point.status = mesh_status::IN_MOTION;
                }
            }

            self.mesh_in_motion = true;
        }
    }

    /// Convert world coordinates to grid space
    /// Matches C++ WaterRenderObjClass::worldToGridSpace()
    fn world_to_grid_space(&self, world_x: f32, world_y: f32) -> Option<(f32, f32)> {
        let dx = world_x - self.grid_transform.origin[0];
        let dy = world_y - self.grid_transform.origin[1];

        let grid_x = (dx * self.grid_transform.direction_x[0]
            + dy * self.grid_transform.direction_x[1])
            / self.grid_transform.cell_size;
        let grid_y = (dx * self.grid_transform.direction_y[0]
            + dy * self.grid_transform.direction_y[1])
            / self.grid_transform.cell_size;

        if grid_x >= 0.0
            && grid_x < self.grid_transform.cells_x as f32
            && grid_y >= 0.0
            && grid_y < self.grid_transform.cells_y as f32
        {
            Some((grid_x, grid_y))
        } else {
            None
        }
    }

    /// Render water surface
    /// Matches C++ WaterRenderObjClass::Render()
    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_uniforms: &CameraUniforms,
    ) {
        // Update camera uniforms
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[*camera_uniforms]),
        );

        // Update water uniforms
        let water_uniforms = WaterUniforms {
            world_transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            water_color: [0.2, 0.4, 0.6, 0.8],
            water_level: self.water_level,
            time: self.river_v_origin,
            wave_scale: 1.0,
            wave_speed: 1.0,
            bump_scale: constants::SEA_BUMP_SCALE,
            reflection_factor: constants::REFLECTION_FACTOR,
            fresnel_bias: 0.1,
            fresnel_power: 2.0,
            uv_scroll: [self.river_x_offset, self.river_y_offset],
            grid_scale: [
                constants::NOISE_REPEAT_FACTOR,
                constants::NOISE_REPEAT_FACTOR,
            ],
        };

        self.queue.write_buffer(
            &self.water_buffer,
            0,
            bytemuck::cast_slice(&[water_uniforms]),
        );

        // Update light uniforms
        let light_uniforms = LightUniforms {
            direction: [-0.57, -0.57, -0.57],
            _padding1: 0.0,
            ambient: [0.3, 0.3, 0.3],
            _padding2: 0.0,
            diffuse: [0.7, 0.7, 0.7],
            _padding3: 0.0,
            specular: [1.0, 1.0, 1.0],
            specular_power: 32.0,
        };

        self.queue.write_buffer(
            &self.light_buffer,
            0,
            bytemuck::cast_slice(&[light_uniforms]),
        );

        // Select pipeline based on quality settings
        let pipeline = if self.use_high_quality {
            &self.render_pipeline
        } else {
            &self.simple_pipeline
        };

        // Render water
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        render_pass.set_bind_group(1, &self.texture_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        render_pass.draw_indexed(0..self.index_count, 0, 0..1);
    }

    /// Set time of day
    pub fn set_time_of_day(&mut self, tod: TimeOfDay) {
        self.time_of_day = tod;
    }

    /// Set water type
    pub fn set_water_type(&mut self, water_type: WaterType) {
        self.water_type = water_type;
    }

    /// Enable/disable high quality rendering
    pub fn set_high_quality(&mut self, enabled: bool) {
        self.use_high_quality = enabled;
    }

    /// Enable/disable reflections
    pub fn set_reflections_enabled(&mut self, enabled: bool) {
        self.enable_reflections = enabled;
    }

    /// Enable/disable caustics
    pub fn set_caustics_enabled(&mut self, enabled: bool) {
        self.enable_caustics = enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_water_vertex_size() {
        assert_eq!(std::mem::size_of::<WaterVertex>(), 40);
    }

    #[test]
    fn test_mesh_generation() {
        let (vertices, indices) = WaterRenderer::generate_water_mesh(10, 10, 100.0, 100.0, 0.0);

        assert_eq!(vertices.len(), 100);
        assert_eq!(indices.len(), (10 - 1) * (10 * 2 + 2) - 2);
    }

    #[test]
    fn test_grid_transform() {
        let transform = GridTransform::default();
        assert_eq!(transform.cells_x, constants::WATER_MESH_X_VERTICES);
        assert_eq!(transform.cells_y, constants::WATER_MESH_Y_VERTICES);
    }
}
