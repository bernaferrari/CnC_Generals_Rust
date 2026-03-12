//! W3DProjectedShadow Module - Advanced Shadow Casting and Rendering System
//! 
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Shadow/W3DProjectedShadow.cpp
//! 
//! This module provides comprehensive shadow mapping, projected shadows, cascade shadow maps,
//! real-time shadow casting, and advanced shadow filtering techniques.

use cgmath::{Point3, Vector3, Vector4, Matrix4, SquareMatrix, InnerSpace, EuclideanSpace, Zero};
use wgpu::{
    Device, Queue, Buffer, BufferDescriptor, BufferUsages, Texture, TextureDescriptor,
    TextureUsages, TextureDimension, TextureFormat, Extent3d, TextureView, Sampler,
    SamplerDescriptor, AddressMode, FilterMode, CompareFunction, RenderPipeline,
    BindGroup, BindGroupDescriptor, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupEntry, ShaderModule, ShaderModuleDescriptor, ShaderSource,
    RenderPassDescriptor, CommandEncoder, RenderPassColorAttachment, Operations,
    LoadOp, StoreOp, Color, RenderPassDepthStencilAttachment,
};
use bytemuck::{Pod, Zeroable};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Duration,
};
use parking_lot::{RwLock, Mutex};
use smallvec::SmallVec;
use slotmap::{SlotMap, DefaultKey};
use anyhow::{Result, Context};
use thiserror::Error;
use game_network::NetworkInstant;

/// Shadow map resolutions
pub const SHADOW_MAP_SIZE_LOW: u32 = 512;
pub const SHADOW_MAP_SIZE_MEDIUM: u32 = 1024;
pub const SHADOW_MAP_SIZE_HIGH: u32 = 2048;
pub const SHADOW_MAP_SIZE_ULTRA: u32 = 4096;

/// Maximum number of shadow cascades
pub const MAX_CASCADE_COUNT: usize = 4;

/// Maximum number of simultaneous shadow casters
pub const MAX_SHADOW_CASTERS: usize = 8;

/// Projected shadow vertex buffer size
pub const SHADOW_VERTEX_BUFFER_SIZE: usize = 10000;

/// PCF (Percentage Closer Filtering) kernel sizes
pub const PCF_KERNEL_SIZES: [i32; 4] = [1, 2, 3, 4];

/// Shadow quality levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShadowQuality {
    Low = 0,
    Medium = 1,
    High = 2,
    Ultra = 3,
}

impl ShadowQuality {
    /// Get shadow map size for quality level
    pub fn get_shadow_map_size(self) -> u32 {
        match self {
            ShadowQuality::Low => SHADOW_MAP_SIZE_LOW,
            ShadowQuality::Medium => SHADOW_MAP_SIZE_MEDIUM,
            ShadowQuality::High => SHADOW_MAP_SIZE_HIGH,
            ShadowQuality::Ultra => SHADOW_MAP_SIZE_ULTRA,
        }
    }

    /// Get PCF kernel size for quality level
    pub fn get_pcf_kernel_size(self) -> i32 {
        PCF_KERNEL_SIZES[self as usize]
    }
}

/// Shadow type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShadowType {
    Directional,
    Point,
    Spot,
    Projected,
}

/// Shadow cascade information for directional lights
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ShadowCascade {
    pub view_matrix: Matrix4<f32>,
    pub projection_matrix: Matrix4<f32>,
    pub view_projection_matrix: Matrix4<f32>,
    pub split_distance: f32,
    pub texel_size: f32,
    pub bias: f32,
    pub normal_bias: f32,
}

impl Default for ShadowCascade {
    fn default() -> Self {
        Self {
            view_matrix: Matrix4::identity(),
            projection_matrix: Matrix4::identity(),
            view_projection_matrix: Matrix4::identity(),
            split_distance: 0.0,
            texel_size: 0.0,
            bias: 0.001,
            normal_bias: 0.01,
        }
    }
}

/// Light source for shadow casting
#[derive(Debug, Clone)]
pub struct ShadowLight {
    pub id: DefaultKey,
    pub light_type: ShadowType,
    pub position: Point3<f32>,
    pub direction: Vector3<f32>,
    pub color: Vector3<f32>,
    pub intensity: f32,
    pub range: f32,
    pub inner_cone_angle: f32,
    pub outer_cone_angle: f32,
    pub cast_shadows: bool,
    pub shadow_bias: f32,
    pub normal_bias: f32,
    pub shadow_map_size: u32,
    pub near_plane: f32,
    pub far_plane: f32,
}

impl Default for ShadowLight {
    fn default() -> Self {
        Self {
            id: DefaultKey::default(),
            light_type: ShadowType::Directional,
            position: Point3::origin(),
            direction: Vector3::new(0.0, -1.0, 0.0),
            color: Vector3::new(1.0, 1.0, 1.0),
            intensity: 1.0,
            range: 100.0,
            inner_cone_angle: 30.0,
            outer_cone_angle: 45.0,
            cast_shadows: true,
            shadow_bias: 0.001,
            normal_bias: 0.01,
            shadow_map_size: SHADOW_MAP_SIZE_MEDIUM,
            near_plane: 0.1,
            far_plane: 1000.0,
        }
    }
}

/// Shadow caster object information
#[derive(Debug, Clone)]
pub struct ShadowCaster {
    pub object_key: DefaultKey,
    pub transform: Matrix4<f32>,
    pub bounds: super::super::wthree_d_tree_buffer::AABB,
    pub mesh_id: u32,
    pub cast_shadows: bool,
    pub receive_shadows: bool,
    pub shadow_lod: u32,
}

/// Shadow map resource
#[derive(Debug)]
pub struct ShadowMap {
    pub texture: Texture,
    pub depth_view: TextureView,
    pub sampler: Sampler,
    pub size: u32,
    pub light_id: DefaultKey,
    pub view_matrix: Matrix4<f32>,
    pub projection_matrix: Matrix4<f32>,
    pub view_projection_matrix: Matrix4<f32>,
}

/// Cascade shadow map for directional lights
#[derive(Debug)]
pub struct CascadeShadowMap {
    pub cascades: SmallVec<[ShadowCascade; MAX_CASCADE_COUNT]>,
    pub shadow_maps: SmallVec<[ShadowMap; MAX_CASCADE_COUNT]>,
    pub light_id: DefaultKey,
    pub cascade_splits: SmallVec<[f32; MAX_CASCADE_COUNT + 1]>,
}

/// Projected shadow vertex for terrain and ground projection
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ProjectedShadowVertex {
    pub position: Vector3<f32>,
    pub uv: [f32; 2],
    pub alpha: f32,
}

/// Shadow rendering statistics
#[derive(Debug, Default)]
pub struct ShadowStats {
    pub total_casters: u32,
    pub rendered_casters: u32,
    pub culled_casters: u32,
    pub shadow_map_updates: u32,
    pub cascade_updates: u32,
    pub projected_shadows: u32,
}

/// Performance metrics for shadow system
#[derive(Debug, Default)]
pub struct ShadowMetrics {
    pub update_time: Duration,
    pub render_time: Duration,
    pub gpu_memory_used: u64,
    pub draw_calls: u32,
    pub stats: ShadowStats,
}

/// Main W3D Projected Shadow implementation
pub struct W3DProjectedShadow {
    // Core GPU resources
    device: Option<Arc<Device>>,
    queue: Option<Arc<Queue>>,
    
    // Shadow lights and casters
    lights: SlotMap<DefaultKey, ShadowLight>,
    shadow_casters: HashMap<DefaultKey, ShadowCaster>,
    
    // Shadow maps and resources
    shadow_maps: HashMap<DefaultKey, ShadowMap>,
    cascade_shadow_maps: HashMap<DefaultKey, CascadeShadowMap>,
    
    // Projected shadow resources
    projected_shadow_vertices: Vec<ProjectedShadowVertex>,
    projected_vertex_buffer: Option<Buffer>,
    projected_texture: Option<Texture>,
    
    // Rendering pipelines and resources
    shadow_render_pipeline: Option<RenderPipeline>,
    shadow_bind_group_layout: Option<BindGroupLayout>,
    shadow_uniform_buffer: Option<Buffer>,
    
    // CSM (Cascade Shadow Mapping) resources
    csm_uniform_buffer: Option<Buffer>,
    csm_bind_group: Option<BindGroup>,
    
    // Projected shadow pipeline
    projected_shadow_pipeline: Option<RenderPipeline>,
    projected_bind_group: Option<BindGroup>,
    
    // Configuration
    shadow_quality: ShadowQuality,
    enable_pcf: bool,
    enable_csm: bool,
    enable_projected_shadows: bool,
    shadow_fade_distance: f32,
    max_shadow_distance: f32,
    
    // Camera information for culling and CSM
    camera_position: Point3<f32>,
    camera_direction: Vector3<f32>,
    camera_near: f32,
    camera_far: f32,
    view_matrix: Matrix4<f32>,
    projection_matrix: Matrix4<f32>,
    
    // Performance tracking
    metrics: ShadowMetrics,
    last_update_time: NetworkInstant,
    frame_counter: u64,
    
    // Thread safety
    update_lock: Mutex<()>,
    render_lock: RwLock<()>,
    
    // State flags
    initialized: bool,
    needs_update: bool,
    debug_mode: bool,
}

/// Error types for shadow operations
#[derive(Error, Debug)]
pub enum ShadowError {
    #[error("Graphics device not available")]
    DeviceNotAvailable,
    #[error("Shadow map creation failed: {0}")]
    ShadowMapCreationFailed(String),
    #[error("Light not found: {0:?}")]
    LightNotFound(DefaultKey),
    #[error("Shadow caster not found: {0:?}")]
    CasterNotFound(DefaultKey),
    #[error("Pipeline creation failed: {0}")]
    PipelineCreationFailed(String),
    #[error("Buffer creation failed: {0}")]
    BufferCreationFailed(String),
    #[error("Texture creation failed: {0}")]
    TextureCreationFailed(String),
    #[error("Shader compilation failed: {0}")]
    ShaderCompilationFailed(String),
    #[error("Invalid shadow quality setting")]
    InvalidQuality,
    #[error("Cascade configuration error: {0}")]
    CascadeError(String),
}

impl Default for W3DProjectedShadow {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DProjectedShadow {
    /// Create new projected shadow system
    pub fn new() -> Self {
        Self {
            device: None,
            queue: None,
            lights: SlotMap::new(),
            shadow_casters: HashMap::new(),
            shadow_maps: HashMap::new(),
            cascade_shadow_maps: HashMap::new(),
            projected_shadow_vertices: Vec::with_capacity(SHADOW_VERTEX_BUFFER_SIZE),
            projected_vertex_buffer: None,
            projected_texture: None,
            shadow_render_pipeline: None,
            shadow_bind_group_layout: None,
            shadow_uniform_buffer: None,
            csm_uniform_buffer: None,
            csm_bind_group: None,
            projected_shadow_pipeline: None,
            projected_bind_group: None,
            shadow_quality: ShadowQuality::Medium,
            enable_pcf: true,
            enable_csm: true,
            enable_projected_shadows: true,
            shadow_fade_distance: 800.0,
            max_shadow_distance: 1000.0,
            camera_position: Point3::origin(),
            camera_direction: Vector3::new(0.0, 0.0, -1.0),
            camera_near: 0.1,
            camera_far: 1000.0,
            view_matrix: Matrix4::identity(),
            projection_matrix: Matrix4::identity(),
            metrics: ShadowMetrics::default(),
            last_update_time: NetworkInstant::now(),
            frame_counter: 0,
            update_lock: Mutex::new(()),
            render_lock: RwLock::new(()),
            initialized: false,
            needs_update: true,
            debug_mode: false,
        }
    }

    /// Initialize shadow system with GPU device and queue
    pub fn init(&mut self, device: Arc<Device>, queue: Arc<Queue>) -> Result<()> {
        let _lock = self.update_lock.lock();
        
        self.device = Some(device.clone());
        self.queue = Some(queue);
        
        // Create uniform buffers
        self.create_uniform_buffers()?;
        
        // Create render pipelines
        self.create_render_pipelines()?;
        
        // Create initial resources
        self.create_projected_shadow_resources()?;
        
        self.initialized = true;
        self.last_update_time = NetworkInstant::now();
        
        Ok(())
    }

    /// Create uniform buffers for shadow rendering
    fn create_uniform_buffers(&mut self) -> Result<()> {
        let device = self.device.as_ref().ok_or(ShadowError::DeviceNotAvailable)?;
        
        // Shadow uniform buffer for light matrices and parameters
        let shadow_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Shadow Uniform Buffer"),
            size: (MAX_SHADOW_CASTERS * std::mem::size_of::<Matrix4<f32>>() * 2) as u64, // view + projection matrices
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.shadow_uniform_buffer = Some(shadow_uniform_buffer);
        
        // CSM uniform buffer for cascade information
        let csm_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("CSM Uniform Buffer"),
            size: (MAX_CASCADE_COUNT * std::mem::size_of::<ShadowCascade>()) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.csm_uniform_buffer = Some(csm_uniform_buffer);
        
        Ok(())
    }

    /// Create render pipelines for shadow rendering
    fn create_render_pipelines(&mut self) -> Result<()> {
        let device = self.device.as_ref().ok_or(ShadowError::DeviceNotAvailable)?;
        
        // Shadow mapping vertex shader
        let shadow_vs_source = r#"
            struct VertexInput {
                @location(0) position: vec3<f32>,
                @location(1) normal: vec3<f32>,
            };
            
            struct VertexOutput {
                @builtin(position) position: vec4<f32>,
                @location(0) world_pos: vec3<f32>,
            };
            
            @group(0) @binding(0)
            var<uniform> light_view_proj: mat4x4<f32>;
            
            @group(0) @binding(1)
            var<uniform> model_matrix: mat4x4<f32>;
            
            @vertex
            fn vs_main(input: VertexInput) -> VertexOutput {
                let world_pos = model_matrix * vec4<f32>(input.position, 1.0);
                var output: VertexOutput;
                output.position = light_view_proj * world_pos;
                output.world_pos = world_pos.xyz;
                return output;
            }
        "#;
        
        // Shadow mapping fragment shader (depth only)
        let shadow_fs_source = r#"
            @fragment
            fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
                return vec4<f32>(input.world_pos.z, 0.0, 0.0, 1.0);
            }
        "#;
        
        let shadow_vs_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shadow Vertex Shader"),
            source: ShaderSource::Wgsl(shadow_vs_source.into()),
            
        });
        
        let shadow_fs_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shadow Fragment Shader"),
            source: ShaderSource::Wgsl(shadow_fs_source.into()),
            
        });
        
        // Create bind group layout for shadow rendering
        let shadow_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Shadow Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        self.shadow_bind_group_layout = Some(shadow_bind_group_layout);
        
        // TODO: Complete pipeline creation with proper vertex layout and depth testing
        
        Ok(())
    }

    /// Create resources for projected shadow rendering
    fn create_projected_shadow_resources(&mut self) -> Result<()> {
        let device = self.device.as_ref().ok_or(ShadowError::DeviceNotAvailable)?;
        
        // Create projected shadow texture
        let projected_texture = device.create_texture(&TextureDescriptor {
            label: Some("Projected Shadow Texture"),
            size: Extent3d {
                width: 512,
                height: 512,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.projected_texture = Some(projected_texture);
        
        // Create projected shadow vertex buffer
        if !self.projected_shadow_vertices.is_empty() {
            let vertex_buffer = device.create_buffer(&BufferDescriptor {
                label: Some("Projected Shadow Vertex Buffer"),
                size: (self.projected_shadow_vertices.len() * std::mem::size_of::<ProjectedShadowVertex>()) as u64,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.projected_vertex_buffer = Some(vertex_buffer);
        }
        
        Ok(())
    }

    /// Add shadow casting light
    pub fn add_light(&mut self, light: ShadowLight) -> Result<DefaultKey> {
        let _lock = self.update_lock.lock();
        
        let light_id = self.lights.insert(light.clone());
        
        // Create shadow map for the light
        if light.cast_shadows {
            self.create_shadow_map(light_id, &light)?;
        }
        
        self.needs_update = true;
        Ok(light_id)
    }

    /// Create shadow map for a light
    fn create_shadow_map(&mut self, light_id: DefaultKey, light: &ShadowLight) -> Result<()> {
        let device = self.device.as_ref().ok_or(ShadowError::DeviceNotAvailable)?;
        
        let shadow_map_size = light.shadow_map_size;
        
        // Create depth texture for shadow map
        let depth_texture = device.create_texture(&TextureDescriptor {
            label: Some("Shadow Map Depth Texture"),
            size: Extent3d {
                width: shadow_map_size,
                height: shadow_map_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        // Create shadow map sampler with comparison
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Shadow Map Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            compare: Some(CompareFunction::LessEqual),
            ..Default::default()
        });
        
        // Calculate light matrices
        let (view_matrix, projection_matrix) = self.calculate_light_matrices(light)?;
        let view_projection_matrix = projection_matrix * view_matrix;
        
        let shadow_map = ShadowMap {
            texture: depth_texture,
            depth_view,
            sampler,
            size: shadow_map_size,
            light_id,
            view_matrix,
            projection_matrix,
            view_projection_matrix,
        };
        
        self.shadow_maps.insert(light_id, shadow_map);
        
        // Create cascade shadow map for directional lights
        if light.light_type == ShadowType::Directional && self.enable_csm {
            self.create_cascade_shadow_map(light_id, light)?;
        }
        
        Ok(())
    }

    /// Calculate light view and projection matrices
    fn calculate_light_matrices(&self, light: &ShadowLight) -> Result<(Matrix4<f32>, Matrix4<f32>)> {
        let view_matrix = match light.light_type {
            ShadowType::Directional => {
                // For directional lights, position is irrelevant, only direction matters
                let up = if light.direction.dot(Vector3::unit_y()).abs() > 0.9 {
                    Vector3::unit_z()
                } else {
                    Vector3::unit_y()
                };
                Matrix4::look_to_rh(light.position, light.direction, up)
            }
            ShadowType::Point => {
                // For point lights, we'd need 6 faces for cube mapping
                // For now, use a simple view matrix
                let up = Vector3::unit_y();
                Matrix4::look_to_rh(light.position, light.direction, up)
            }
            ShadowType::Spot => {
                let up = Vector3::unit_y();
                Matrix4::look_to_rh(light.position, light.direction, up)
            }
            ShadowType::Projected => {
                let up = Vector3::unit_y();
                Matrix4::look_to_rh(light.position, light.direction, up)
            }
        };
        
        let projection_matrix = match light.light_type {
            ShadowType::Directional => {
                // Use orthographic projection for directional lights
                let size = 100.0; // Should be calculated based on scene bounds
                cgmath::ortho(-size, size, -size, size, light.near_plane, light.far_plane)
            }
            ShadowType::Point => {
                // Use perspective projection for point lights
                cgmath::perspective(cgmath::Deg(90.0), 1.0, light.near_plane, light.range)
            }
            ShadowType::Spot => {
                // Use perspective projection for spot lights
                cgmath::perspective(cgmath::Deg(light.outer_cone_angle * 2.0), 1.0, light.near_plane, light.range)
            }
            ShadowType::Projected => {
                // Custom projection for projected shadows
                cgmath::perspective(cgmath::Deg(60.0), 1.0, light.near_plane, light.range)
            }
        };
        
        Ok((view_matrix, projection_matrix))
    }

    /// Create cascade shadow map for directional light
    fn create_cascade_shadow_map(&mut self, light_id: DefaultKey, light: &ShadowLight) -> Result<()> {
        let device = self.device.as_ref().ok_or(ShadowError::DeviceNotAvailable)?;
        
        let cascade_count = 4; // Standard 4-cascade CSM
        let shadow_map_size = light.shadow_map_size;
        
        // Calculate cascade splits
        let mut cascade_splits = SmallVec::new();
        cascade_splits.push(self.camera_near);
        
        for i in 1..cascade_count {
            let ratio = i as f32 / cascade_count as f32;
            // Use logarithmic split scheme for better distribution
            let log_split = self.camera_near * (self.camera_far / self.camera_near).powf(ratio);
            let uniform_split = self.camera_near + (self.camera_far - self.camera_near) * ratio;
            let split = log_split.lerp(uniform_split, 0.5);
            cascade_splits.push(split);
        }
        cascade_splits.push(self.camera_far);
        
        let mut cascades = SmallVec::new();
        let mut shadow_maps = SmallVec::new();
        
        for i in 0..cascade_count {
            // Calculate cascade bounds
            let near_dist = cascade_splits[i];
            let far_dist = cascade_splits[i + 1];
            
            // Create cascade shadow map
            let depth_texture = device.create_texture(&TextureDescriptor {
                label: Some(&format!("CSM Cascade {} Depth Texture", i)),
                size: Extent3d {
                    width: shadow_map_size,
                    height: shadow_map_size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Depth32Float,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            
            let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
            
            let sampler = device.create_sampler(&SamplerDescriptor {
                label: Some(&format!("CSM Cascade {} Sampler", i)),
                address_mode_u: AddressMode::ClampToEdge,
                address_mode_v: AddressMode::ClampToEdge,
                address_mode_w: AddressMode::ClampToEdge,
                mag_filter: FilterMode::Linear,
                min_filter: FilterMode::Linear,
                mipmap_filter: FilterMode::Nearest,
                compare: Some(CompareFunction::LessEqual),
                ..Default::default()
            });
            
            // Calculate cascade matrices (simplified)
            let (view_matrix, projection_matrix) = self.calculate_cascade_matrices(light, near_dist, far_dist)?;
            let view_projection_matrix = projection_matrix * view_matrix;
            
            let cascade = ShadowCascade {
                view_matrix,
                projection_matrix,
                view_projection_matrix,
                split_distance: far_dist,
                texel_size: 1.0 / shadow_map_size as f32,
                bias: light.shadow_bias,
                normal_bias: light.normal_bias,
            };
            
            let shadow_map = ShadowMap {
                texture: depth_texture,
                depth_view,
                sampler,
                size: shadow_map_size,
                light_id,
                view_matrix,
                projection_matrix,
                view_projection_matrix,
            };
            
            cascades.push(cascade);
            shadow_maps.push(shadow_map);
        }
        
        let cascade_shadow_map = CascadeShadowMap {
            cascades,
            shadow_maps,
            light_id,
            cascade_splits,
        };
        
        self.cascade_shadow_maps.insert(light_id, cascade_shadow_map);
        Ok(())
    }

    /// Calculate cascade matrices for CSM
    fn calculate_cascade_matrices(&self, light: &ShadowLight, near_dist: f32, far_dist: f32) -> Result<(Matrix4<f32>, Matrix4<f32>)> {
        // For now, use the same light matrices as regular shadow mapping
        // In a full implementation, this would calculate optimal bounds for each cascade
        // based on the camera frustum and light direction
        self.calculate_light_matrices(light)
    }

    /// Add shadow caster object
    pub fn add_shadow_caster(&mut self, caster: ShadowCaster) -> Result<()> {
        let _lock = self.update_lock.lock();
        
        self.shadow_casters.insert(caster.object_key, caster);
        self.needs_update = true;
        Ok(())
    }

    /// Remove shadow caster
    pub fn remove_shadow_caster(&mut self, object_key: DefaultKey) -> Result<()> {
        let _lock = self.update_lock.lock();
        
        if self.shadow_casters.remove(&object_key).is_none() {
            return Err(ShadowError::CasterNotFound(object_key));
        }
        
        self.needs_update = true;
        Ok(())
    }

    /// Update shadow system with camera information
    pub fn update(
        &mut self,
        camera_position: Point3<f32>,
        camera_direction: Vector3<f32>,
        view_matrix: Matrix4<f32>,
        projection_matrix: Matrix4<f32>,
    ) -> Result<()> {
        let start_time = NetworkInstant::now();
        let _lock = self.update_lock.lock();
        
        self.camera_position = camera_position;
        self.camera_direction = camera_direction;
        self.view_matrix = view_matrix;
        self.projection_matrix = projection_matrix;
        self.frame_counter += 1;
        
        // Reset metrics
        self.metrics.stats = ShadowStats::default();
        
        // Update shadow caster visibility and LOD
        self.update_shadow_casters()?;
        
        // Update projected shadows
        if self.enable_projected_shadows {
            self.update_projected_shadows()?;
        }
        
        // Update light matrices and cascade splits
        self.update_light_matrices()?;
        
        // Update uniform buffers
        self.update_uniform_buffers()?;
        
        self.metrics.update_time = start_time.elapsed();
        self.last_update_time = NetworkInstant::now();
        self.needs_update = false;
        
        Ok(())
    }

    /// Update shadow caster visibility and culling
    fn update_shadow_casters(&mut self) -> Result<()> {
        for caster in self.shadow_casters.values_mut() {
            // Calculate distance from camera
            let distance = (caster.bounds.center() - self.camera_position).magnitude();
            
            // Distance culling
            if distance > self.max_shadow_distance {
                caster.cast_shadows = false;
                self.metrics.stats.culled_casters += 1;
            } else {
                caster.cast_shadows = true;
                self.metrics.stats.rendered_casters += 1;
            }
            
            // Update shadow LOD based on distance
            caster.shadow_lod = if distance < 50.0 {
                0 // High detail
            } else if distance < 200.0 {
                1 // Medium detail
            } else {
                2 // Low detail
            };
            
            self.metrics.stats.total_casters += 1;
        }
        
        Ok(())
    }

    /// Update projected shadows (blob shadows on terrain)
    fn update_projected_shadows(&mut self) -> Result<()> {
        self.projected_shadow_vertices.clear();
        
        for caster in self.shadow_casters.values() {
            if !caster.cast_shadows {
                continue;
            }
            
            // Generate projected shadow vertices for terrain rendering
            // This is simplified - in practice would involve ray casting to terrain
            let center = caster.bounds.center();
            let size = caster.bounds.extents().magnitude() * 2.0;
            
            // Create quad for projected shadow
            let vertices = [
                ProjectedShadowVertex {
                    position: Vector3::new(center.x - size, 0.1, center.z - size),
                    uv: [0.0, 0.0],
                    alpha: 0.5,
                },
                ProjectedShadowVertex {
                    position: Vector3::new(center.x + size, 0.1, center.z - size),
                    uv: [1.0, 0.0],
                    alpha: 0.5,
                },
                ProjectedShadowVertex {
                    position: Vector3::new(center.x + size, 0.1, center.z + size),
                    uv: [1.0, 1.0],
                    alpha: 0.5,
                },
                ProjectedShadowVertex {
                    position: Vector3::new(center.x - size, 0.1, center.z + size),
                    uv: [0.0, 1.0],
                    alpha: 0.5,
                },
            ];
            
            self.projected_shadow_vertices.extend_from_slice(&vertices);
            self.metrics.stats.projected_shadows += 1;
        }
        
        // Update vertex buffer
        if !self.projected_shadow_vertices.is_empty() {
            self.update_projected_vertex_buffer()?;
        }
        
        Ok(())
    }

    /// Update projected shadow vertex buffer
    fn update_projected_vertex_buffer(&mut self) -> Result<()> {
        let device = self.device.as_ref().ok_or(ShadowError::DeviceNotAvailable)?;
        let queue = self.queue.as_ref().ok_or(ShadowError::DeviceNotAvailable)?;
        
        if self.projected_vertex_buffer.is_none() {
            let vertex_buffer = device.create_buffer(&BufferDescriptor {
                label: Some("Projected Shadow Vertex Buffer"),
                size: (SHADOW_VERTEX_BUFFER_SIZE * std::mem::size_of::<ProjectedShadowVertex>()) as u64,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.projected_vertex_buffer = Some(vertex_buffer);
        }
        
        if let Some(ref buffer) = self.projected_vertex_buffer {
            let data = bytemuck::cast_slice(&self.projected_shadow_vertices);
            queue.write_buffer(buffer, 0, data);
        }
        
        Ok(())
    }

    /// Update light matrices for shadow mapping
    fn update_light_matrices(&mut self) -> Result<()> {
        for (light_id, light) in &self.lights {
            if let Some(shadow_map) = self.shadow_maps.get_mut(&light_id) {
                let (view_matrix, projection_matrix) = self.calculate_light_matrices(light)?;
                shadow_map.view_matrix = view_matrix;
                shadow_map.projection_matrix = projection_matrix;
                shadow_map.view_projection_matrix = projection_matrix * view_matrix;
            }
            
            // Update cascade shadow maps
            if let Some(csm) = self.cascade_shadow_maps.get_mut(&light_id) {
                for (i, cascade) in csm.cascades.iter_mut().enumerate() {
                    let near_dist = csm.cascade_splits[i];
                    let far_dist = csm.cascade_splits[i + 1];
                    let (view_matrix, projection_matrix) = self.calculate_cascade_matrices(light, near_dist, far_dist)?;
                    
                    cascade.view_matrix = view_matrix;
                    cascade.projection_matrix = projection_matrix;
                    cascade.view_projection_matrix = projection_matrix * view_matrix;
                    cascade.split_distance = far_dist;
                }
            }
        }
        
        Ok(())
    }

    /// Update uniform buffers with current shadow data
    fn update_uniform_buffers(&mut self) -> Result<()> {
        let queue = self.queue.as_ref().ok_or(ShadowError::DeviceNotAvailable)?;
        
        // Update shadow uniform buffer
        if let Some(ref buffer) = self.shadow_uniform_buffer {
            let mut matrices = Vec::new();
            for shadow_map in self.shadow_maps.values() {
                matrices.push(shadow_map.view_projection_matrix);
            }
            
            if !matrices.is_empty() {
                let data = bytemuck::cast_slice(&matrices);
                queue.write_buffer(buffer, 0, data);
            }
        }
        
        // Update CSM uniform buffer
        if let Some(ref buffer) = self.csm_uniform_buffer {
            let mut cascades = Vec::new();
            for csm in self.cascade_shadow_maps.values() {
                cascades.extend_from_slice(&csm.cascades);
            }
            
            if !cascades.is_empty() {
                let data = bytemuck::cast_slice(&cascades);
                queue.write_buffer(buffer, 0, data);
            }
        }
        
        Ok(())
    }

    /// Set shadow quality level
    pub fn set_shadow_quality(&mut self, quality: ShadowQuality) -> Result<()> {
        if self.shadow_quality != quality {
            self.shadow_quality = quality;
            
            // Recreate shadow maps with new resolution
            for (light_id, light) in &self.lights {
                if light.cast_shadows {
                    let mut updated_light = light.clone();
                    updated_light.shadow_map_size = quality.get_shadow_map_size();
                    self.create_shadow_map(light_id, &updated_light)?;
                }
            }
            
            self.needs_update = true;
        }
        
        Ok(())
    }

    /// Enable or disable PCF filtering
    pub fn set_pcf_enabled(&mut self, enabled: bool) {
        self.enable_pcf = enabled;
        self.needs_update = true;
    }

    /// Enable or disable cascade shadow mapping
    pub fn set_csm_enabled(&mut self, enabled: bool) {
        self.enable_csm = enabled;
        self.needs_update = true;
    }

    /// Enable or disable projected shadows
    pub fn set_projected_shadows_enabled(&mut self, enabled: bool) {
        self.enable_projected_shadows = enabled;
        self.needs_update = true;
    }

    /// Set maximum shadow distance
    pub fn set_max_shadow_distance(&mut self, distance: f32) {
        self.max_shadow_distance = distance;
    }

    /// Set shadow fade distance
    pub fn set_shadow_fade_distance(&mut self, distance: f32) {
        self.shadow_fade_distance = distance;
    }

    /// Get shadow map for light
    pub fn get_shadow_map(&self, light_id: DefaultKey) -> Option<&ShadowMap> {
        self.shadow_maps.get(&light_id)
    }

    /// Get cascade shadow map for light
    pub fn get_cascade_shadow_map(&self, light_id: DefaultKey) -> Option<&CascadeShadowMap> {
        self.cascade_shadow_maps.get(&light_id)
    }

    /// Get projected shadow vertices
    pub fn get_projected_shadow_vertices(&self) -> &[ProjectedShadowVertex] {
        &self.projected_shadow_vertices
    }

    /// Get projected vertex buffer
    pub fn get_projected_vertex_buffer(&self) -> Option<&Buffer> {
        self.projected_vertex_buffer.as_ref()
    }

    /// Get current performance metrics
    pub fn get_metrics(&self) -> &ShadowMetrics {
        &self.metrics
    }

    /// Check if system is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Enable or disable debug mode
    pub fn set_debug_mode(&mut self, enabled: bool) {
        self.debug_mode = enabled;
    }

    /// Clear all shadow data
    pub fn clear(&mut self) {
        let _lock = self.update_lock.lock();
        
        self.lights.clear();
        self.shadow_casters.clear();
        self.shadow_maps.clear();
        self.cascade_shadow_maps.clear();
        self.projected_shadow_vertices.clear();
        self.needs_update = true;
    }
}

// Thread-safe implementation
unsafe impl Send for W3DProjectedShadow {}
unsafe impl Sync for W3DProjectedShadow {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shadow_system_creation() {
        let shadow_system = W3DProjectedShadow::new();
        assert!(!shadow_system.is_initialized());
        assert_eq!(shadow_system.shadow_quality, ShadowQuality::Medium);
        assert!(shadow_system.enable_pcf);
        assert!(shadow_system.enable_csm);
        assert!(shadow_system.enable_projected_shadows);
    }

    #[test]
    fn test_shadow_quality_settings() {
        assert_eq!(ShadowQuality::Low.get_shadow_map_size(), SHADOW_MAP_SIZE_LOW);
        assert_eq!(ShadowQuality::Medium.get_shadow_map_size(), SHADOW_MAP_SIZE_MEDIUM);
        assert_eq!(ShadowQuality::High.get_shadow_map_size(), SHADOW_MAP_SIZE_HIGH);
        assert_eq!(ShadowQuality::Ultra.get_shadow_map_size(), SHADOW_MAP_SIZE_ULTRA);
    }

    #[test]
    fn test_shadow_light_creation() {
        let light = ShadowLight::default();
        assert_eq!(light.light_type, ShadowType::Directional);
        assert_eq!(light.intensity, 1.0);
        assert!(light.cast_shadows);
        assert_eq!(light.shadow_map_size, SHADOW_MAP_SIZE_MEDIUM);
    }

    #[test]
    fn test_projected_shadow_vertex() {
        let vertex = ProjectedShadowVertex {
            position: Vector3::new(0.0, 0.0, 0.0),
            uv: [0.5, 0.5],
            alpha: 0.8,
        };
        assert_eq!(vertex.position, Vector3::new(0.0, 0.0, 0.0));
        assert_eq!(vertex.uv, [0.5, 0.5]);
        assert_eq!(vertex.alpha, 0.8);
    }

    #[test]
    fn test_shadow_cascade_default() {
        let cascade = ShadowCascade::default();
        assert_eq!(cascade.view_matrix, Matrix4::identity());
        assert_eq!(cascade.projection_matrix, Matrix4::identity());
        assert_eq!(cascade.split_distance, 0.0);
        assert_eq!(cascade.bias, 0.001);
        assert_eq!(cascade.normal_bias, 0.01);
    }
}
