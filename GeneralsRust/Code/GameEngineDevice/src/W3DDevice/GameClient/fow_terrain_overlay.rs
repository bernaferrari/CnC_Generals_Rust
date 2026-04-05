//! GPU-Accelerated Fog of War Terrain Overlay Renderer
//!
//! This module implements hardware-accelerated rendering of the fog-of-war effect
//! on the 3D terrain, using wgpu for cross-platform GPU rendering.
//!
//! ## Architecture
//!
//! ```text
//! ShroudManager (CPU) → FOW Texture (GPU) → Terrain Shader → Screen
//!     ↓                         ↓                  ↓
//! Grid state           RGBA8 texture      Fragment shader
//! (Hidden/Explored/    (256x256 or        (blend overlay
//!  Visible)             map resolution)     with terrain)
//! ```
//!
//! ## C++ Reference
//!
//! Ports behavior from:
//! - `W3DShroud.cpp` - Original shroud rendering
//! - `TerrainRenderObject.cpp` - Terrain texture blending
//! - `Display.cpp` - setShroudLevel() calls

use std::sync::Arc;
use wgpu::{
    include_wgsl, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendComponent, BlendFactor,
    BlendOperation, BlendState, Buffer, BufferBindingType, BufferDescriptor, BufferUsages,
    ColorTargetState, ColorWrites, CompareFunction, DepthBiasState, DepthStencilState, Device,
    Extent3d, FilterMode, FragmentState, FrontFace, MultisampleState, PipelineLayout,
    PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, Queue,
    RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor,
    ShaderStages, StencilState, Texture, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
    VertexState,
};

/// FOW colors matching C++ constants from Display.cpp
pub const FOW_COLOR_SHROUDED: [f32; 4] = [0.0, 0.0, 0.0, 1.0]; // Pure black
pub const FOW_COLOR_FOGGED: [f32; 4] = [0.0, 0.0, 0.0, 0.6]; // 60% black (darkened)
pub const FOW_COLOR_VISIBLE: [f32; 4] = [0.0, 0.0, 0.0, 0.0]; // Transparent (no overlay)

/// Matches C++ PartitionCell shroud grid dimensions
/// Cell size is configurable, typically 50 world units per cell
pub const DEFAULT_FOW_CELL_SIZE: f32 = 50.0;

/// GPU uniform buffer for FOW rendering parameters
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct FowUniforms {
    /// World-to-texture transform matrix (4x4)
    world_to_texture: [[f32; 4]; 4],
    /// Current player ID being rendered
    player_id: u32,
    /// FOW alpha intensity (0.0-1.0)
    fog_intensity: f32,
    /// Gradient smoothing factor (0=hard edges, 1=smooth)
    smoothing: f32,
    /// Observer mode flag (1=bypass FOW, 0=normal)
    observer_mode: u32,
}

impl Default for FowUniforms {
    fn default() -> Self {
        Self {
            world_to_texture: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            player_id: 0,
            fog_intensity: 0.8,
            smoothing: 0.1,
            observer_mode: 0,
        }
    }
}

/// GPU-Accelerated FOW Terrain Overlay Renderer
///
/// Renders fog-of-war as a translucent overlay on the 3D terrain mesh.
/// Uses texture sampling to look up per-cell visibility state and applies
/// appropriate darkening/occlusion effects.
pub struct FowTerrainOverlay {
    /// WGPU device
    device: Arc<Device>,
    /// WGPU queue for uploads
    queue: Arc<Queue>,

    /// FOW texture (R8 format: 0=shrouded, 128=fogged, 255=visible)
    fow_texture: Texture,
    /// Texture view for binding
    fow_texture_view: TextureView,
    /// Texture sampler (bilinear for smooth gradients)
    fow_sampler: Sampler,

    /// Uniform buffer for rendering parameters
    uniform_buffer: Buffer,
    /// Current uniforms (CPU-side)
    uniforms: FowUniforms,

    /// Bind group for textures and uniforms
    bind_group: BindGroup,
    /// Bind group layout
    bind_group_layout: BindGroupLayout,

    /// Render pipeline for FOW overlay
    pipeline: RenderPipeline,

    /// Texture dimensions (matches shroud grid)
    texture_width: u32,
    texture_height: u32,

    /// World bounds (for coordinate mapping)
    world_min_x: f32,
    world_min_z: f32,
    world_max_x: f32,
    world_max_z: f32,
}

impl FowTerrainOverlay {
    /// Create new FOW terrain overlay renderer
    ///
    /// # Arguments
    ///
    /// * `device` - WGPU device
    /// * `queue` - WGPU command queue
    /// * `texture_width` - FOW texture width (typically map_width / cell_size)
    /// * `texture_height` - FOW texture height (typically map_height / cell_size)
    /// * `world_bounds` - (min_x, min_z, max_x, max_z) in world units
    /// * `surface_format` - Target surface format for blending
    /// * `depth_format` - Depth buffer format
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        texture_width: u32,
        texture_height: u32,
        world_bounds: (f32, f32, f32, f32),
        surface_format: TextureFormat,
        depth_format: TextureFormat,
    ) -> Self {
        let (world_min_x, world_min_z, world_max_x, world_max_z) = world_bounds;

        // Create FOW texture (R8 format for efficient single-channel storage)
        let fow_texture = device.create_texture(&TextureDescriptor {
            label: Some("FOW Texture"),
            size: Extent3d {
                width: texture_width,
                height: texture_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let fow_texture_view = fow_texture.create_view(&TextureViewDescriptor::default());

        // Create sampler with bilinear filtering for smooth FOW gradients
        // Matches C++ bilinear sampling behavior
        let fow_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("FOW Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });

        // Create uniform buffer
        let mut uniforms = FowUniforms::default();

        // Calculate world-to-texture transform
        // Maps world XZ coordinates to texture UV coordinates [0, 1]
        let world_width = world_max_x - world_min_x;
        let world_height = world_max_z - world_min_z;
        uniforms.world_to_texture = [
            [1.0 / world_width, 0.0, 0.0, 0.0],
            [0.0, 1.0 / world_height, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [
                -world_min_x / world_width,
                -world_min_z / world_height,
                0.0,
                1.0,
            ],
        ];

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("FOW Uniform Buffer"),
            size: std::mem::size_of::<FowUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Upload initial uniforms
        queue.write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("FOW Bind Group Layout"),
            entries: &[
                // FOW texture
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                // Uniforms
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
            ],
        });

        // Create bind group
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("FOW Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&fow_texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&fow_sampler),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        // Load shaders
        let shader = device.create_shader_module(include_wgsl!("fow_terrain_overlay.wgsl"));

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("FOW Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline with alpha blending
        // Matches C++ alpha blending: SrcAlpha, InvSrcAlpha
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("FOW Terrain Overlay Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: CompareFunction::LessEqual,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Self {
            device,
            queue,
            fow_texture,
            fow_texture_view,
            fow_sampler,
            uniform_buffer,
            uniforms,
            bind_group,
            bind_group_layout,
            pipeline,
            texture_width,
            texture_height,
            world_min_x,
            world_min_z,
            world_max_x,
            world_max_z,
        }
    }

    /// Update FOW texture from shroud grid data
    ///
    /// Uploads per-cell visibility state to GPU texture.
    ///
    /// # Arguments
    ///
    /// * `grid_data` - Raw R8 texture data (0=shrouded, 128=fogged, 255=visible)
    ///
    /// # C++ Reference
    ///
    /// Matches C++ Display::setShroudLevel() calls from PartitionManager.cpp
    pub fn update_texture(&self, grid_data: &[u8]) {
        assert_eq!(
            grid_data.len(),
            (self.texture_width * self.texture_height) as usize,
            "Grid data size mismatch"
        );

        self.queue.write_texture(
            self.fow_texture.as_image_copy(),
            grid_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(self.texture_width),
                rows_per_image: Some(self.texture_height),
            },
            Extent3d {
                width: self.texture_width,
                height: self.texture_height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Set current player ID for rendering
    pub fn set_player_id(&mut self, player_id: u32) {
        self.uniforms.player_id = player_id;
        self.upload_uniforms();
    }

    /// Set fog intensity (0.0 = no fog, 1.0 = full opacity)
    pub fn set_fog_intensity(&mut self, intensity: f32) {
        self.uniforms.fog_intensity = intensity.clamp(0.0, 1.0);
        self.upload_uniforms();
    }

    /// Set gradient smoothing (0.0 = hard edges, 1.0 = smooth transitions)
    pub fn set_smoothing(&mut self, smoothing: f32) {
        self.uniforms.smoothing = smoothing.clamp(0.0, 1.0);
        self.upload_uniforms();
    }

    /// Enable/disable observer mode (bypasses FOW)
    pub fn set_observer_mode(&mut self, enabled: bool) {
        self.uniforms.observer_mode = if enabled { 1 } else { 0 };
        self.upload_uniforms();
    }

    /// Upload uniforms to GPU
    fn upload_uniforms(&self) {
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&self.uniforms));
    }

    /// Render FOW overlay to the current render pass
    ///
    /// Draws a full-screen quad with the FOW overlay effect applied.
    ///
    /// # Arguments
    ///
    /// * `render_pass` - Active render pass to draw into
    pub fn render<'rpass>(&'rpass self, render_pass: &mut wgpu::RenderPass<'rpass>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        // Draw full-screen quad (4 vertices, triangle strip)
        render_pass.draw(0..4, 0..1);
    }

    /// Get texture dimensions
    pub fn get_texture_size(&self) -> (u32, u32) {
        (self.texture_width, self.texture_height)
    }

    /// Get world bounds
    pub fn get_world_bounds(&self) -> (f32, f32, f32, f32) {
        (
            self.world_min_x,
            self.world_min_z,
            self.world_max_x,
            self.world_max_z,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fow_uniforms_size() {
        // Ensure uniform buffer is correctly sized (GPU alignment requirements)
        assert_eq!(
            std::mem::size_of::<FowUniforms>(),
            80 // 64 (matrix) + 4 + 4 + 4 + 4
        );
    }

    #[test]
    fn test_fow_uniforms_default() {
        let uniforms = FowUniforms::default();
        assert_eq!(uniforms.player_id, 0);
        assert_eq!(uniforms.fog_intensity, 0.8);
        assert_eq!(uniforms.smoothing, 0.1);
        assert_eq!(uniforms.observer_mode, 0);
    }

    #[test]
    fn test_fow_colors() {
        // Verify FOW color constants match C++ values
        assert_eq!(FOW_COLOR_SHROUDED[3], 1.0); // Fully opaque
        assert_eq!(FOW_COLOR_FOGGED[3], 0.6); // 60% opacity
        assert_eq!(FOW_COLOR_VISIBLE[3], 0.0); // Transparent
    }
}
