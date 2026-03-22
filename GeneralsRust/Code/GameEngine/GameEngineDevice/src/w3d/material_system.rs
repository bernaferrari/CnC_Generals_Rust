//! # W3D Material System - Advanced PBR Shader Management
//!
//! This module implements a complete modern material and shader system featuring:
//! - Physically Based Rendering (PBR) materials
//! - Dynamic shader compilation and caching
//! - Texture atlas management and streaming
//! - Material parameter animation
//! - Shader variants for different rendering paths
//! - GPU-resident material data structures

use super::{BoundingBox, Result, W3DError};
use crate::video::{ColorFormat, Resolution};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2, Vec3, Vec4};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
#[cfg(feature = "w3d")]
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

#[cfg(feature = "w3d")]
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindingType, BlendComponent, BlendFactor, BlendOperation,
    BlendState, Buffer, BufferBindingType, BufferDescriptor, BufferUsages, ColorTargetState,
    ColorWrites, CompareFunction, ComputePipeline, ComputePipelineDescriptor, DepthBiasState,
    DepthStencilState, Device, Extent3d, Face, FilterMode, FragmentState, FrontFace,
    MultisampleState, Origin3d, PipelineLayout, PipelineLayoutDescriptor, PolygonMode,
    PrimitiveState, PrimitiveTopology, Queue, RenderPipeline, RenderPipelineDescriptor, Sampler,
    SamplerBindingType, SamplerBorderColor, SamplerDescriptor, ShaderModule,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, StencilState, StorageTextureAccess,
    TexelCopyBufferLayout, TexelCopyTextureInfo, Texture, TextureAspect, TextureDescriptor,
    TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureView,
    TextureViewDimension, VertexBufferLayout, VertexState,
};

/// Maximum number of materials in GPU buffer
const MAX_MATERIALS: usize = 2048;
/// Maximum number of texture atlas layers
const MAX_TEXTURE_LAYERS: usize = 256;
/// Default texture resolution for missing textures
const DEFAULT_TEXTURE_SIZE: u32 = 64;

/// PBR material parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DMaterialData {
    /// Base color (albedo) - RGBA
    pub base_color: [f32; 4],
    /// Metallic factor (0.0 = dielectric, 1.0 = metallic)
    pub metallic: f32,
    /// Roughness factor (0.0 = mirror, 1.0 = completely rough)
    pub roughness: f32,
    /// Ambient occlusion factor
    pub ambient_occlusion: f32,
    /// Normal map intensity
    pub normal_intensity: f32,
    /// Emissive color and intensity
    pub emissive: [f32; 4],
    /// Texture coordinate transform (scale_u, scale_v, offset_u, offset_v)
    pub uv_transform: [f32; 4],
    /// Texture array indices (diffuse, normal, metallic_roughness, emissive)
    pub texture_indices: [u32; 4],
    /// Texture array indices (occlusion, height, detail_normal, detail_mask)
    pub texture_indices_2: [u32; 4],
    /// Material flags and parameters
    pub flags: u32,
    /// Alpha cutoff for alpha testing
    pub alpha_cutoff: f32,
    /// Detail texture scale
    pub detail_scale: f32,
    /// Height scale for parallax mapping
    pub height_scale: f32,
    /// Animation time offset
    pub animation_time: f32,
    /// Custom parameters for advanced materials
    pub custom_params: [f32; 3],
}

impl Default for W3DMaterialData {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            ambient_occlusion: 1.0,
            normal_intensity: 1.0,
            emissive: [0.0, 0.0, 0.0, 0.0],
            uv_transform: [1.0, 1.0, 0.0, 0.0],
            texture_indices: [0, 0, 0, 0],
            texture_indices_2: [0, 0, 0, 0],
            flags: 0,
            alpha_cutoff: 0.5,
            detail_scale: 1.0,
            height_scale: 0.05,
            animation_time: 0.0,
            custom_params: [0.0; 3],
        }
    }
}

/// Material flags for different rendering features
#[derive(Debug, Clone, Copy)]
pub struct MaterialFlags;

impl MaterialFlags {
    /// Alpha blending enabled
    pub const ALPHA_BLEND: u32 = 1 << 0;
    /// Alpha testing enabled
    pub const ALPHA_TEST: u32 = 1 << 1;
    /// Two-sided rendering
    pub const DOUBLE_SIDED: u32 = 1 << 2;
    /// Emissive material
    pub const EMISSIVE: u32 = 1 << 3;
    /// Normal mapping enabled
    pub const NORMAL_MAP: u32 = 1 << 4;
    /// Parallax mapping enabled
    pub const PARALLAX_MAP: u32 = 1 << 5;
    /// Detail textures enabled
    pub const DETAIL_TEXTURES: u32 = 1 << 6;
    /// Animated material
    pub const ANIMATED: u32 = 1 << 7;
    /// Vertex colors enabled
    pub const VERTEX_COLORS: u32 = 1 << 8;
    /// Subsurface scattering
    pub const SUBSURFACE: u32 = 1 << 9;
    /// Clear coat layer
    pub const CLEAR_COAT: u32 = 1 << 10;
    /// Anisotropic reflections
    pub const ANISOTROPIC: u32 = 1 << 11;
}

/// Material template for different object types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct W3DMaterial {
    /// Material name/ID
    pub name: String,
    /// Display name
    pub display_name: String,
    /// Material data
    pub data: W3DMaterialData,
    /// Shader variant to use
    pub shader_variant: String,
    /// Texture file paths
    pub texture_paths: MaterialTextures,
    /// Render queue priority
    pub render_queue: u32,
    /// LOD bias for texture selection
    pub lod_bias: f32,
    /// Is the material transparent?
    pub transparent: bool,
    /// Cast shadows?
    pub cast_shadows: bool,
    /// Receive shadows?
    pub receive_shadows: bool,
}

/// Texture paths for a material
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaterialTextures {
    /// Diffuse/albedo texture
    pub diffuse: Option<String>,
    /// Normal map texture
    pub normal: Option<String>,
    /// Metallic/roughness texture (metallic in B, roughness in G)
    pub metallic_roughness: Option<String>,
    /// Ambient occlusion texture
    pub occlusion: Option<String>,
    /// Emissive texture
    pub emissive: Option<String>,
    /// Height/displacement map
    pub height: Option<String>,
    /// Detail normal map
    pub detail_normal: Option<String>,
    /// Detail mask texture
    pub detail_mask: Option<String>,
    /// Custom textures for advanced materials
    pub custom: Vec<String>,
}

/// Shader variant definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct W3DShaderVariant {
    /// Variant name
    pub name: String,
    /// Vertex shader source or path
    pub vertex_shader: String,
    /// Fragment shader source or path
    pub fragment_shader: String,
    /// Geometry shader (optional)
    pub geometry_shader: Option<String>,
    /// Compute shader for material processing (optional)
    pub compute_shader: Option<String>,
    /// Preprocessor defines
    pub defines: HashMap<String, String>,
    /// Vertex input layout
    pub vertex_layout: Vec<VertexAttribute>,
    /// Render state overrides
    pub render_state: RenderState,
}

/// Render state configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderState {
    /// Depth testing enabled
    pub depth_test: bool,
    /// Depth writing enabled
    pub depth_write: bool,
    /// Depth comparison function
    pub depth_compare: String, // "less", "greater", "equal", etc.
    /// Blending configuration
    pub blend_mode: BlendMode,
    /// Face culling mode
    pub cull_mode: String, // "none", "front", "back"
    /// Polygon fill mode
    pub fill_mode: String, // "solid", "wireframe", "point"
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            depth_test: true,
            depth_write: true,
            depth_compare: "less".to_string(),
            blend_mode: BlendMode::Opaque,
            cull_mode: "back".to_string(),
            fill_mode: "solid".to_string(),
        }
    }
}

/// Blend mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlendMode {
    /// No blending (opaque)
    Opaque,
    /// Alpha blending
    Alpha,
    /// Additive blending
    Additive,
    /// Multiplicative blending
    Multiply,
    /// Custom blend factors
    Custom {
        src_color: String,
        dst_color: String,
        color_op: String,
        src_alpha: String,
        dst_alpha: String,
        alpha_op: String,
    },
}

/// GPU shader representation
#[cfg(feature = "w3d")]
pub struct W3DShader {
    /// Shader name
    pub name: String,
    /// Vertex shader module
    pub vertex_module: ShaderModule,
    /// Fragment shader module
    pub fragment_module: ShaderModule,
    /// Geometry shader module (optional)
    pub geometry_module: Option<ShaderModule>,
    /// Compute shader module (optional)
    pub compute_module: Option<ShaderModule>,
    /// Render pipeline
    pub render_pipeline: Option<RenderPipeline>,
    /// Compute pipeline
    pub compute_pipeline: Option<ComputePipeline>,
    /// Bind group layout
    pub bind_group_layout: BindGroupLayout,
    /// Pipeline layout
    pub pipeline_layout: PipelineLayout,
    /// Vertex attributes
    pub vertex_attributes: Vec<wgpu::VertexAttribute>,
}

/// Material system managing all materials and shaders
pub struct W3DShaderManager {
    /// GPU device
    device: Arc<Device>,
    /// GPU queue
    queue: Arc<Queue>,

    /// Loaded materials
    materials: Arc<RwLock<HashMap<String, W3DMaterial>>>,
    /// GPU material buffer
    #[cfg(feature = "w3d")]
    material_buffer: Buffer,
    /// Material buffer data
    material_buffer_data: Arc<RwLock<Vec<W3DMaterialData>>>,
    /// Material name to buffer index mapping
    material_indices: Arc<RwLock<HashMap<String, usize>>>,

    /// Shader variants
    shader_variants: Arc<RwLock<HashMap<String, W3DShaderVariant>>>,
    /// Compiled shaders
    #[cfg(feature = "w3d")]
    compiled_shaders: Arc<RwLock<HashMap<String, W3DShader>>>,

    /// Texture atlas system
    texture_manager: Arc<W3DTextureManager>,

    /// Default materials
    default_material: Option<String>,
    missing_material: Option<String>,

    /// Shader search paths
    shader_paths: Vec<PathBuf>,
}

impl W3DShaderManager {
    /// Create new shader manager
    #[cfg(feature = "w3d")]
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Result<Self> {
        tracing::info!("Initializing W3D shader manager");

        // Create material uniform buffer
        let material_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Material Buffer"),
            size: (MAX_MATERIALS * std::mem::size_of::<W3DMaterialData>()) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let texture_manager = Arc::new(W3DTextureManager::new(device.clone(), queue.clone())?);

        let manager = Self {
            device,
            queue,
            materials: Arc::new(RwLock::new(HashMap::new())),
            material_buffer,
            material_buffer_data: Arc::new(RwLock::new(Vec::new())),
            material_indices: Arc::new(RwLock::new(HashMap::new())),
            shader_variants: Arc::new(RwLock::new(HashMap::new())),
            compiled_shaders: Arc::new(RwLock::new(HashMap::new())),
            texture_manager,
            default_material: None,
            missing_material: None,
            shader_paths: vec![
                PathBuf::from("shaders/"),
                PathBuf::from("assets/shaders/"),
                PathBuf::from("data/shaders/"),
            ],
        };

        tracing::info!("W3D shader manager initialized");
        Ok(manager)
    }

    /// Create new shader manager (no GPU features)
    #[cfg(not(feature = "w3d"))]
    pub fn new() -> Result<Self> {
        let texture_manager = Arc::new(W3DTextureManager::new()?);

        Ok(Self {
            materials: Arc::new(RwLock::new(HashMap::new())),
            material_buffer_data: Arc::new(RwLock::new(Vec::new())),
            shader_variants: Arc::new(RwLock::new(HashMap::new())),
            texture_manager,
            default_material: None,
            missing_material: None,
            shader_paths: vec![
                PathBuf::from("shaders/"),
                PathBuf::from("assets/shaders/"),
                PathBuf::from("data/shaders/"),
            ],
        })
    }

    /// Initialize default materials and shaders
    pub async fn initialize(&mut self) -> Result<()> {
        tracing::info!("Initializing default materials and shaders");

        // Create default PBR shader variant
        self.create_default_pbr_shader().await?;

        // Create default materials
        self.create_default_materials().await?;

        // Load built-in shader variants
        self.load_builtin_shaders().await?;

        tracing::info!("Default materials and shaders initialized");
        Ok(())
    }

    /// Create default PBR shader variant
    async fn create_default_pbr_shader(&mut self) -> Result<()> {
        let pbr_variant = W3DShaderVariant {
            name: "pbr_default".to_string(),
            vertex_shader: include_str!("../shaders/w3d_default.wgsl").to_string(),
            fragment_shader: include_str!("../shaders/w3d_default.wgsl").to_string(),
            geometry_shader: None,
            compute_shader: None,
            defines: HashMap::new(),
            vertex_layout: Self::create_default_vertex_layout(),
            render_state: RenderState::default(),
        };

        self.shader_variants
            .write()
            .insert("pbr_default".to_string(), pbr_variant);
        self.compile_shader("pbr_default").await?;

        Ok(())
    }

    /// Create default vertex layout
    fn create_default_vertex_layout() -> Vec<VertexAttribute> {
        use wgpu::{VertexAttribute, VertexFormat};

        vec![
            VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x3, // position
            },
            VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: VertexFormat::Float32x3, // normal
            },
            VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: VertexFormat::Float32x2, // tex_coords
            },
            VertexAttribute {
                offset: 32,
                shader_location: 3,
                format: VertexFormat::Float32x4, // color
            },
            VertexAttribute {
                offset: 48,
                shader_location: 4,
                format: VertexFormat::Uint32x4, // bone_indices
            },
            VertexAttribute {
                offset: 64,
                shader_location: 5,
                format: VertexFormat::Float32x4, // bone_weights
            },
        ]
    }

    /// Create default materials
    async fn create_default_materials(&mut self) -> Result<()> {
        // Default PBR material
        let default_material = W3DMaterial {
            name: "default".to_string(),
            display_name: "Default Material".to_string(),
            data: W3DMaterialData::default(),
            shader_variant: "pbr_default".to_string(),
            texture_paths: MaterialTextures::default(),
            render_queue: 2000,
            lod_bias: 0.0,
            transparent: false,
            cast_shadows: true,
            receive_shadows: true,
        };

        // Missing material (magenta for debugging)
        let mut missing_data = W3DMaterialData::default();
        missing_data.base_color = [1.0, 0.0, 1.0, 1.0]; // Magenta
        missing_data.metallic = 0.0;
        missing_data.roughness = 1.0;

        let missing_material = W3DMaterial {
            name: "missing".to_string(),
            display_name: "Missing Material".to_string(),
            data: missing_data,
            shader_variant: "pbr_default".to_string(),
            texture_paths: MaterialTextures::default(),
            render_queue: 2000,
            lod_bias: 0.0,
            transparent: false,
            cast_shadows: true,
            receive_shadows: true,
        };

        self.add_material(default_material).await?;
        self.add_material(missing_material).await?;

        self.default_material = Some("default".to_string());
        self.missing_material = Some("missing".to_string());

        Ok(())
    }

    /// Load built-in shader variants
    async fn load_builtin_shaders(&mut self) -> Result<()> {
        // Transparent shader variant
        let mut transparent_variant = self
            .shader_variants
            .read()
            .get("pbr_default")
            .unwrap()
            .clone();
        transparent_variant.name = "pbr_transparent".to_string();
        transparent_variant.render_state.blend_mode = BlendMode::Alpha;
        transparent_variant.render_state.depth_write = false;

        // Emissive shader variant
        let mut emissive_variant = transparent_variant.clone();
        emissive_variant.name = "pbr_emissive".to_string();
        emissive_variant
            .defines
            .insert("EMISSIVE".to_string(), "1".to_string());
        emissive_variant.render_state.blend_mode = BlendMode::Additive;

        // Skinned mesh variant
        let mut skinned_variant = self
            .shader_variants
            .read()
            .get("pbr_default")
            .unwrap()
            .clone();
        skinned_variant.name = "pbr_skinned".to_string();
        skinned_variant
            .defines
            .insert("SKINNED".to_string(), "1".to_string());

        // Add variants
        self.shader_variants
            .write()
            .insert(transparent_variant.name.clone(), transparent_variant);
        self.shader_variants
            .write()
            .insert(emissive_variant.name.clone(), emissive_variant);
        self.shader_variants
            .write()
            .insert(skinned_variant.name.clone(), skinned_variant);

        // Compile all variants
        self.compile_shader("pbr_transparent").await?;
        self.compile_shader("pbr_emissive").await?;
        self.compile_shader("pbr_skinned").await?;

        Ok(())
    }

    /// Add material to the system
    pub async fn add_material(&mut self, material: W3DMaterial) -> Result<String> {
        let name = material.name.clone();

        // Load textures for the material
        self.load_material_textures(&material).await?;

        // Add to material buffer
        let mut buffer_data = self.material_buffer_data.write();
        let material_index = buffer_data.len();
        buffer_data.push(material.data);
        self.material_indices
            .write()
            .insert(name.clone(), material_index);

        // Update GPU buffer
        #[cfg(feature = "w3d")]
        {
            let data_slice = bytemuck::cast_slice(&buffer_data[..]);
            self.queue
                .write_buffer(&self.material_buffer, 0, data_slice);
        }

        // Store material
        self.materials.write().insert(name.clone(), material);

        tracing::debug!("Added material '{}' at index {}", name, material_index);
        Ok(name)
    }

    /// Load textures for a material
    async fn load_material_textures(&self, material: &W3DMaterial) -> Result<()> {
        let textures = &material.texture_paths;

        // Load each texture type
        if let Some(path) = &textures.diffuse {
            self.texture_manager.load_texture(path).await?;
        }
        if let Some(path) = &textures.normal {
            self.texture_manager.load_texture(path).await?;
        }
        if let Some(path) = &textures.metallic_roughness {
            self.texture_manager.load_texture(path).await?;
        }
        if let Some(path) = &textures.occlusion {
            self.texture_manager.load_texture(path).await?;
        }
        if let Some(path) = &textures.emissive {
            self.texture_manager.load_texture(path).await?;
        }
        if let Some(path) = &textures.height {
            self.texture_manager.load_texture(path).await?;
        }

        Ok(())
    }

    /// Compile shader variant
    #[cfg(feature = "w3d")]
    async fn compile_shader(&mut self, variant_name: &str) -> Result<()> {
        let variant = self
            .shader_variants
            .read()
            .get(variant_name)
            .ok_or_else(|| {
                W3DError::ShaderCompilationFailed(format!(
                    "Shader variant '{}' not found",
                    variant_name
                ))
            })?
            .clone();

        tracing::debug!("Compiling shader variant: {}", variant_name);

        // Create shader modules
        let vertex_module = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some(&format!("{} Vertex", variant_name)),
            source: ShaderSource::Wgsl(variant.vertex_shader.into()),
        });

        let fragment_module = self.device.create_shader_module(ShaderModuleDescriptor {
            label: Some(&format!("{} Fragment", variant_name)),
            source: ShaderSource::Wgsl(variant.fragment_shader.into()),
        });

        // Create bind group layout
        let bind_group_layout = self
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some(&format!("{} Bind Group Layout", variant_name)),
                entries: &[
                    // Camera uniforms
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Material uniforms
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Texture array
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            multisampled: false,
                            view_dimension: TextureViewDimension::D2Array,
                            sample_type: TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    // Sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // Create pipeline layout
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some(&format!("{} Pipeline Layout", variant_name)),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        // Convert render state to wgpu types
        let blend_state = self.convert_blend_mode(&variant.render_state.blend_mode);
        let cull_mode = self.convert_cull_mode(&variant.render_state.cull_mode);
        let depth_compare = self.convert_depth_compare(&variant.render_state.depth_compare);

        // Create render pipeline
        let render_pipeline = self
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some(&format!("{} Render Pipeline", variant_name)),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &vertex_module,
                    entry_point: Some("vs_main"),
                    buffers: &[self.create_vertex_buffer_layout(&variant.vertex_layout)],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(FragmentState {
                    module: &fragment_module,
                    entry_point: Some("fs_main"),
                    targets: &[Some(ColorTargetState {
                        format: TextureFormat::Rgba8UnormSrgb, // Default format
                        blend: blend_state,
                        write_mask: ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: FrontFace::Ccw,
                    cull_mode,
                    unclipped_depth: false,
                    polygon_mode: PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: if variant.render_state.depth_test {
                    Some(DepthStencilState {
                        format: TextureFormat::Depth32Float,
                        depth_write_enabled: variant.render_state.depth_write,
                        depth_compare,
                        stencil: StencilState::default(),
                        bias: DepthBiasState::default(),
                    })
                } else {
                    None
                },
                multisample: MultisampleState::default(),
                cache: None,
                multiview: None,
            });

        let shader = W3DShader {
            name: variant_name.to_string(),
            vertex_module,
            fragment_module,
            geometry_module: None,
            compute_module: None,
            render_pipeline: Some(render_pipeline),
            compute_pipeline: None,
            bind_group_layout,
            pipeline_layout,
            vertex_attributes: variant
                .vertex_layout
                .into_iter()
                .enumerate()
                .map(|(i, attr)| wgpu::VertexAttribute {
                    offset: attr.offset,
                    shader_location: i as u32,
                    format: attr.format,
                })
                .collect(),
        };

        self.compiled_shaders
            .write()
            .insert(variant_name.to_string(), shader);

        tracing::info!("Successfully compiled shader variant: {}", variant_name);
        Ok(())
    }

    /// Convert blend mode to wgpu blend state
    #[cfg(feature = "w3d")]
    fn convert_blend_mode(&self, blend_mode: &BlendMode) -> Option<BlendState> {
        match blend_mode {
            BlendMode::Opaque => None,
            BlendMode::Alpha => Some(BlendState::ALPHA_BLENDING),
            BlendMode::Additive => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent::OVER,
            }),
            BlendMode::Multiply => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::Dst,
                    dst_factor: BlendFactor::Zero,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent::OVER,
            }),
            BlendMode::Custom {
                src_color,
                dst_color,
                color_op,
                src_alpha,
                dst_alpha,
                alpha_op,
            } => Some(BlendState {
                color: BlendComponent {
                    src_factor: Self::parse_blend_factor(src_color),
                    dst_factor: Self::parse_blend_factor(dst_color),
                    operation: Self::parse_blend_operation(color_op),
                },
                alpha: BlendComponent {
                    src_factor: Self::parse_blend_factor(src_alpha),
                    dst_factor: Self::parse_blend_factor(dst_alpha),
                    operation: Self::parse_blend_operation(alpha_op),
                },
            }),
        }
    }

    #[cfg(feature = "w3d")]
    fn parse_blend_factor(value: &str) -> BlendFactor {
        match value.trim().to_ascii_lowercase().as_str() {
            "zero" => BlendFactor::Zero,
            "one" => BlendFactor::One,
            "src" | "src_color" => BlendFactor::Src,
            "one_minus_src" | "one_minus_src_color" => BlendFactor::OneMinusSrc,
            "dst" | "dst_color" => BlendFactor::Dst,
            "one_minus_dst" | "one_minus_dst_color" => BlendFactor::OneMinusDst,
            "src_alpha" => BlendFactor::SrcAlpha,
            "one_minus_src_alpha" => BlendFactor::OneMinusSrcAlpha,
            "dst_alpha" => BlendFactor::DstAlpha,
            "one_minus_dst_alpha" => BlendFactor::OneMinusDstAlpha,
            "constant" | "const" => BlendFactor::Constant,
            "one_minus_constant" => BlendFactor::OneMinusConstant,
            "src_alpha_saturated" | "src_alpha_saturate" => BlendFactor::SrcAlphaSaturated,
            _ => BlendFactor::One,
        }
    }

    #[cfg(feature = "w3d")]
    fn parse_blend_operation(value: &str) -> BlendOperation {
        match value.trim().to_ascii_lowercase().as_str() {
            "add" => BlendOperation::Add,
            "subtract" => BlendOperation::Subtract,
            "reverse_subtract" | "rev_subtract" => BlendOperation::ReverseSubtract,
            "min" => BlendOperation::Min,
            "max" => BlendOperation::Max,
            _ => BlendOperation::Add,
        }
    }

    /// Convert cull mode string to wgpu cull mode
    #[cfg(feature = "w3d")]
    fn convert_cull_mode(&self, cull_mode: &str) -> Option<Face> {
        match cull_mode {
            "none" => None,
            "front" => Some(Face::Front),
            "back" => Some(Face::Back),
            _ => Some(Face::Back),
        }
    }

    /// Convert depth compare string to wgpu compare function
    #[cfg(feature = "w3d")]
    fn convert_depth_compare(&self, depth_compare: &str) -> CompareFunction {
        match depth_compare {
            "never" => CompareFunction::Never,
            "less" => CompareFunction::Less,
            "equal" => CompareFunction::Equal,
            "less_equal" => CompareFunction::LessEqual,
            "greater" => CompareFunction::Greater,
            "not_equal" => CompareFunction::NotEqual,
            "greater_equal" => CompareFunction::GreaterEqual,
            "always" => CompareFunction::Always,
            _ => CompareFunction::Less,
        }
    }

    /// Create vertex buffer layout from vertex attributes
    #[cfg(feature = "w3d")]
    fn create_vertex_buffer_layout(&self, attributes: &[VertexAttribute]) -> VertexBufferLayout {
        let wgpu_attributes: Vec<wgpu::VertexAttribute> = attributes
            .iter()
            .enumerate()
            .map(|(i, attr)| wgpu::VertexAttribute {
                offset: attr.offset,
                shader_location: i as u32,
                format: attr.format,
            })
            .collect();

        VertexBufferLayout {
            array_stride: attributes
                .last()
                .map(|attr| attr.offset + attr.format.size())
                .unwrap_or(0),
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu_attributes,
        }
    }

    /// Get material by name
    pub fn get_material(&self, name: &str) -> Option<W3DMaterial> {
        self.materials.read().get(name).cloned()
    }

    /// Get compiled shader
    #[cfg(feature = "w3d")]
    pub fn get_shader(&self, name: &str) -> Option<Arc<W3DShader>> {
        self.compiled_shaders
            .read()
            .get(name)
            .map(|shader| Arc::new(shader.clone()))
    }

    /// Update material parameters
    pub async fn update_material(&mut self, name: &str, data: W3DMaterialData) -> Result<()> {
        if let Some(material) = self.materials.write().get_mut(name) {
            material.data = data;

            // Find material index and update GPU buffer
            if let Some(index) = self.material_indices.read().get(name).copied() {
                #[cfg(feature = "w3d")]
                {
                    let offset = index * std::mem::size_of::<W3DMaterialData>();
                    self.queue.write_buffer(
                        &self.material_buffer,
                        offset as u64,
                        bytemuck::cast_slice(&[data]),
                    );
                }
            }

            Ok(())
        } else {
            Err(W3DError::ResourceError(format!(
                "Material '{}' not found",
                name
            )))
        }
    }

    /// Get texture manager
    pub fn get_texture_manager(&self) -> &Arc<W3DTextureManager> {
        &self.texture_manager
    }
}

/// Texture management system
pub struct W3DTextureManager {
    /// GPU device
    #[cfg(feature = "w3d")]
    device: Arc<Device>,
    /// GPU queue
    #[cfg(feature = "w3d")]
    queue: Arc<Queue>,

    /// Loaded textures
    textures: Arc<RwLock<HashMap<String, W3DTexture>>>,
    /// Cache of resolved texture paths keyed by normalized lookup name
    resolved_paths: Arc<RwLock<HashMap<String, PathBuf>>>,
    /// Negative cache to avoid repeated filesystem probes for missing textures
    missing_paths: Arc<RwLock<HashSet<String>>>,
    /// Maps normalized aliases/case variants to the canonical texture key
    alias_map: Arc<RwLock<HashMap<String, String>>>,
    /// Texture atlas
    #[cfg(feature = "w3d")]
    texture_atlas: Mutex<Option<Arc<Texture>>>,
    /// Texture atlas view
    #[cfg(feature = "w3d")]
    atlas_view: Mutex<Option<Arc<TextureView>>>,
    /// Next atlas layer index
    #[cfg(feature = "w3d")]
    atlas_layers_used: AtomicU32,
    /// Default sampler
    #[cfg(feature = "w3d")]
    default_sampler: Sampler,

    /// Texture search paths
    texture_paths: Vec<PathBuf>,
    /// Maximum texture size
    max_texture_size: u32,
    /// Generate mipmaps
    generate_mipmaps: bool,
}

/// Texture representation
#[derive(Debug, Clone)]
pub struct W3DTexture {
    /// Texture name
    pub name: String,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels  
    pub height: u32,
    /// Format
    pub format: ColorFormat,
    /// Mip levels
    pub mip_levels: u32,
    /// Atlas layer index
    pub atlas_layer: u32,
    /// Is compressed?
    pub compressed: bool,
    /// GPU texture handle
    #[cfg(feature = "w3d")]
    pub gpu_texture: Option<Arc<Texture>>,
    /// GPU texture view
    #[cfg(feature = "w3d")]
    pub gpu_view: Option<Arc<TextureView>>,
}

impl W3DTextureManager {
    const TEXTURE_EXTENSIONS: [&'static str; 5] = ["tga", "dds", "png", "jpg", "jpeg"];

    /// Create new texture manager
    #[cfg(feature = "w3d")]
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Result<Self> {
        let default_sampler = device.create_sampler(&SamplerDescriptor {
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
            anisotropy_clamp: 16,
            border_color: Some(SamplerBorderColor::TransparentBlack),
        });

        Ok(Self {
            device,
            queue,
            textures: Arc::new(RwLock::new(HashMap::new())),
            resolved_paths: Arc::new(RwLock::new(HashMap::new())),
            missing_paths: Arc::new(RwLock::new(HashSet::new())),
            alias_map: Arc::new(RwLock::new(HashMap::new())),
            texture_atlas: Mutex::new(None),
            atlas_view: Mutex::new(None),
            atlas_layers_used: AtomicU32::new(0),
            default_sampler,
            texture_paths: vec![
                PathBuf::from("textures/"),
                PathBuf::from("assets/textures/"),
                PathBuf::from("data/textures/"),
            ],
            max_texture_size: 2048,
            generate_mipmaps: true,
        })
    }

    /// Create new texture manager (no GPU)
    #[cfg(not(feature = "w3d"))]
    pub fn new() -> Result<Self> {
        Ok(Self {
            textures: Arc::new(RwLock::new(HashMap::new())),
            resolved_paths: Arc::new(RwLock::new(HashMap::new())),
            missing_paths: Arc::new(RwLock::new(HashSet::new())),
            alias_map: Arc::new(RwLock::new(HashMap::new())),
            texture_paths: vec![
                PathBuf::from("textures/"),
                PathBuf::from("assets/textures/"),
                PathBuf::from("data/textures/"),
            ],
            max_texture_size: 2048,
            generate_mipmaps: true,
        })
    }

    #[cfg(feature = "w3d")]
    fn ensure_texture_atlas(&self) -> Result<Arc<Texture>> {
        if let Ok(guard) = self.texture_atlas.lock() {
            if let Some(atlas) = guard.as_ref() {
                return Ok(Arc::clone(atlas));
            }
        }

        let atlas = self.device.create_texture(&TextureDescriptor {
            label: Some("W3D Texture Atlas"),
            size: Extent3d {
                width: self.max_texture_size,
                height: self.max_texture_size,
                depth_or_array_layers: MAX_TEXTURE_LAYERS as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = atlas.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(TextureViewDimension::D2Array),
            ..Default::default()
        });

        if let Ok(mut guard) = self.texture_atlas.lock() {
            *guard = Some(Arc::new(atlas));
        }
        if let Ok(mut guard) = self.atlas_view.lock() {
            *guard = Some(Arc::new(view));
        }

        self.texture_atlas
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(Arc::clone))
            .ok_or_else(|| {
                W3DError::ResourceError("Failed to initialize texture atlas".to_string())
            })
    }

    #[cfg(feature = "w3d")]
    fn allocate_atlas_layer(&self) -> Result<u32> {
        let layer = self.atlas_layers_used.fetch_add(1, Ordering::Relaxed);
        if layer as usize >= MAX_TEXTURE_LAYERS {
            return Err(W3DError::ResourceError(
                "Texture atlas layer limit reached".to_string(),
            ));
        }
        Ok(layer)
    }

    /// Load texture from file
    pub async fn load_texture(&self, path: &str) -> Result<String> {
        // Normalize input to share cache entries across case variants and separators
        let normalized = Self::normalize_lookup_key(path);

        // Fast path: alias/canonical already present and loaded
        if let Some(canonical) = self.alias_map.read().get(&normalized).cloned() {
            if self.textures.read().contains_key(&canonical) {
                return Ok(canonical);
            }
        }

        tracing::debug!("Loading texture: {}", path);

        // Resolve texture path once, recording positive/negative caches
        let (texture_path, canonical_key) = self.resolve_texture_path(path)?;

        // Another thread may have loaded after resolution
        if self.textures.read().contains_key(&canonical_key) {
            return Ok(canonical_key);
        }

        // Load image data
        let image_data = tokio::fs::read(&texture_path).await.map_err(|e| {
            W3DError::ResourceError(format!("Failed to read texture {}: {}", path, e))
        })?;

        // Decode image
        let extension = texture_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();
        let image = match extension.as_str() {
            "tga" => image::load_from_memory_with_format(&image_data, image::ImageFormat::Tga),
            "dds" => image::load_from_memory_with_format(&image_data, image::ImageFormat::Dds),
            _ => image::load_from_memory(&image_data),
        }
        .map_err(|e| {
            W3DError::ResourceError(format!("Failed to decode texture {}: {}", path, e))
        })?;

        let mut rgba = image.to_rgba8();
        let (mut width, mut height) = rgba.dimensions();

        if width > self.max_texture_size || height > self.max_texture_size {
            let scale_w = self.max_texture_size as f32 / width as f32;
            let scale_h = self.max_texture_size as f32 / height as f32;
            let scale = scale_w.min(scale_h).max(0.0);
            let new_width = (width as f32 * scale).round().max(1.0) as u32;
            let new_height = (height as f32 * scale).round().max(1.0) as u32;
            let resized = image::imageops::resize(
                &rgba,
                new_width,
                new_height,
                image::imageops::FilterType::Nearest,
            );
            rgba = resized;
            width = new_width;
            height = new_height;
        }

        #[cfg(feature = "w3d")]
        let atlas_layer = {
            let layer = self.allocate_atlas_layer()?;
            let atlas = self.ensure_texture_atlas()?;
            self.queue.write_texture(
                TexelCopyTextureInfo {
                    texture: &atlas,
                    mip_level: 0,
                    origin: Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: TextureAspect::All,
                },
                &rgba,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            layer
        };

        #[cfg(not(feature = "w3d"))]
        let atlas_layer = 0;

        // Create texture
        #[cfg(feature = "w3d")]
        let (gpu_texture, gpu_view) = {
            let texture = self.device.create_texture(&TextureDescriptor {
                label: Some(path),
                size: Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: if self.generate_mipmaps {
                    (width.min(height) as f32).log2().floor() as u32 + 1
                } else {
                    1
                },
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            });

            self.queue.write_texture(
                TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                &rgba,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            (Some(Arc::new(texture)), Some(Arc::new(view)))
        };

        #[cfg(not(feature = "w3d"))]
        let (gpu_texture, gpu_view) = (None, None);

        let texture = W3DTexture {
            name: canonical_key.clone(),
            width,
            height,
            format: ColorFormat::Rgba8,
            mip_levels: if self.generate_mipmaps {
                (width.min(height) as f32).log2().floor() as u32 + 1
            } else {
                1
            },
            atlas_layer,
            compressed: false,
            gpu_texture,
            gpu_view,
        };

        self.textures.write().insert(canonical_key.clone(), texture);

        tracing::info!("Loaded texture: {} ({}x{})", canonical_key, width, height);
        Ok(canonical_key)
    }

    /// Resolve texture path with normalized/negative caching
    fn resolve_texture_path(&self, path: &str) -> Result<(PathBuf, String)> {
        let normalized = Self::normalize_lookup_key(path);

        if let Some(cached) = self.resolved_paths.read().get(&normalized) {
            let key = Self::canonical_key_from_path(cached);
            return Ok((cached.clone(), key));
        }

        if self.missing_paths.read().contains(&normalized) {
            return Err(W3DError::ResourceError(format!(
                "Texture file not found: {}",
                path
            )));
        }

        let path_obj = Path::new(path);
        let mut resolved: Option<PathBuf> = None;

        // Try absolute path as-is
        if path_obj.is_absolute() {
            if let Some(found) = Self::probe_candidate(path_obj) {
                resolved = Some(found);
            }
        } else {
            'outer: for search_path in &self.texture_paths {
                let full_path = search_path.join(path_obj);
                if let Some(found) = Self::probe_candidate(&full_path) {
                    resolved = Some(found);
                    break 'outer;
                }

                for ext in Self::TEXTURE_EXTENSIONS {
                    let ext_path = full_path.with_extension(ext);
                    if let Some(found) = Self::probe_candidate(&ext_path) {
                        resolved = Some(found);
                        break 'outer;
                    }
                }
            }
        }

        match resolved {
            Some(found) => {
                let canonical_key = Self::canonical_key_from_path(&found);

                self.resolved_paths
                    .write()
                    .insert(normalized.clone(), found.clone());
                {
                    let mut alias_map = self.alias_map.write();
                    alias_map.insert(normalized.clone(), canonical_key.clone());
                    alias_map.insert(
                        Self::normalize_lookup_key(&canonical_key),
                        canonical_key.clone(),
                    );
                }
                self.missing_paths.write().remove(&normalized);

                Ok((found, canonical_key))
            }
            None => {
                self.missing_paths.write().insert(normalized);
                Err(W3DError::ResourceError(format!(
                    "Texture file not found: {}",
                    path
                )))
            }
        }
    }

    /// Try a single path without repeating filesystem probes on failure
    fn probe_candidate(path: &Path) -> Option<PathBuf> {
        path.try_exists()
            .ok()
            .filter(|exists| *exists)
            .map(|_| path.to_path_buf())
    }

    /// Normalize lookup key: lowercase + forward slashes
    fn normalize_lookup_key(path: &str) -> String {
        path.replace('\\', "/").to_lowercase()
    }

    /// Canonical key from resolved path to collapse aliases/case variants
    fn canonical_key_from_path(path: &Path) -> String {
        Self::normalize_lookup_key(&path.to_string_lossy())
    }

    /// Get texture by name
    pub fn get_texture(&self, name: &str) -> Option<W3DTexture> {
        let normalized = Self::normalize_lookup_key(name);
        let key = self
            .alias_map
            .read()
            .get(&normalized)
            .cloned()
            .unwrap_or(normalized);

        self.textures.read().get(&key).cloned()
    }

    /// Get default sampler
    #[cfg(feature = "w3d")]
    pub fn get_default_sampler(&self) -> &Sampler {
        &self.default_sampler
    }
}
