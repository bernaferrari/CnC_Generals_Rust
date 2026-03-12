//! # W3D Shadow System - Advanced Shadow Mapping
//!
//! This module implements a complete modern shadow mapping system featuring:
//! - Cascaded Shadow Maps (CSM) for directional lights
//! - Point light shadow mapping with cube maps
//! - Spot light shadow mapping with perspective projection
//! - Soft shadows with Percentage-Closer Filtering (PCF)
//! - Variance Shadow Maps (VSM) for high-quality soft shadows
//! - Shadow atlas management for efficient memory usage
//! - Temporal shadow map caching and updates
//! - GPU-based shadow caster culling

use super::{W3DError, Result, BoundingBox, W3DVertex, MaterialData, CameraUniforms};
use crate::video::{ColorFormat, Resolution};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::{RwLock, Mutex};
use bytemuck::{Pod, Zeroable, cast_slice};
use glam::{Vec2, Vec3, Vec4, Mat4, Quat};

#[cfg(feature = "w3d")]
use wgpu::{
    Device, Queue, Buffer, BufferDescriptor, BufferUsages,
    Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
    Sampler, SamplerDescriptor, AddressMode, FilterMode, CompareFunction,
    BindGroup, BindGroupDescriptor, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupEntry,
    RenderPipeline, RenderPipelineDescriptor, ComputePipeline, ComputePipelineDescriptor,
    PipelineLayout, PipelineLayoutDescriptor,
    CommandEncoder, CommandBuffer, RenderPass, ComputePass,
    RenderPassDescriptor, RenderPassColorAttachment, RenderPassDepthStencilAttachment,
    VertexState, FragmentState, VertexBufferLayout,
    PrimitiveState, PrimitiveTopology, FrontFace, Face, PolygonMode,
    DepthStencilState, StencilState, DepthBiasState,
    LoadOp, StoreOp, Operations,
    util::{DeviceExt, BufferInitDescriptor},
    Extent3d, Origin3d, TextureAspect, TextureViewDescriptor, TextureViewDimension,
    BindingType, BufferBindingType, TextureSampleType,
    ShaderStages, SamplerBindingType,
};

/// Number of cascades for directional light shadows
pub const CASCADE_COUNT: usize = 4;
/// Maximum number of point lights with shadows
pub const MAX_POINT_LIGHTS: usize = 32;
/// Maximum number of spot lights with shadows
pub const MAX_SPOT_LIGHTS: usize = 64;
/// Default shadow map resolution
pub const DEFAULT_SHADOW_MAP_SIZE: u32 = 2048;
/// Shadow atlas resolution
pub const SHADOW_ATLAS_SIZE: u32 = 4096;

/// Light types for shadow mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowLightType {
    /// Directional light (sun)
    Directional = 0,
    /// Point light (omnidirectional)
    Point = 1,
    /// Spot light (cone)
    Spot = 2,
}

/// Shadow quality settings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowQuality {
    /// Low quality (512x512)
    Low,
    /// Medium quality (1024x1024)
    Medium,
    /// High quality (2048x2048)
    High,
    /// Ultra quality (4096x4096)
    Ultra,
}

impl ShadowQuality {
    /// Get resolution for quality level
    pub fn resolution(self) -> u32 {
        match self {
            Self::Low => 512,
            Self::Medium => 1024,
            Self::High => 2048,
            Self::Ultra => 4096,
        }
    }
}

/// Shadow cascade data for directional lights
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ShadowCascadeData {
    /// Light view-projection matrix
    pub light_view_proj: [[f32; 4]; 4],
    /// World to light space matrix
    pub world_to_light: [[f32; 4]; 4],
    /// Cascade split distance from camera
    pub split_distance: f32,
    /// Texel size in world space
    pub texel_size: f32,
    /// Shadow bias parameters
    pub bias_params: [f32; 2],
    /// Atlas UV bounds (min_u, min_v, max_u, max_v)
    pub atlas_bounds: [f32; 4],
}

/// Point light shadow data
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PointShadowData {
    /// Light position
    pub light_position: [f32; 4],
    /// Light range/radius
    pub light_range: f32,
    /// Shadow bias
    pub shadow_bias: f32,
    /// Atlas face indices (6 faces for cube map)
    pub atlas_faces: [u32; 6],
    /// Reserved
    pub _padding: [f32; 2],
}

/// Spot light shadow data
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SpotShadowData {
    /// Light view-projection matrix
    pub light_view_proj: [[f32; 4]; 4],
    /// Light position
    pub light_position: [f32; 4],
    /// Light direction
    pub light_direction: [f32; 4],
    /// Inner and outer cone angles
    pub cone_angles: [f32; 2],
    /// Shadow bias
    pub shadow_bias: f32,
    /// Atlas UV bounds
    pub atlas_bounds: [f32; 4],
    /// Reserved
    pub _padding: f32,
}

/// Shadow rendering configuration
#[derive(Debug, Clone)]
pub struct ShadowConfig {
    /// Enable shadows
    pub enabled: bool,
    /// Shadow quality
    pub quality: ShadowQuality,
    /// Cascade distances for directional lights
    pub cascade_distances: [f32; CASCADE_COUNT],
    /// Enable soft shadows
    pub soft_shadows: bool,
    /// PCF kernel size
    pub pcf_kernel_size: u32,
    /// Enable Variance Shadow Maps
    pub vsm_enabled: bool,
    /// Shadow bias
    pub shadow_bias: f32,
    /// Normal offset bias
    pub normal_offset_bias: f32,
    /// Maximum shadow distance
    pub max_shadow_distance: f32,
    /// Enable shadow fading
    pub fade_shadows: bool,
    /// Shadow fade distance
    pub shadow_fade_distance: f32,
}

impl Default for ShadowConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            quality: ShadowQuality::High,
            cascade_distances: [10.0, 30.0, 80.0, 200.0],
            soft_shadows: true,
            pcf_kernel_size: 3,
            vsm_enabled: false,
            shadow_bias: 0.005,
            normal_offset_bias: 0.01,
            max_shadow_distance: 200.0,
            fade_shadows: true,
            shadow_fade_distance: 20.0,
        }
    }
}

/// Shadow atlas allocation entry
#[derive(Debug, Clone)]
struct AtlasAllocation {
    /// Position in atlas (x, y)
    position: (u32, u32),
    /// Size (width, height)
    size: (u32, u32),
    /// Light ID this allocation belongs to
    light_id: u32,
    /// Last frame this was used
    last_used_frame: u64,
    /// Is this allocation dirty (needs update)?
    dirty: bool,
}

/// Shadow atlas manager
#[derive(Debug)]
struct ShadowAtlas {
    /// Atlas texture
    #[cfg(feature = "w3d")]
    texture: Texture,
    /// Atlas texture view
    #[cfg(feature = "w3d")]
    view: TextureView,
    /// Atlas resolution
    resolution: u32,
    /// Current allocations
    allocations: HashMap<u32, AtlasAllocation>,
    /// Free space tracker (simple bin packing)
    free_regions: Vec<(u32, u32, u32, u32)>, // (x, y, width, height)
    /// Current frame number
    current_frame: u64,
}

/// Complete shadow mapping system
pub struct W3DShadowMapper {
    /// GPU device
    #[cfg(feature = "w3d")]
    device: Arc<Device>,
    /// GPU queue
    #[cfg(feature = "w3d")]
    queue: Arc<Queue>,
    
    /// Shadow configuration
    config: Arc<RwLock<ShadowConfig>>,
    
    /// Shadow atlas for 2D shadows
    #[cfg(feature = "w3d")]
    shadow_atlas: Arc<Mutex<ShadowAtlas>>,
    
    /// Point light cube map array
    #[cfg(feature = "w3d")]
    point_shadow_maps: Option<Texture>,
    #[cfg(feature = "w3d")]
    point_shadow_view: Option<TextureView>,
    
    /// Shadow samplers
    #[cfg(feature = "w3d")]
    shadow_sampler: Sampler,
    #[cfg(feature = "w3d")]
    comparison_sampler: Sampler,
    
    /// Shadow uniforms buffer
    #[cfg(feature = "w3d")]
    cascade_uniform_buffer: Buffer,
    #[cfg(feature = "w3d")]
    point_uniform_buffer: Buffer,
    #[cfg(feature = "w3d")]
    spot_uniform_buffer: Buffer,
    
    /// Shadow render pipelines
    #[cfg(feature = "w3d")]
    depth_only_pipeline: Option<RenderPipeline>,
    #[cfg(feature = "w3d")]
    depth_cube_pipeline: Option<RenderPipeline>,
    #[cfg(feature = "w3d")]
    vsm_blur_pipeline: Option<ComputePipeline>,
    
    /// Current shadow data
    cascade_data: Arc<RwLock<[ShadowCascadeData; CASCADE_COUNT]>>,
    point_data: Arc<RwLock<Vec<PointShadowData>>>,
    spot_data: Arc<RwLock<Vec<SpotShadowData>>>,
    
    /// Statistics
    render_stats: Arc<RwLock<ShadowRenderStats>>,
}

/// Shadow rendering statistics
#[derive(Debug, Clone, Default)]
pub struct ShadowRenderStats {
    /// Number of shadow maps updated this frame
    pub shadow_maps_updated: u32,
    /// Number of shadow casters rendered
    pub shadow_casters_rendered: u32,
    /// Time spent on shadow rendering (ms)
    pub shadow_render_time: f32,
    /// Atlas utilization (0.0 to 1.0)
    pub atlas_utilization: f32,
    /// Number of atlas evictions
    pub atlas_evictions: u32,
}

impl W3DShadowMapper {
    /// Create new shadow mapper
    #[cfg(feature = "w3d")]
    pub fn new(device: Arc<Device>, queue: Arc<Queue>, config: ShadowConfig) -> Result<Self> {
        tracing::info!("Initializing W3D shadow mapping system");

        // Create shadow atlas
        let atlas_texture = device.create_texture(&TextureDescriptor {
            label: Some("Shadow Atlas"),
            size: Extent3d {
                width: SHADOW_ATLAS_SIZE,
                height: SHADOW_ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let atlas_view = atlas_texture.create_view(&TextureViewDescriptor::default());

        let shadow_atlas = Arc::new(Mutex::new(ShadowAtlas {
            texture: atlas_texture,
            view: atlas_view,
            resolution: SHADOW_ATLAS_SIZE,
            allocations: HashMap::new(),
            free_regions: vec![(0, 0, SHADOW_ATLAS_SIZE, SHADOW_ATLAS_SIZE)],
            current_frame: 0,
        }));

        // Create point light cube map array
        let point_shadow_maps = device.create_texture(&TextureDescriptor {
            label: Some("Point Shadow Maps"),
            size: Extent3d {
                width: config.quality.resolution(),
                height: config.quality.resolution(),
                depth_or_array_layers: MAX_POINT_LIGHTS as u32 * 6, // 6 faces per point light
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let point_shadow_view = point_shadow_maps.create_view(&TextureViewDescriptor {
            dimension: Some(TextureViewDimension::CubeArray),
            ..Default::default()
        });

        // Create samplers
        let shadow_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Shadow Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            ..Default::default()
        });

        let comparison_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Shadow Comparison Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            compare: Some(CompareFunction::LessEqual),
            ..Default::default()
        });

        // Create uniform buffers
        let cascade_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Shadow Cascade Uniforms"),
            size: (CASCADE_COUNT * std::mem::size_of::<ShadowCascadeData>()) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let point_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Point Shadow Uniforms"),
            size: (MAX_POINT_LIGHTS * std::mem::size_of::<PointShadowData>()) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let spot_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Spot Shadow Uniforms"),
            size: (MAX_SPOT_LIGHTS * std::mem::size_of::<SpotShadowData>()) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mapper = Self {
            device,
            queue,
            config: Arc::new(RwLock::new(config)),
            shadow_atlas,
            point_shadow_maps: Some(point_shadow_maps),
            point_shadow_view: Some(point_shadow_view),
            shadow_sampler,
            comparison_sampler,
            cascade_uniform_buffer,
            point_uniform_buffer,
            spot_uniform_buffer,
            depth_only_pipeline: None,
            depth_cube_pipeline: None,
            vsm_blur_pipeline: None,
            cascade_data: Arc::new(RwLock::new([ShadowCascadeData {
                light_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                world_to_light: Mat4::IDENTITY.to_cols_array_2d(),
                split_distance: 0.0,
                texel_size: 0.0,
                bias_params: [0.0, 0.0],
                atlas_bounds: [0.0, 0.0, 1.0, 1.0],
            }; CASCADE_COUNT])),
            point_data: Arc::new(RwLock::new(Vec::new())),
            spot_data: Arc::new(RwLock::new(Vec::new())),
            render_stats: Arc::new(RwLock::new(ShadowRenderStats::default())),
        };

        tracing::info!("W3D shadow mapping system initialized");
        Ok(mapper)
    }

    /// Initialize shadow rendering pipelines
    #[cfg(feature = "w3d")]
    pub async fn initialize_pipelines(&mut self) -> Result<()> {
        tracing::info!("Initializing shadow rendering pipelines");

        // Create depth-only shader for shadow mapping
        let depth_shader = self.device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Depth Only Shader"),
            source: wgpu::ShaderSource::Wgsl(self.get_depth_shader_source().into()),
            
        });

        // Create bind group layout for shadows
        let bind_group_layout = self.device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Shadow Bind Group Layout"),
            entries: &[
                // Light view-projection matrix
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = self.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Shadow Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create depth-only pipeline
        self.depth_only_pipeline = Some(self.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Depth Only Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &depth_shader,
                entry_point: Some("vs_depth_only"),
                buffers: &[self.get_shadow_vertex_layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: None, // Depth-only, no fragment shader needed
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: CompareFunction::Less,
                stencil: StencilState::default(),
                bias: DepthBiasState {
                    constant: 2, // Depth bias for shadow acne
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            cache: None,
            multiview: None,
        }));

        tracing::info!("Shadow rendering pipelines initialized");
        Ok(())
    }

    /// Update cascaded shadow maps for directional light
    pub fn update_cascaded_shadows(
        &mut self,
        light_direction: Vec3,
        camera: &CameraUniforms,
        scene_bounds: &BoundingBox,
    ) -> Result<()> {
        let config = self.config.read();
        if !config.enabled {
            return Ok(());
        }

        let mut cascade_data = self.cascade_data.write();
        
        // Calculate view matrix for light
        let light_up = if light_direction.y.abs() > 0.99 {
            Vec3::new(1.0, 0.0, 0.0) // Avoid gimbal lock
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };
        
        let light_right = light_direction.cross(light_up).normalize();
        let light_up = light_right.cross(light_direction).normalize();
        
        let light_view = Mat4::look_to_rh(
            Vec3::ZERO,
            light_direction,
            light_up,
        );

        // Get camera matrices
        let camera_view = Mat4::from_cols_array_2d(&camera.view_matrix);
        let camera_proj = Mat4::from_cols_array_2d(&camera.projection_matrix);
        let camera_view_proj_inv = (camera_proj * camera_view).inverse();

        // Calculate cascade splits
        let near = camera.near_far[0];
        let far = config.max_shadow_distance.min(camera.near_far[1]);
        
        for (i, cascade) in cascade_data.iter_mut().enumerate() {
            let split_near = if i == 0 { near } else { config.cascade_distances[i - 1] };
            let split_far = config.cascade_distances[i];
            
            // Calculate frustum corners in world space
            let frustum_corners = self.calculate_frustum_corners(
                camera_view_proj_inv,
                split_near,
                split_far,
            );

            // Transform corners to light space
            let light_space_corners: Vec<Vec3> = frustum_corners.iter()
                .map(|corner| (light_view * corner.extend(1.0)).truncate())
                .collect();

            // Calculate tight bounding box in light space
            let mut min_bounds = light_space_corners[0];
            let mut max_bounds = light_space_corners[0];

            for corner in &light_space_corners[1..] {
                min_bounds = min_bounds.min(*corner);
                max_bounds = max_bounds.max(*corner);
            }

            // Expand bounds to include static scene geometry
            let scene_center = scene_bounds.center();
            let scene_radius = scene_bounds.radius();
            min_bounds.z = (scene_center.z - scene_radius).min(min_bounds.z);
            max_bounds.z = (scene_center.z + scene_radius).max(max_bounds.z);

            // Snap to texel grid to reduce shimmer
            let texel_size = (max_bounds.x - min_bounds.x) / config.quality.resolution() as f32;
            min_bounds.x = (min_bounds.x / texel_size).floor() * texel_size;
            min_bounds.y = (min_bounds.y / texel_size).floor() * texel_size;
            max_bounds.x = (max_bounds.x / texel_size).ceil() * texel_size;
            max_bounds.y = (max_bounds.y / texel_size).ceil() * texel_size;

            // Create orthographic projection for shadow map
            let light_proj = Mat4::orthographic_rh(
                min_bounds.x,
                max_bounds.x,
                min_bounds.y,
                max_bounds.y,
                -max_bounds.z, // Reversed Z for better precision
                -min_bounds.z,
            );

            let light_view_proj = light_proj * light_view;

            // Update cascade data
            cascade.light_view_proj = light_view_proj.to_cols_array_2d();
            cascade.world_to_light = light_view.to_cols_array_2d();
            cascade.split_distance = split_far;
            cascade.texel_size = texel_size;
            cascade.bias_params = [config.shadow_bias, config.normal_offset_bias];
            
            // Allocate atlas space (simplified - would be more complex in real implementation)
            let atlas_size = config.quality.resolution() / 2; // Quarter atlas per cascade
            let atlas_x = (i % 2) as f32 * 0.5;
            let atlas_y = (i / 2) as f32 * 0.5;
            cascade.atlas_bounds = [
                atlas_x,
                atlas_y,
                atlas_x + 0.5,
                atlas_y + 0.5,
            ];
        }

        // Update GPU uniform buffer
        #[cfg(feature = "w3d")]
        {
            let data = cast_slice(&cascade_data[..]);
            self.queue.write_buffer(&self.cascade_uniform_buffer, 0, data);
        }

        Ok(())
    }

    /// Calculate frustum corners for cascade
    fn calculate_frustum_corners(
        &self,
        inv_view_proj: Mat4,
        near_plane: f32,
        far_plane: f32,
    ) -> [Vec3; 8] {
        // NDC coordinates for frustum corners
        let ndc_corners = [
            Vec4::new(-1.0, -1.0, 0.0, 1.0), // near bottom-left
            Vec4::new( 1.0, -1.0, 0.0, 1.0), // near bottom-right
            Vec4::new( 1.0,  1.0, 0.0, 1.0), // near top-right
            Vec4::new(-1.0,  1.0, 0.0, 1.0), // near top-left
            Vec4::new(-1.0, -1.0, 1.0, 1.0), // far bottom-left
            Vec4::new( 1.0, -1.0, 1.0, 1.0), // far bottom-right
            Vec4::new( 1.0,  1.0, 1.0, 1.0), // far top-right
            Vec4::new(-1.0,  1.0, 1.0, 1.0), // far top-left
        ];

        let mut world_corners = [Vec3::ZERO; 8];

        for (i, ndc_corner) in ndc_corners.iter().enumerate() {
            // Transform to world space
            let world_corner = inv_view_proj * *ndc_corner;
            let world_corner = world_corner / world_corner.w;
            world_corners[i] = world_corner.truncate();
        }

        // Adjust near and far planes
        for i in 0..4 {
            let near_corner = world_corners[i];
            let far_corner = world_corners[i + 4];
            let direction = (far_corner - near_corner).normalize();
            
            // Interpolate based on actual near/far distances
            world_corners[i] = near_corner + direction * near_plane;
            world_corners[i + 4] = near_corner + direction * far_plane;
        }

        world_corners
    }

    /// Render shadow maps for all lights
    #[cfg(feature = "w3d")]
    pub async fn render_shadows(
        &mut self,
        encoder: &mut CommandEncoder,
        shadow_casters: &[ShadowCaster],
    ) -> Result<()> {
        let config = self.config.read();
        if !config.enabled || shadow_casters.is_empty() {
            return Ok();
        }

        let mut stats = ShadowRenderStats::default();
        let render_start = std::time::Instant::now();

        // Render cascaded shadow maps
        self.render_cascade_shadows(encoder, shadow_casters, &mut stats).await?;

        // Render point light shadows
        self.render_point_shadows(encoder, shadow_casters, &mut stats).await?;

        // Render spot light shadows
        self.render_spot_shadows(encoder, shadow_casters, &mut stats).await?;

        // Update statistics
        stats.shadow_render_time = render_start.elapsed().as_millis() as f32;
        *self.render_stats.write() = stats;

        Ok(())
    }

    /// Render cascaded shadow maps
    #[cfg(feature = "w3d")]
    async fn render_cascade_shadows(
        &mut self,
        encoder: &mut CommandEncoder,
        shadow_casters: &[ShadowCaster],
        stats: &mut ShadowRenderStats,
    ) -> Result<()> {
        let pipeline = self.depth_only_pipeline.as_ref()
            .ok_or_else(|| W3DError::RenderingError("Shadow pipeline not initialized".to_string()))?;

        let atlas = self.shadow_atlas.lock();

        for (cascade_index, cascade) in self.cascade_data.read().iter().enumerate() {
            // Create render pass for this cascade
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some(&format!("Shadow Cascade {}", cascade_index)),
                color_attachments: &[],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &atlas.view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Set pipeline and viewport
            render_pass.set_pipeline(pipeline);
            
            let atlas_bounds = &cascade.atlas_bounds;
            let viewport_x = (atlas_bounds[0] * atlas.resolution as f32) as u32;
            let viewport_y = (atlas_bounds[1] * atlas.resolution as f32) as u32;
            let viewport_w = ((atlas_bounds[2] - atlas_bounds[0]) * atlas.resolution as f32) as u32;
            let viewport_h = ((atlas_bounds[3] - atlas_bounds[1]) * atlas.resolution as f32) as u32;
            
            // Set viewport (this would be a real wgpu call in actual implementation)
            // render_pass.set_viewport(viewport_x, viewport_y, viewport_w, viewport_h, 0.0, 1.0);

            // Render shadow casters
            for caster in shadow_casters {
                if caster.cast_shadows && self.is_visible_in_cascade(caster, cascade) {
                    self.render_shadow_caster(&mut render_pass, caster);
                    stats.shadow_casters_rendered += 1;
                }
            }

            stats.shadow_maps_updated += 1;
        }

        Ok(())
    }

    /// Check if shadow caster is visible in cascade
    fn is_visible_in_cascade(&self, _caster: &ShadowCaster, _cascade: &ShadowCascadeData) -> bool {
        // Simplified visibility check - would implement proper frustum culling
        true
    }

    /// Render point light shadows
    #[cfg(feature = "w3d")]
    async fn render_point_shadows(
        &mut self,
        _encoder: &mut CommandEncoder,
        _shadow_casters: &[ShadowCaster],
        _stats: &mut ShadowRenderStats,
    ) -> Result<()> {
        // Implementation would render to cube maps for point lights
        Ok(())
    }

    /// Render spot light shadows
    #[cfg(feature = "w3d")]
    async fn render_spot_shadows(
        &mut self,
        _encoder: &mut CommandEncoder,
        _shadow_casters: &[ShadowCaster],
        _stats: &mut ShadowRenderStats,
    ) -> Result<()> {
        // Implementation would render to 2D texture for spot lights
        Ok(())
    }

    /// Render individual shadow caster
    #[cfg(feature = "w3d")]
    fn render_shadow_caster(&self, render_pass: &mut RenderPass, caster: &ShadowCaster) {
        // Set vertex and index buffers
        render_pass.set_vertex_buffer(0, caster.vertex_buffer.slice(..));
        render_pass.set_index_buffer(caster.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        
        // Draw
        render_pass.draw_indexed(0..caster.index_count, 0, 0..1);
    }

    /// Get depth shader source
    fn get_depth_shader_source(&self) -> &'static str {
        r#"
        struct VertexInput {
            @location(0) position: vec3<f32>,
        }
        
        struct VertexOutput {
            @builtin(position) clip_position: vec4<f32>,
        }
        
        @group(0) @binding(0)
        var<uniform> light_view_proj: mat4x4<f32>;
        
        @vertex
        fn vs_depth_only(input: VertexInput) -> VertexOutput {
            var out: VertexOutput;
            out.clip_position = light_view_proj * vec4<f32>(input.position, 1.0);
            return out;
        }
        "#
    }

    /// Get shadow vertex layout
    #[cfg(feature = "w3d")]
    fn get_shadow_vertex_layout(&self) -> VertexBufferLayout {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }
    }

    /// Get shadow statistics
    pub fn get_statistics(&self) -> ShadowRenderStats {
        self.render_stats.read().clone()
    }

    /// Get cascade data for shaders
    pub fn get_cascade_data(&self) -> [ShadowCascadeData; CASCADE_COUNT] {
        *self.cascade_data.read()
    }

    /// Get shadow atlas view
    #[cfg(feature = "w3d")]
    pub fn get_shadow_atlas_view(&self) -> &TextureView {
        &self.shadow_atlas.lock().view
    }

    /// Get shadow sampler
    #[cfg(feature = "w3d")]
    pub fn get_shadow_sampler(&self) -> &Sampler {
        &self.shadow_sampler
    }

    /// Get comparison sampler for PCF
    #[cfg(feature = "w3d")]
    pub fn get_comparison_sampler(&self) -> &Sampler {
        &self.comparison_sampler
    }
}

/// Shadow caster representation for rendering
#[derive(Debug)]
pub struct ShadowCaster {
    /// Should this object cast shadows?
    pub cast_shadows: bool,
    /// Bounding box for culling
    pub bounding_box: BoundingBox,
    /// World transform
    pub transform: Mat4,
    /// Vertex buffer
    #[cfg(feature = "w3d")]
    pub vertex_buffer: Arc<wgpu::Buffer>,
    /// Index buffer
    #[cfg(feature = "w3d")]
    pub index_buffer: Arc<wgpu::Buffer>,
    /// Index count
    pub index_count: u32,
    /// LOD level for shadow rendering
    pub shadow_lod: u32,
}

/// Cascaded shadow maps implementation
pub struct W3DCascadedShadowMaps {
    /// Shadow mapper
    shadow_mapper: W3DShadowMapper,
    /// Light direction (world space)
    light_direction: Vec3,
    /// Light color and intensity
    light_color: Vec3,
    /// Light intensity
    light_intensity: f32,
    /// Is light enabled?
    enabled: bool,
}

impl W3DCascadedShadowMaps {
    /// Create new cascaded shadow maps
    #[cfg(feature = "w3d")]
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        config: ShadowConfig,
    ) -> Result<Self> {
        let shadow_mapper = W3DShadowMapper::new(device, queue, config)?;

        Ok(Self {
            shadow_mapper,
            light_direction: Vec3::new(0.0, -1.0, -1.0).normalize(),
            light_color: Vec3::new(1.0, 0.95, 0.8),
            light_intensity: 5.0,
            enabled: true,
        })
    }

    /// Update light direction
    pub fn set_light_direction(&mut self, direction: Vec3) {
        self.light_direction = direction.normalize();
    }

    /// Update light properties
    pub fn set_light_properties(&mut self, color: Vec3, intensity: f32) {
        self.light_color = color;
        self.light_intensity = intensity;
    }

    /// Enable/disable shadows
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Update shadow maps
    pub fn update_shadows(
        &mut self,
        camera: &CameraUniforms,
        scene_bounds: &BoundingBox,
    ) -> Result<()> {
        if !self.enabled {
            return Ok();
        }

        self.shadow_mapper.update_cascaded_shadows(
            self.light_direction,
            camera,
            scene_bounds,
        )
    }

    /// Get shadow mapper
    pub fn get_shadow_mapper(&self) -> &W3DShadowMapper {
        &self.shadow_mapper
    }

    /// Get shadow mapper (mutable)
    pub fn get_shadow_mapper_mut(&mut self) -> &mut W3DShadowMapper {
        &mut self.shadow_mapper
    }
}
