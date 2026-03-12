//! WGPU Backend - Modern cross-platform implementation
//!
//! This module provides WGPU-based implementations that maintain the same API
//! as the existing DirectX system, allowing seamless integration.

use glam::{Mat4, Vec3};
use std::sync::Arc;
use wgpu::*;

use crate::{
    bump_mapping::{BumpDiffShaderDef, BumpSpecShaderDef},
    class_ids::ShdDefClassId,
    cubemap::CubeMapShaderDef,
    gloss_mask::GlossMaskShaderDef,
    interface::RenderInfo,
    simple::SimpleShaderDef,
    ShdDefClass, ShdInterface, ShdResult,
};

/// WGPU-based shader interface implementation that maintains the same API
pub struct WgpuShaderInterface {
    class_id: u32,
    definition: Arc<dyn ShdDefClass>,

    // WGPU resources
    device: Arc<Device>,
    queue: Arc<Queue>,
    render_pipeline: Option<RenderPipeline>,
    bind_group: Option<BindGroup>,

    // Uniform buffers
    camera_buffer: Buffer,
    material_buffer: Buffer,
    light_buffer: Buffer,

    // Textures
    diffuse_texture: Option<TextureView>,
    normal_texture: Option<TextureView>,
    sampler: Sampler,

    // Pass information
    pass_count: u32,
    current_pass: u32,
}

impl WgpuShaderInterface {
    pub fn new(
        class_id: u32,
        definition: Arc<dyn ShdDefClass>,
        device: Arc<Device>,
        queue: Arc<Queue>,
    ) -> ShdResult<Self> {
        // Create uniform buffers
        let camera_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Camera Uniform Buffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let material_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Material Uniform Buffer"),
            size: std::mem::size_of::<MaterialUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Light Uniform Buffer"),
            size: std::mem::size_of::<LightUniform>() as u64 * 8, // 8 lights max
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create default sampler
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Default Sampler"),
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 32.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        });

        Ok(Self {
            class_id,
            definition,
            device,
            queue,
            render_pipeline: None,
            bind_group: None,
            camera_buffer,
            material_buffer,
            light_buffer,
            diffuse_texture: None,
            normal_texture: None,
            sampler,
            pass_count: 1, // Default to single pass
            current_pass: 0,
        })
    }

    /// Initialize the WGPU pipeline based on shader class ID
    pub fn initialize_pipeline(&mut self, surface_format: TextureFormat) -> ShdResult<()> {
        let shader_source = match ShdDefClassId::try_from(self.class_id)? {
            ShdDefClassId::Simple => include_str!("../shaders/modern/simple.wgsl"),
            ShdDefClassId::BumpDiff => include_str!("../shaders/modern/bump_mapping.wgsl"),
            ShdDefClassId::BumpSpec => include_str!("../shaders/modern/bump_mapping.wgsl"),
            ShdDefClassId::CubeMap => include_str!("../shaders/modern/cubemap.wgsl"),
            ShdDefClassId::GlossMask => include_str!("../shaders/modern/simple.wgsl"), // Fallback
            ShdDefClassId::LegacyW3D => include_str!("../shaders/modern/simple.wgsl"), // Fallback
            _ => include_str!("../shaders/modern/simple.wgsl"), // Default fallback
        };

        let shader = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("WGPU Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        // Create bind group layout
        let bind_group_layout = self
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                entries: &[
                    // Camera uniforms (binding 0)
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Light uniforms (binding 1)
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Material uniforms (binding 2)
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Diffuse texture (binding 3)
                    BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            multisampled: false,
                            view_dimension: TextureViewDimension::D2,
                            sample_type: TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    // Normal texture (binding 4) - for bump mapping
                    BindGroupLayoutEntry {
                        binding: 4,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            multisampled: false,
                            view_dimension: TextureViewDimension::D2,
                            sample_type: TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    // Sampler (binding 5)
                    BindGroupLayoutEntry {
                        binding: 5,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("Shader Bind Group Layout"),
            });

        let pipeline_layout = self
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        // Create render pipeline
        self.render_pipeline = Some(self.device.create_render_pipeline(
            &RenderPipelineDescriptor {
                label: Some("WGPU Render Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[WgpuVertex::desc()],
                    compilation_options: PipelineCompilationOptions::default(),
                },
                fragment: Some(FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(ColorTargetState {
                        format: surface_format,
                        blend: Some(BlendState::ALPHA_BLENDING),
                        write_mask: ColorWrites::ALL,
                    })],
                    compilation_options: PipelineCompilationOptions::default(),
                }),
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: FrontFace::Ccw,
                    cull_mode: Some(Face::Back),
                    polygon_mode: PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(DepthStencilState {
                    format: TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: CompareFunction::Less,
                    stencil: StencilState::default(),
                    bias: DepthBiasState::default(),
                }),
                multisample: MultisampleState::default(),
                multiview: None,
                cache: None,
            },
        ));

        self.create_bind_group(&bind_group_layout)?;

        Ok(())
    }

    fn create_bind_group(&mut self, layout: &BindGroupLayout) -> ShdResult<()> {
        // Create a default 1x1 white texture if no textures are set
        let default_texture = if self.diffuse_texture.is_none() {
            let texture = self.device.create_texture(&TextureDescriptor {
                label: Some("Default White Texture"),
                size: Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            });

            // Write white pixel data
            self.queue.write_texture(
                TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                &[255, 255, 255, 255], // White pixel
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
                Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );

            texture.create_view(&TextureViewDescriptor::default())
        } else {
            return Ok(()); // Use existing texture
        };

        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: self.light_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: self.material_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(
                        self.diffuse_texture.as_ref().unwrap_or(&default_texture),
                    ),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::TextureView(
                        self.normal_texture.as_ref().unwrap_or(&default_texture),
                    ),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: BindingResource::Sampler(&self.sampler),
                },
            ],
            label: Some("Shader Bind Group"),
        });

        self.bind_group = Some(bind_group);
        Ok(())
    }

    /// Update camera uniform buffer
    pub fn update_camera(&self, view_projection: Mat4, view_position: Vec3) {
        let camera_uniform = CameraUniform {
            view_projection: view_projection.to_cols_array_2d(),
            view_position: view_position.into(),
            _padding: 0.0,
        };

        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );
    }

    /// Update material uniform buffer  
    pub fn update_material(&self, ambient: Vec3, diffuse: Vec3, specular: Vec3, shininess: f32) {
        let material_uniform = MaterialUniform {
            ambient: ambient.into(),
            _padding1: 0.0,
            diffuse: diffuse.into(),
            _padding2: 0.0,
            specular: specular.into(),
            shininess,
        };

        self.queue.write_buffer(
            &self.material_buffer,
            0,
            bytemuck::cast_slice(&[material_uniform]),
        );
    }
}

// Implement the same ShdInterface trait as the legacy system
impl ShdInterface for WgpuShaderInterface {
    fn get_class_id(&self) -> u32 {
        self.class_id
    }

    fn get_pass_count(&self) -> u32 {
        self.pass_count
    }

    fn apply_shared(&mut self, _pass: u32, render_info: &RenderInfo) -> ShdResult<()> {
        let view_proj = render_info.projection_matrix * render_info.view_matrix;
        let world_view_proj = view_proj * render_info.world_matrix;
        self.update_camera(world_view_proj, render_info.camera_position);
        Ok(())
    }

    fn apply_instance(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        let class_id = ShdDefClassId::try_from(self.class_id)?;

        match class_id {
            ShdDefClassId::Simple => {
                if let Some(def) = self.definition.as_any().downcast_ref::<SimpleShaderDef>() {
                    self.update_material(def.get_ambient(), def.get_diffuse(), Vec3::ZERO, 1.0);
                }
            }
            ShdDefClassId::BumpDiff => {
                if let Some(def) = self.definition.as_any().downcast_ref::<BumpDiffShaderDef>() {
                    self.update_material(def.get_ambient(), def.get_diffuse(), Vec3::ZERO, 1.0);
                }
            }
            ShdDefClassId::BumpSpec => {
                if let Some(def) = self.definition.as_any().downcast_ref::<BumpSpecShaderDef>() {
                    let shininess = def.get_specular_bumpiness().x.max(1.0);
                    self.update_material(
                        def.get_ambient(),
                        def.get_diffuse(),
                        def.get_specular(),
                        shininess,
                    );
                }
            }
            ShdDefClassId::CubeMap => {
                if let Some(def) = self.definition.as_any().downcast_ref::<CubeMapShaderDef>() {
                    self.update_material(
                        def.get_ambient(),
                        def.get_diffuse(),
                        def.get_specular(),
                        8.0,
                    );
                }
            }
            ShdDefClassId::GlossMask => {
                if let Some(def) = self
                    .definition
                    .as_any()
                    .downcast_ref::<GlossMaskShaderDef>()
                {
                    self.update_material(
                        def.get_ambient(),
                        def.get_diffuse(),
                        def.get_specular(),
                        20.0_f32,
                    );
                }
            }
            ShdDefClassId::LegacyW3D => {
                self.update_material(Vec3::splat(0.3), Vec3::splat(0.7), Vec3::ZERO, 1.0);
            }
            _ => {
                // Fallback to a reasonable default
                self.update_material(Vec3::splat(0.2), Vec3::splat(0.8), Vec3::ZERO, 1.0);
            }
        }

        Ok(())
    }

    fn get_vertex_stream_count(&self) -> u32 {
        1 // WGPU uses interleaved vertex data
    }

    fn get_vertex_size(&self, _stream: u32) -> u32 {
        std::mem::size_of::<WgpuVertex>() as u32
    }

    fn use_hardware_vertex_processing(&self) -> bool {
        true // WGPU always uses hardware vertex processing
    }

    fn get_texture_count(&self) -> u32 {
        match ShdDefClassId::try_from(self.class_id).unwrap_or(ShdDefClassId::Simple) {
            ShdDefClassId::BumpDiff | ShdDefClassId::BumpSpec => 2, // Diffuse + Normal
            ShdDefClassId::CubeMap => 2,                            // Diffuse + Environment
            _ => 1,                                                 // Just diffuse
        }
    }

    fn is_opaque(&self) -> bool {
        // Most shaders are opaque, override in specific implementations for alpha
        true
    }
}

impl std::fmt::Debug for WgpuShaderInterface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WgpuShaderInterface")
            .field("class_id", &self.class_id)
            .field("pass_count", &self.pass_count)
            .field("current_pass", &self.current_pass)
            .finish_non_exhaustive()
    }
}

/// WGPU vertex structure that matches our WGSL shader
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WgpuVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    tangent: [f32; 3],
    color: [f32; 4],
}

impl WgpuVertex {
    pub fn desc<'a>() -> VertexBufferLayout<'a> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<WgpuVertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                // Position
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x3,
                },
                // Normal
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as BufferAddress,
                    shader_location: 1,
                    format: VertexFormat::Float32x3,
                },
                // UV
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as BufferAddress,
                    shader_location: 2,
                    format: VertexFormat::Float32x2,
                },
                // Tangent
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as BufferAddress,
                    shader_location: 3,
                    format: VertexFormat::Float32x3,
                },
                // Color
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 11]>() as BufferAddress,
                    shader_location: 4,
                    format: VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Uniform structures that match WGSL
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_projection: [[f32; 4]; 4],
    view_position: [f32; 3],
    _padding: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    position: [f32; 3],
    _padding1: f32,
    color: [f32; 3],
    intensity: f32,
    direction: [f32; 3],
    _padding2: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniform {
    ambient: [f32; 3],
    _padding1: f32,
    diffuse: [f32; 3],
    _padding2: f32,
    specular: [f32; 3],
    shininess: f32,
}

/// Factory function that creates WGPU shader interfaces
/// This maintains compatibility with your existing shader factory system
pub fn create_wgpu_shader_interface(
    class_id: u32,
    definition: Arc<dyn ShdDefClass>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    surface_format: TextureFormat,
) -> ShdResult<Box<dyn ShdInterface>> {
    let mut interface = WgpuShaderInterface::new(class_id, definition, device, queue)?;
    interface.initialize_pipeline(surface_format)?;
    Ok(Box::new(interface))
}
