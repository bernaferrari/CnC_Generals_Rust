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
    Extent3d, FilterMode, FragmentState, FrontFace, MultisampleState, Origin3d,
    PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, Queue,
    RenderPipeline as WgpuRenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType,
    SamplerDescriptor, ShaderStages, StencilState, TexelCopyBufferLayout, TexelCopyTextureInfo,
    Texture, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType,
    TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension, VertexState,
};

/// FOW colors matching C++ constants from Display.cpp
pub const FOW_COLOR_SHROUDED: [f32; 4] = [0.0, 0.0, 0.0, 1.0]; // Pure black
pub const FOW_COLOR_FOGGED: [f32; 4] = [0.0, 0.0, 0.0, 0.6]; // 60% black (darkened)
pub const FOW_COLOR_VISIBLE: [f32; 4] = [0.0, 0.0, 0.0, 0.0]; // Transparent (no overlay)

/// Matches C++ PartitionCell shroud grid dimensions
/// Cell size is configurable, typically 50 world units per cell
pub const DEFAULT_FOW_CELL_SIZE: f32 = 50.0;
/// C++ `DEFAULT_SHROUD_CELL_SIZE` uses `MAP_XY_FACTOR`.
pub const DEFAULT_SHROUD_CELL_SIZE: f32 = DEFAULT_FOW_CELL_SIZE;
/// C++ fog interpolation reaches full brightness in one second.
pub const FOG_INTERPOLATION_RATE: f32 = 255.0 / 1000.0;

/// Height-map metrics needed by `W3DShroud::init` and `W3DShroud::render`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShroudMapMetrics {
    pub x_extent: i32,
    pub y_extent: i32,
    pub border_size_inline: i32,
    pub draw_width: i32,
    pub draw_height: i32,
    pub draw_origin_x: i32,
    pub draw_origin_y: i32,
    pub map_xy_factor: f32,
}

impl Default for ShroudMapMetrics {
    fn default() -> Self {
        Self {
            x_extent: 0,
            y_extent: 0,
            border_size_inline: 0,
            draw_width: 0,
            draw_height: 0,
            draw_origin_x: 0,
            draw_origin_y: 0,
            map_xy_factor: DEFAULT_FOW_CELL_SIZE,
        }
    }
}

/// Runtime globals read by the original `W3DShroud` code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct W3DShroudConfig {
    pub shroud_alpha: u8,
    pub shroud_color: u32,
    pub fog_of_war_on: bool,
}

impl Default for W3DShroudConfig {
    fn default() -> Self {
        Self {
            shroud_alpha: 0,
            shroud_color: 0x00ff_ffff,
            fog_of_war_on: false,
        }
    }
}

/// Texture-copy rectangle computed by `W3DShroud::render`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShroudCopyRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub dst_x: i32,
    pub dst_y: i32,
    pub cleared_border: bool,
}

/// CPU mirror of C++ `W3DShroud`.
#[derive(Debug, Clone, PartialEq)]
pub struct W3DShroudState {
    config: W3DShroudConfig,
    num_cells_x: i32,
    num_cells_y: i32,
    num_max_visible_cells_x: i32,
    num_max_visible_cells_y: i32,
    cell_width: f32,
    cell_height: f32,
    src_texture_width: i32,
    src_texture_height: i32,
    src_texture_pitch: i32,
    dst_texture_width: i32,
    dst_texture_height: i32,
    src_pixels: Vec<u16>,
    dst_border_pixels: Vec<u16>,
    final_fog_data: Vec<u8>,
    current_fog_data: Vec<u8>,
    clear_dst_texture: bool,
    border_shroud_level: u8,
    shroud_filter_enabled: bool,
    draw_origin_x: f32,
    draw_origin_y: f32,
    draw_fog_of_war: bool,
}

impl Default for W3DShroudState {
    fn default() -> Self {
        Self::new(W3DShroudConfig::default())
    }
}

impl W3DShroudState {
    pub fn new(config: W3DShroudConfig) -> Self {
        Self {
            config,
            num_cells_x: 0,
            num_cells_y: 0,
            num_max_visible_cells_x: 0,
            num_max_visible_cells_y: 0,
            cell_width: DEFAULT_SHROUD_CELL_SIZE,
            cell_height: DEFAULT_SHROUD_CELL_SIZE,
            src_texture_width: 0,
            src_texture_height: 0,
            src_texture_pitch: 0,
            dst_texture_width: 0,
            dst_texture_height: 0,
            src_pixels: Vec::new(),
            dst_border_pixels: Vec::new(),
            final_fog_data: Vec::new(),
            current_fog_data: Vec::new(),
            clear_dst_texture: true,
            border_shroud_level: config.shroud_alpha,
            shroud_filter_enabled: true,
            draw_origin_x: 0.0,
            draw_origin_y: 0.0,
            draw_fog_of_war: config.fog_of_war_on,
        }
    }

    pub fn init(&mut self, map: ShroudMapMetrics, world_cell_size_x: f32, world_cell_size_y: f32) {
        self.cell_width = world_cell_size_x;
        self.cell_height = world_cell_size_y;

        let usable_x = (map.x_extent - 1 - map.border_size_inline * 2).max(0) as f32;
        let usable_y = (map.y_extent - 1 - map.border_size_inline * 2).max(0) as f32;
        self.num_cells_x = (usable_x * map.map_xy_factor / self.cell_width).ceil() as i32;
        self.num_cells_y = (usable_y * map.map_xy_factor / self.cell_height).ceil() as i32;

        self.num_max_visible_cells_x = (((map.draw_width - 1).max(0) as f32 * map.map_xy_factor
            / self.cell_width)
            .floor() as i32)
            + 1;
        self.num_max_visible_cells_y = (((map.draw_height - 1).max(0) as f32 * map.map_xy_factor
            / self.cell_height)
            .floor() as i32)
            + 1;

        self.src_texture_width = self.num_cells_x;
        self.src_texture_height = self.num_cells_y + 1;
        self.src_texture_pitch = self.src_texture_width * 2;

        self.dst_texture_width = validate_texture_size((self.num_cells_x + 2).max(0));
        self.dst_texture_height = validate_texture_size((self.num_cells_y + 2).max(0));

        let src_len = (self.src_texture_width * self.src_texture_height).max(0) as usize;
        self.src_pixels = vec![0; src_len];
        self.current_fog_data = vec![0; src_len];
        self.final_fog_data = vec![0; self.current_fog_data.len()];
        self.clear_dst_texture = true;
        self.draw_fog_of_war = self.config.fog_of_war_on;

        if self.config.fog_of_war_on {
            self.fill_shroud_data(self.config.shroud_alpha);
        }
    }

    pub fn reset(&mut self) {
        self.src_pixels.clear();
        self.current_fog_data.clear();
        self.final_fog_data.clear();
        self.src_texture_width = 0;
        self.src_texture_height = 0;
        self.src_texture_pitch = 0;
        self.clear_dst_texture = true;
    }

    pub fn release_resources(&mut self) {
        self.dst_border_pixels.clear();
    }

    pub fn reacquire_resources(&mut self) -> bool {
        if self.dst_texture_width == 0 {
            return true;
        }
        self.clear_dst_texture = true;
        true
    }

    pub fn num_cells_x(&self) -> i32 {
        self.num_cells_x
    }

    pub fn num_cells_y(&self) -> i32 {
        self.num_cells_y
    }

    pub fn texture_width(&self) -> i32 {
        self.dst_texture_width
    }

    pub fn texture_height(&self) -> i32 {
        self.dst_texture_height
    }

    pub fn cell_width(&self) -> f32 {
        self.cell_width
    }

    pub fn cell_height(&self) -> f32 {
        self.cell_height
    }

    pub fn draw_origin_x(&self) -> f32 {
        self.draw_origin_x
    }

    pub fn draw_origin_y(&self) -> f32 {
        self.draw_origin_y
    }

    pub fn shroud_filter_enabled(&self) -> bool {
        self.shroud_filter_enabled
    }

    pub fn clear_dst_texture(&self) -> bool {
        self.clear_dst_texture
    }

    pub fn pixel_at(&self, x: i32, y: i32) -> Option<u16> {
        self.texture_index(x, y)
            .and_then(|index| self.src_pixels.get(index).copied())
    }

    pub fn get_shroud_level(&self, x: i32, y: i32) -> u8 {
        let Some(index) = self.cell_index(x, y) else {
            return 0;
        };
        let Some(pixel) = self.src_pixels.get(index).copied() else {
            return 0;
        };
        if self.config.fog_of_war_on {
            let alpha = (pixel >> 12) & 0x0f;
            ((1.0 - alpha as f32 / 15.0) * 255.0) as u8
        } else {
            (((pixel >> 5) & 0x3f) as f32 / 63.0 * 255.0) as u8
        }
    }

    pub fn set_shroud_level(&mut self, x: i32, y: i32, level: u8, texture_only: bool) -> bool {
        let Some(index) = self.cell_index(x, y) else {
            return false;
        };
        if index >= self.src_pixels.len() {
            return false;
        }
        let level = level.max(self.config.shroud_alpha);
        if !texture_only {
            if let Some(final_level) = self.final_fog_data.get_mut(index) {
                *final_level = level;
            }
        }
        self.src_pixels[index] = self.encode_level(level);
        true
    }

    pub fn fill_shroud_data(&mut self, level: u8) {
        let level = level.max(self.config.shroud_alpha);
        let pixel = self.encode_level(level);
        for y in 0..self.num_cells_y {
            for x in 0..self.num_cells_x {
                if let Some(index) = self.cell_index(x, y) {
                    self.src_pixels[index] = pixel;
                    if let Some(final_level) = self.final_fog_data.get_mut(index) {
                        *final_level = level;
                    }
                }
            }
        }
    }

    pub fn set_border_shroud_level(&mut self, level: u8) {
        self.border_shroud_level = level;
        self.clear_dst_texture = true;
    }

    pub fn fill_border_shroud_data(&mut self, level: u8) {
        let level = level.max(self.config.shroud_alpha);
        let pixel = self.encode_level(level);
        let bottom_row = self.num_cells_y;
        for x in 0..self.num_cells_x {
            if let Some(index) = self.texture_index(x, bottom_row) {
                self.src_pixels[index] = pixel;
            }
        }
        self.dst_border_pixels =
            vec![pixel; (self.dst_texture_width * self.dst_texture_height).max(0) as usize];
    }

    pub fn render(&mut self, map: ShroudMapMetrics) -> Option<ShroudCopyRect> {
        if self.src_pixels.is_empty() {
            return None;
        }
        if self.config.fog_of_war_on != self.draw_fog_of_war {
            self.reset();
            self.init(map, self.cell_width, self.cell_height);
        }

        // Zero Hour intentionally updates the whole shroud texture.
        let mut vis_start_x = 0;
        let mut vis_start_y = 0;

        let mut vis_end_x = self.num_cells_x;
        let mut vis_end_y = self.num_cells_y;

        if vis_end_x > self.num_cells_x {
            vis_start_x = (vis_start_x - (vis_end_x - self.num_cells_x)).max(0);
            vis_end_x = self.num_cells_x;
        }
        if vis_end_y > self.num_cells_y {
            vis_start_y = (vis_start_y - (vis_end_y - self.num_cells_y)).max(0);
            vis_end_y = self.num_cells_y;
        }

        self.draw_origin_x = vis_start_x as f32 * self.cell_width;
        self.draw_origin_y = vis_start_y as f32 * self.cell_height;

        let cleared_border = self.clear_dst_texture;
        if self.clear_dst_texture {
            self.clear_dst_texture = false;
            self.fill_border_shroud_data(self.border_shroud_level);
        }

        Some(ShroudCopyRect {
            left: vis_start_x,
            top: vis_start_y,
            right: vis_end_x,
            bottom: vis_end_y,
            dst_x: 1,
            dst_y: 1,
            cleared_border,
        })
    }

    pub fn interpolate_fog_levels(&mut self, elapsed_ms: u32) {
        if elapsed_ms == 0 {
            return;
        }
        let level_delta = (FOG_INTERPOLATION_RATE * elapsed_ms as f32)
            .min(255.0)
            .max(0.0) as u8;
        if level_delta == 0 {
            return;
        }
        for y in 0..self.num_cells_y {
            for x in 0..self.num_cells_x {
                let data_index = (x + y * self.num_cells_x) as usize;
                let current = self.current_fog_data[data_index];
                let final_level = self.final_fog_data[data_index];
                if current == final_level {
                    continue;
                }
                let next = if final_level < current {
                    current.saturating_sub(level_delta).max(final_level)
                } else {
                    current.saturating_add(level_delta).min(final_level)
                };
                self.current_fog_data[data_index] = next;
                self.set_shroud_level(x, y, next, true);
            }
        }
    }

    pub fn set_shroud_filter(&mut self, enable: bool) {
        self.shroud_filter_enabled = enable;
    }

    fn cell_index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.num_cells_x && y < self.num_cells_y)
            .then_some((x + y * self.src_texture_width) as usize)
    }

    fn texture_index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.src_texture_width && y < self.src_texture_height)
            .then_some((x + y * self.src_texture_width) as usize)
    }

    fn encode_level(&self, level: u8) -> u16 {
        if self.config.fog_of_war_on {
            let red = ((self.config.shroud_color >> 16) & 0xff) as u16;
            let green = ((self.config.shroud_color >> 8) & 0xff) as u16;
            let blue = (self.config.shroud_color & 0xff) as u16;
            let alpha = 255u16.saturating_sub(level as u16);
            ((blue >> 4) & 0x0f)
                | (((green >> 4) & 0x0f) << 4)
                | (((red >> 4) & 0x0f) << 8)
                | (((alpha >> 4) & 0x0f) << 12)
        } else {
            let mut blue = level as u32 * (self.config.shroud_color & 0xff) / 255;
            let mut green = level as u32 * ((self.config.shroud_color >> 8) & 0xff) / 255;
            let mut red = level as u32 * ((self.config.shroud_color >> 16) & 0xff) / 255;
            if level == 255 {
                red = 255;
                green = 255;
                blue = 255;
            }
            (((blue & 0xf8) >> 3) | ((green & 0xfc) << 3) | ((red & 0xf8) << 8)) as u16
        }
    }
}

fn validate_texture_size(size: i32) -> i32 {
    if size <= 1 {
        return size.max(1);
    }
    (size as u32).next_power_of_two() as i32
}

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
    pipeline: WgpuRenderPipeline,

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
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
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
            cache: None,
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
            TexelCopyTextureInfo {
                texture: &self.fow_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            grid_data,
            TexelCopyBufferLayout {
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
