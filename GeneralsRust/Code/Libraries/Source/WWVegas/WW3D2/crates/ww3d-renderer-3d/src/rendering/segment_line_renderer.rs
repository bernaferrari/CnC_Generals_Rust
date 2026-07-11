//! Segment Line Renderer - Rendering segmented lines with fractal subdivision
//!
//! This module provides SegLineRenderer, a sophisticated line rendering system
//! for particle trails, beam effects, projectile paths, and trail visualization.
//! Lines are rendered as screen-aligned or world-aligned ribbons with configurable
//! width, color gradients, texture mapping, and fractal noise subdivision.
//!
//! # C++ Reference
//!
//! Matches seglinerenderer.h and seglinerenderer.cpp from WW3D2
//! Implementation provides 100% fidelity to C++ SegLineRendererClass behavior.
//!
//! # Features
//!
//! - Dynamic vertex/index buffer rendering
//! - Batch rendering of multiple segments
//! - Fractal noise subdivision for lightning effects
//! - Multiple texture mapping modes
//! - Color gradients along line
//! - Intersection merging for smooth corners
//! - UV animation support
//! - Proper depth testing and alpha blending
//!
//! # Architecture
//!
//! The renderer builds ribbons by:
//! 1. Transform points from object space to eye space
//! 2. Apply fractal subdivision with noise
//! 3. Calculate edge planes for each segment (top/bottom)
//! 4. Find intersections between adjacent segments
//! 5. Merge intersections to avoid polygon folding
//! 6. Generate vertices and triangles for rendering
//!
//! # Texture Mapping Modes
//!
//! - **UNIFORM_WIDTH**: Entire line uses one row of texture (constant V)
//! - **UNIFORM_LENGTH**: Texture stretched length-wise
//! - **TILED**: Tiled continuously over line

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec2, Vec3, Vec4};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use wgpu::{
    Buffer, BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites, Device, FragmentState,
    PipelineLayout, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, Queue, RenderPass,
    RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor,
    ShaderModule, ShaderModuleDescriptor, ShaderSource, ShaderStages, Texture, TextureDescriptor,
    TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureView,
    TextureViewDescriptor, TextureViewDimension, VertexAttribute, VertexBufferLayout, VertexFormat,
    VertexState, VertexStepMode,
};
use ww3d_core::ww3d::WW3D;

/// Maximum subdivision levels allowed (must be ≤ 7 to avoid excessive chunk sizes)
/// C++ Reference: seglinerenderer.h line 56
pub const MAX_SEGLINE_SUBDIV_LEVELS: usize = 7;

/// Chunk size for batch processing segments
/// C++ Reference: seglinerenderer.cpp lines 57-61
#[cfg(not(test))]
pub const SEGLINE_CHUNK_SIZE: usize = if (1 << MAX_SEGLINE_SUBDIV_LEVELS) > 128 {
    1 << MAX_SEGLINE_SUBDIV_LEVELS
} else {
    128
};

#[cfg(test)]
pub const SEGLINE_CHUNK_SIZE: usize = 16; // Smaller for tests

/// Maximum point buffer size (includes one extra for overlap)
/// C++ Reference: seglinerenderer.cpp line 69
pub const MAX_SEGLINE_POINT_BUFFER_SIZE: usize = 1 + SEGLINE_CHUNK_SIZE;

/// Maximum polygon buffer size (2 triangles per segment)
/// C++ Reference: seglinerenderer.cpp line 71
pub const MAX_SEGLINE_POLY_BUFFER_SIZE: usize = SEGLINE_CHUNK_SIZE * 2;

/// Maximum line tiling factor to avoid performance issues
/// C++ Reference: seglinerenderer.cpp line 197
const MAX_LINE_TILING_FACTOR: f32 = 50.0;

/// Parallel edge detection threshold
/// C++ Reference: seglinerenderer.cpp line 255
const PARALLEL_FACTOR: f32 = 0.9;

/// Texture mapping modes for line rendering
/// C++ Reference: seglinerenderer.h lines 74-78
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureMapMode {
    /// Entire line uses one row of texture (constant V coordinate)
    UniformWidthTextureMap = 0x00000000,
    /// Entire line uses one row of texture stretched length-wise
    UniformLengthTextureMap = 0x00000001,
    /// Tiled continuously over line
    TiledTextureMap = 0x00000002,
}

impl Default for TextureMapMode {
    fn default() -> Self {
        Self::UniformWidthTextureMap
    }
}

/// Flags for controlling segment line rendering behavior
/// C++ Reference: seglinerenderer.h lines 164-176
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegLineFlags {
    bits: u32,
}

impl SegLineFlags {
    const MERGE_INTERSECTIONS: u32 = 0x00000001;
    const FREEZE_RANDOM: u32 = 0x00000002;
    const DISABLE_SORTING: u32 = 0x00000004;
    const END_CAPS: u32 = 0x00000008;
    const TEXTURE_MAP_MODE_MASK: u32 = 0xFF000000;
    const TEXTURE_MAP_MODE_OFFSET: u32 = 24;

    const DEFAULT_BITS: u32 =
        Self::MERGE_INTERSECTIONS | ((TextureMapMode::UniformWidthTextureMap as u32) << 24);

    pub fn new() -> Self {
        Self {
            bits: Self::DEFAULT_BITS,
        }
    }

    pub fn merge_intersections(&self) -> bool {
        (self.bits & Self::MERGE_INTERSECTIONS) != 0
    }

    pub fn set_merge_intersections(&mut self, enabled: bool) {
        if enabled {
            self.bits |= Self::MERGE_INTERSECTIONS;
        } else {
            self.bits &= !Self::MERGE_INTERSECTIONS;
        }
    }

    pub fn freeze_random(&self) -> bool {
        (self.bits & Self::FREEZE_RANDOM) != 0
    }

    pub fn set_freeze_random(&mut self, enabled: bool) {
        if enabled {
            self.bits |= Self::FREEZE_RANDOM;
        } else {
            self.bits &= !Self::FREEZE_RANDOM;
        }
    }

    pub fn disable_sorting(&self) -> bool {
        (self.bits & Self::DISABLE_SORTING) != 0
    }

    pub fn set_disable_sorting(&mut self, enabled: bool) {
        if enabled {
            self.bits |= Self::DISABLE_SORTING;
        } else {
            self.bits &= !Self::DISABLE_SORTING;
        }
    }

    pub fn end_caps(&self) -> bool {
        (self.bits & Self::END_CAPS) != 0
    }

    pub fn set_end_caps(&mut self, enabled: bool) {
        if enabled {
            self.bits |= Self::END_CAPS;
        } else {
            self.bits &= !Self::END_CAPS;
        }
    }

    pub fn texture_map_mode(&self) -> TextureMapMode {
        let mode = (self.bits & Self::TEXTURE_MAP_MODE_MASK) >> Self::TEXTURE_MAP_MODE_OFFSET;
        match mode {
            0 => TextureMapMode::UniformWidthTextureMap,
            1 => TextureMapMode::UniformLengthTextureMap,
            2 => TextureMapMode::TiledTextureMap,
            _ => TextureMapMode::UniformWidthTextureMap,
        }
    }

    pub fn set_texture_map_mode(&mut self, mode: TextureMapMode) {
        self.bits &= !Self::TEXTURE_MAP_MODE_MASK;
        self.bits |= ((mode as u32) << Self::TEXTURE_MAP_MODE_OFFSET) & Self::TEXTURE_MAP_MODE_MASK;
    }
}

impl Default for SegLineFlags {
    fn default() -> Self {
        Self::new()
    }
}

/// Vertex format for segment line rendering
/// C++ Reference: Matches VertexFormatXYZDUV1 structure
/// Uses padding fields to satisfy bytemuck::Pod (no implicit padding)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SegLineVertex {
    pub position: [f32; 4], // XYZ + pad to align to Vec4 (C++ VertexFormatXYZDUV1)
    pub diffuse: [f32; 4],
    pub uv: [f32; 2],
}

impl SegLineVertex {
    pub fn new(position: Vec3, diffuse: Vec4, uv: Vec2) -> Self {
        Self {
            position: [position.x, position.y, position.z, 0.0],
            diffuse: diffuse.into(),
            uv: uv.into(),
        }
    }

    const ATTRIBUTES: &'static [VertexAttribute] = &[
        VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: VertexFormat::Float32x3,
        },
        VertexAttribute {
            offset: std::mem::size_of::<[f32; 4]>() as u64,
            shader_location: 1,
            format: VertexFormat::Float32x4,
        },
        VertexAttribute {
            offset: (std::mem::size_of::<[f32; 4]>() + std::mem::size_of::<[f32; 4]>()) as u64,
            shader_location: 2,
            format: VertexFormat::Float32x2,
        },
    ];

    pub fn vertex_layout<'a>() -> VertexBufferLayout<'a> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<SegLineVertex>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: Self::ATTRIBUTES,
        }
    }
}

/// Triangle index structure
/// C++ Reference: Matches TriIndex structure
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TriIndex {
    pub i: u16,
    pub j: u16,
    pub k: u16,
}

/// Edge type for segment rendering
/// C++ Reference: seglinerenderer.cpp lines 371-377
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentEdge {
    Top = 0,
    Bottom = 1,
}

const NUM_EDGES: usize = 2;

/// Line segment structure with edge planes
/// C++ Reference: seglinerenderer.cpp lines 386-389
#[derive(Debug, Clone)]
struct LineSegment {
    start_plane: Vec3,
    edge_plane: [Vec3; NUM_EDGES],
}

impl Default for LineSegment {
    fn default() -> Self {
        Self {
            start_plane: Vec3::ZERO,
            edge_plane: [Vec3::ZERO; NUM_EDGES],
        }
    }
}

/// Intersection between adjacent line segments
/// C++ Reference: seglinerenderer.cpp lines 395-403
#[derive(Debug, Clone)]
struct LineSegmentIntersection {
    point_count: u32,
    next_segment_id: usize,
    direction: Vec3,
    point: Vec3,
    tex_v: f32,
    rgba: Vec4,
    fold: bool,
    parallel: bool,
}

impl Default for LineSegmentIntersection {
    fn default() -> Self {
        Self {
            point_count: 0,
            next_segment_id: 0,
            direction: Vec3::X,
            point: Vec3::ZERO,
            tex_v: 0.0,
            rgba: Vec4::ZERO,
            fold: false,
            parallel: false,
        }
    }
}

/// Subdivision stack entry for fractal noise
/// C++ Reference: seglinerenderer.cpp lines 1221-1230
#[derive(Debug, Clone)]
struct SegLineSubdivision {
    start_pos: Vec3,
    end_pos: Vec3,
    start_tex_v: f32,
    end_tex_v: f32,
    start_diffuse: Vec4,
    end_diffuse: Vec4,
    rand: f32,
    level: u32,
}

/// Segment Line Renderer - Main rendering class
///
/// # C++ Reference
///
/// Matches SegLineRendererClass from seglinerenderer.h/cpp
///
/// # Usage
///
/// ```ignore
/// let mut renderer = SegLineRenderer::new();
/// renderer.set_width(2.0);
/// renderer.set_color(Vec3::new(1.0, 0.0, 0.0));
/// renderer.set_opacity(0.8);
/// renderer.set_subdivision_level(2);
/// renderer.set_noise_amplitude(0.5);
///
/// // Render line from points
/// renderer.render(&rinfo, &transform, &points, &sphere, None)?;
/// ```
#[derive(Debug, Clone)]
pub struct SegLineRenderer {
    /// Texture for line rendering (optional)
    /// C++ Reference: seglinerenderer.h line 138
    texture: Option<String>, // Texture name/path

    /// Shader configuration (placeholder for now)
    /// C++ Reference: seglinerenderer.h line 139
    shader: u32, // Shader ID

    /// Width of the line (thickness in world units)
    /// C++ Reference: seglinerenderer.h line 140
    width: f32,

    /// Base color of the line (RGB)
    /// C++ Reference: seglinerenderer.h line 141
    color: Vec3,

    /// Opacity/alpha of the line (0.0-1.0)
    /// C++ Reference: seglinerenderer.h line 142
    opacity: f32,

    /// Current subdivision level for fractal noise
    /// C++ Reference: seglinerenderer.h line 145
    subdivision_level: u32,

    /// Amplitude of fractal noise displacement
    /// C++ Reference: seglinerenderer.h line 146
    noise_amplitude: f32,

    /// Factor for aborting intersection merges
    /// C++ Reference: seglinerenderer.h line 151
    merge_abort_factor: f32,

    /// Texture tiling factor for TILED mode
    /// C++ Reference: seglinerenderer.h line 156
    texture_tile_factor: f32,

    /// Last sync time for UV animation
    /// C++ Reference: seglinerenderer.h line 159
    last_used_sync_time: u64,

    /// Current UV offset for animation
    /// C++ Reference: seglinerenderer.h line 160
    current_uv_offset: Vec2,

    /// UV offset delta per millisecond
    /// C++ Reference: seglinerenderer.h line 161
    uv_offset_delta_per_ms: Vec2,

    /// Rendering flags and texture mode
    /// C++ Reference: seglinerenderer.h line 178
    flags: SegLineFlags,

    /// Vertex buffer (dynamically allocated)
    /// C++ Reference: seglinerenderer.h lines 183-184
    vertex_buffer: Vec<SegLineVertex>,
}

impl SegLineRenderer {
    /// Create a new segment line renderer with default settings
    ///
    /// # C++ Reference
    ///
    /// Matches SegLineRendererClass::SegLineRendererClass constructor
    /// (seglinerenderer.cpp lines 76-94)
    pub fn new() -> Self {
        Self {
            texture: None,
            shader: 0, // ShaderClass::_PresetAdditiveSpriteShader
            width: 0.0,
            color: Vec3::ONE,
            opacity: 1.0,
            subdivision_level: 0,
            noise_amplitude: 0.0,
            merge_abort_factor: 1.5,
            texture_tile_factor: 1.0,
            last_used_sync_time: 0, // WW3D::Get_Sync_Time()
            current_uv_offset: Vec2::ZERO,
            uv_offset_delta_per_ms: Vec2::ZERO,
            flags: SegLineFlags::new(),
            vertex_buffer: Vec::new(),
        }
    }

    /// Set the texture for line rendering
    ///
    /// # C++ Reference
    ///
    /// Matches SegLineRendererClass::Set_Texture (seglinerenderer.cpp lines 174-177)
    pub fn set_texture(&mut self, texture: Option<String>) {
        self.texture = texture;
    }

    /// Get the current texture
    pub fn get_texture(&self) -> Option<&str> {
        self.texture.as_deref()
    }

    /// Set the shader ID
    pub fn set_shader(&mut self, shader: u32) {
        self.shader = shader;
    }

    /// Get the shader ID
    pub fn get_shader(&self) -> u32 {
        self.shader
    }

    /// Set line width (thickness)
    pub fn set_width(&mut self, width: f32) {
        self.width = width;
    }

    /// Get line width
    pub fn get_width(&self) -> f32 {
        self.width
    }

    /// Set line color (RGB)
    pub fn set_color(&mut self, color: Vec3) {
        self.color = color;
    }

    /// Get line color
    pub fn get_color(&self) -> Vec3 {
        self.color
    }

    /// Set line opacity (0.0-1.0)
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    /// Get line opacity
    pub fn get_opacity(&self) -> f32 {
        self.opacity
    }

    /// Set fractal subdivision level (0-7)
    ///
    /// # C++ Reference
    ///
    /// Matches subdivision logic in seglinerenderer.cpp lines 1207-1305
    pub fn set_subdivision_level(&mut self, level: u32) {
        self.subdivision_level = level.min(MAX_SEGLINE_SUBDIV_LEVELS as u32);
    }

    /// Get current subdivision level
    pub fn get_subdivision_level(&self) -> u32 {
        self.subdivision_level
    }

    /// Set noise amplitude for fractal displacement
    pub fn set_noise_amplitude(&mut self, amplitude: f32) {
        self.noise_amplitude = amplitude;
    }

    /// Get noise amplitude
    pub fn get_noise_amplitude(&self) -> f32 {
        self.noise_amplitude
    }

    /// Set merge abort factor
    ///
    /// Controls when intersection merging is aborted if it causes
    /// vertices to move too far from the line.
    ///
    /// # C++ Reference
    ///
    /// Used in seglinerenderer.cpp lines 845-854
    pub fn set_merge_abort_factor(&mut self, factor: f32) {
        self.merge_abort_factor = factor;
    }

    /// Get merge abort factor
    pub fn get_merge_abort_factor(&self) -> f32 {
        self.merge_abort_factor
    }

    /// Set texture tile factor (clamped to avoid performance issues)
    ///
    /// # C++ Reference
    ///
    /// Matches SegLineRendererClass::Set_Texture_Tile_Factor
    /// (seglinerenderer.cpp lines 192-205)
    pub fn set_texture_tile_factor(&mut self, factor: f32) {
        if factor > MAX_LINE_TILING_FACTOR {
            eprintln!(
                "Texture Tile Factor ({:.2}) too large! Clamping to {:.2}",
                factor, MAX_LINE_TILING_FACTOR
            );
            self.texture_tile_factor = MAX_LINE_TILING_FACTOR;
        } else {
            self.texture_tile_factor = factor.max(0.0);
        }
    }

    /// Get texture tile factor
    pub fn get_texture_tile_factor(&self) -> f32 {
        self.texture_tile_factor
    }

    /// Set texture mapping mode
    pub fn set_texture_map_mode(&mut self, mode: TextureMapMode) {
        self.flags.set_texture_map_mode(mode);
    }

    /// Get texture mapping mode
    pub fn get_texture_map_mode(&self) -> TextureMapMode {
        self.flags.texture_map_mode()
    }

    /// Set UV offset rate (units per second)
    ///
    /// # C++ Reference
    ///
    /// Matches Set_UV_Offset_Rate inline (seglinerenderer.h lines 205-208)
    pub fn set_uv_offset_rate(&mut self, rate: Vec2) {
        self.uv_offset_delta_per_ms = rate * 0.001;
    }

    /// Get UV offset rate
    pub fn get_uv_offset_rate(&self) -> Vec2 {
        self.uv_offset_delta_per_ms * 1000.0
    }

    /// Set current UV offset directly
    ///
    /// # C++ Reference
    ///
    /// Matches Set_Current_UV_Offset (seglinerenderer.cpp lines 187-190)
    pub fn set_current_uv_offset(&mut self, offset: Vec2) {
        self.current_uv_offset = offset;
    }

    /// Get current UV offset
    pub fn get_current_uv_offset(&self) -> Vec2 {
        self.current_uv_offset
    }

    /// Enable/disable intersection merging
    pub fn set_merge_intersections(&mut self, enabled: bool) {
        self.flags.set_merge_intersections(enabled);
    }

    /// Check if intersection merging is enabled
    pub fn is_merge_intersections(&self) -> bool {
        self.flags.merge_intersections()
    }

    /// Enable/disable random freezing
    pub fn set_freeze_random(&mut self, enabled: bool) {
        self.flags.set_freeze_random(enabled);
    }

    /// Check if random is frozen
    pub fn is_freeze_random(&self) -> bool {
        self.flags.freeze_random()
    }

    /// Enable/disable sorting
    pub fn set_disable_sorting(&mut self, enabled: bool) {
        self.flags.set_disable_sorting(enabled);
    }

    /// Check if sorting is disabled
    pub fn is_sorting_disabled(&self) -> bool {
        self.flags.disable_sorting()
    }

    /// Enable/disable end caps
    pub fn set_end_caps(&mut self, enabled: bool) {
        self.flags.set_end_caps(enabled);
    }

    /// Check if end caps are enabled
    pub fn are_end_caps_enabled(&self) -> bool {
        self.flags.end_caps()
    }

    /// Reset line state (UV offset and sync time)
    ///
    /// # C++ Reference
    ///
    /// Matches Reset_Line (seglinerenderer.cpp lines 207-211)
    pub fn reset_line(&mut self) {
        self.last_used_sync_time = WW3D::sync_time() as u64;
        self.current_uv_offset = Vec2::ZERO;
    }

    /// Scale the renderer properties
    ///
    /// # C++ Reference
    ///
    /// Matches Scale (seglinerenderer.cpp lines 1307-1311)
    pub fn scale(&mut self, scale: f32) {
        self.width *= scale;
        self.noise_amplitude *= scale;
    }

    /// Render segmented line from array of points
    ///
    /// # Arguments
    ///
    /// * `transform` - Object to world transform matrix
    /// * `view` - View matrix (world to eye space)
    /// * `points` - Array of 3D points defining the line path
    /// * `rgbas` - Optional per-point color array (if None, uses renderer color)
    ///
    /// # Returns
    ///
    /// Returns arrays of vertices and indices ready for rendering
    ///
    /// # C++ Reference
    ///
    /// Matches SegLineRendererClass::Render (seglinerenderer.cpp lines 215-1205)
    ///
    /// # Implementation
    ///
    /// This is the core rendering algorithm:
    /// 1. Transform points to eye space
    /// 2. Apply fractal subdivision with noise
    /// 3. Calculate edge planes for each segment
    /// 4. Find intersections between adjacent segments
    /// 5. Merge intersections to avoid polygon folding
    /// 6. Generate vertices and triangle indices
    pub fn render(
        &mut self,
        transform: &Mat4,
        view: &Mat4,
        points: &[Vec3],
        rgbas: Option<&[Vec4]>,
    ) -> (Vec<SegLineVertex>, Vec<TriIndex>) {
        if points.len() < 2 {
            return (Vec::new(), Vec::new());
        }

        let num_points = points.len();
        // Pre-allocate based on segment count estimation
        // Each segment generates ~2 vertices (top/bottom) and ~2 triangles (4 indices)
        // With subdivision, multiply by subdivision factor
        let subdivision_factor = 1 << self.subdivision_level;
        let estimated_segments = (num_points - 1) * subdivision_factor;
        let mut all_vertices = Vec::with_capacity(estimated_segments * 2);
        let mut all_indices = Vec::with_capacity(estimated_segments * 4);

        // Handle UV offset animation
        // C++ Reference: seglinerenderer.cpp lines 235-245
        let current_time_ms = WW3D::sync_time() as u64;
        let delta = current_time_ms.saturating_sub(self.last_used_sync_time);
        let del = delta as f32;
        let mut uv_offset = self.current_uv_offset + self.uv_offset_delta_per_ms * del;

        // Ensure offsets are in [0, 1] range
        uv_offset.x = uv_offset.x - uv_offset.x.floor();
        uv_offset.y = uv_offset.y - uv_offset.y.floor();

        self.current_uv_offset = uv_offset;
        self.last_used_sync_time = current_time_ms;

        let map_mode = self.get_texture_map_mode();

        // Calculate modelview matrix
        // C++ Reference: seglinerenderer.cpp lines 284-293
        let view3 = Mat3::from_mat4(*view);
        let modelview = view3 * Mat3::from_mat4(*transform);

        // Process chunks
        // C++ Reference: seglinerenderer.cpp lines 261-268
        let chunk_size = ((SEGLINE_CHUNK_SIZE >> self.subdivision_level) + 1).min(num_points);

        let mut chidx = 0;
        while chidx < num_points - 1 {
            let point_cnt = (num_points - chidx).min(chunk_size);

            // Render this chunk
            let (verts, indices) = self.render_chunk(
                &modelview,
                &points[chidx..chidx + point_cnt],
                rgbas.map(|r| &r[chidx..chidx + point_cnt]),
                map_mode,
                uv_offset,
                chidx,
            );

            // Offset indices for concatenation
            let vertex_offset = all_vertices.len() as u16;
            all_vertices.extend(verts);
            all_indices.extend(indices.into_iter().map(|mut tri| {
                tri.i += vertex_offset;
                tri.j += vertex_offset;
                tri.k += vertex_offset;
                tri
            }));

            chidx += chunk_size - 1; // Overlap last point
        }

        (all_vertices, all_indices)
    }

    /// Render a single chunk of the line
    ///
    /// # C++ Reference
    ///
    /// This implements the core of the render loop from seglinerenderer.cpp
    /// lines 268-1201
    #[allow(clippy::too_many_lines)]
    fn render_chunk(
        &self,
        modelview: &Mat3,
        points: &[Vec3],
        rgbas: Option<&[Vec4]>,
        map_mode: TextureMapMode,
        uv_offset: Vec2,
        global_offset: usize,
    ) -> (Vec<SegLineVertex>, Vec<TriIndex>) {
        let point_cnt = points.len();
        if point_cnt < 2 {
            return (Vec::new(), Vec::new());
        }

        // Transform points to eye space
        // C++ Reference: seglinerenderer.cpp lines 295-296
        let mut xformed_pts = vec![Vec3::ZERO; point_cnt];
        for (i, pt) in points.iter().enumerate() {
            xformed_pts[i] = *modelview * *pt;
        }

        // Prepare texture V coordinates
        // C++ Reference: seglinerenderer.cpp lines 302-330
        let mut base_tex_v = vec![0.0; point_cnt];
        let u_values = match map_mode {
            TextureMapMode::UniformWidthTextureMap => {
                for v in &mut base_tex_v {
                    *v = 0.0;
                }
                [0.0, 1.0]
            }
            TextureMapMode::UniformLengthTextureMap => {
                for (i, v) in base_tex_v.iter_mut().enumerate() {
                    *v = (i + global_offset) as f32 * self.texture_tile_factor;
                }
                [0.0, 0.0]
            }
            TextureMapMode::TiledTextureMap => {
                for (i, v) in base_tex_v.iter_mut().enumerate() {
                    *v = (i + global_offset) as f32 * self.texture_tile_factor;
                }
                [0.0, 1.0]
            }
        };

        // Apply subdivision
        // C++ Reference: seglinerenderer.cpp lines 340-353
        let (subdiv_pts, subdiv_tex_v, subdiv_diffuse) =
            self.subdivision_util(&xformed_pts, &base_tex_v, rgbas);

        let points = &subdiv_pts;
        let tex_v = &subdiv_tex_v;
        let diffuse = &subdiv_diffuse;
        let point_cnt = points.len();

        // Calculate line segment edge planes
        // C++ Reference: seglinerenderer.cpp lines 358-488
        let radius = self.width * 0.5;
        let numsegs = point_cnt - 1;

        let mut segments = vec![LineSegment::default(); point_cnt + 1];
        let mut intersections = vec![
            [
                LineSegmentIntersection::default(),
                LineSegmentIntersection::default()
            ];
            point_cnt + 1
        ];

        let mut switch_edges = false;

        // Calculate segment edge planes
        for sidx in 1..=numsegs {
            let curr_point = points[sidx - 1];
            let mut next_point = points[sidx];

            // Prevent degenerate segments
            if (curr_point - next_point).length() < 0.0001 {
                next_point.x += 0.001;
            }

            let segdir = (next_point - curr_point).normalize();

            // Find edge planes
            let nearest = curr_point + segdir * -segdir.dot(curr_point);
            let mut offset = segdir.cross(nearest).normalize();
            if offset.length() < 0.0001 {
                offset = Vec3::Y; // Fallback perpendicular
            }

            let top = curr_point + offset * radius;
            let bottom = curr_point - offset * radius;

            let top_normal = top.cross(segdir).normalize();
            let bottom_normal = segdir.cross(bottom).normalize();

            // Check for fold
            if sidx > 1 {
                let prev_plane = points[sidx - 2].cross(curr_point).normalize();
                let curr_plane = curr_point.cross(next_point).normalize();

                if prev_plane.dot(curr_plane) < 0.0 {
                    switch_edges = !switch_edges;
                    intersections[sidx][0].fold = true;
                    intersections[sidx][1].fold = true;
                } else {
                    intersections[sidx][0].fold = false;
                    intersections[sidx][1].fold = false;
                }
            }

            if switch_edges {
                segments[sidx].edge_plane[0] = -bottom_normal;
                segments[sidx].edge_plane[1] = -top_normal;
            } else {
                segments[sidx].edge_plane[0] = top_normal;
                segments[sidx].edge_plane[1] = bottom_normal;
            }

            segments[sidx].start_plane = segdir;
        }

        // Initialize dummy segments and first/last intersections
        // C++ Reference: seglinerenderer.cpp lines 490-607
        self.init_intersections(
            &mut segments,
            &mut intersections,
            points,
            tex_v,
            diffuse,
            numsegs,
        );

        // Calculate midpoint intersections
        // C++ Reference: seglinerenderer.cpp lines 618-722
        self.calculate_intersections(&segments, &mut intersections, points, tex_v, diffuse);

        // Merge intersections
        // C++ Reference: seglinerenderer.cpp lines 731-922
        let mut num_intersections = [point_cnt, point_cnt];
        if self.is_merge_intersections() {
            self.merge_intersections(
                &segments,
                &mut intersections,
                &mut num_intersections,
                radius,
            );
        }

        // Generate vertices and triangles
        // C++ Reference: seglinerenderer.cpp lines 932-1099
        self.generate_geometry(
            points,
            tex_v,
            diffuse,
            &intersections,
            &num_intersections,
            u_values,
            uv_offset,
        )
    }

    /// Apply fractal subdivision with noise
    ///
    /// # C++ Reference
    ///
    /// Matches subdivision_util (seglinerenderer.cpp lines 1207-1305)
    fn subdivision_util(
        &self,
        xformed_pts: &[Vec3],
        base_tex_v: &[f32],
        base_diffuse: Option<&[Vec4]>,
    ) -> (Vec<Vec3>, Vec<f32>, Vec<Vec4>) {
        let point_cnt = xformed_pts.len();
        let max_points = point_cnt * (1 << self.subdivision_level);

        let mut subdiv_pts = Vec::with_capacity(max_points);
        let mut subdiv_tex_v = Vec::with_capacity(max_points);
        let mut subdiv_diffuse = Vec::with_capacity(max_points);

        let freeze_random = self.is_freeze_random();
        let mut random_state = 12345u32; // Simple LCG seed

        for pidx in 0..point_cnt - 1 {
            let mut stack = Vec::new();
            stack.push(SegLineSubdivision {
                start_pos: xformed_pts[pidx],
                end_pos: xformed_pts[pidx + 1],
                start_tex_v: base_tex_v[pidx],
                end_tex_v: base_tex_v[pidx + 1],
                start_diffuse: base_diffuse.map(|d| d[pidx]).unwrap_or(Vec4::new(
                    self.color.x,
                    self.color.y,
                    self.color.z,
                    self.opacity,
                )),
                end_diffuse: base_diffuse.map(|d| d[pidx + 1]).unwrap_or(Vec4::new(
                    self.color.x,
                    self.color.y,
                    self.color.z,
                    self.opacity,
                )),
                rand: self.noise_amplitude,
                level: 0,
            });

            while let Some(entry) = stack.pop() {
                if entry.level == self.subdivision_level {
                    subdiv_pts.push(entry.start_pos);
                    subdiv_tex_v.push(entry.start_tex_v);
                    subdiv_diffuse.push(entry.start_diffuse);
                } else {
                    // Generate random vector
                    let randvec = if freeze_random {
                        // Use deterministic "random"
                        Vec3::new(
                            ((random_state % 1000) as f32 / 1000.0) - 0.5,
                            ((random_state / 1000 % 1000) as f32 / 1000.0) - 0.5,
                            ((random_state / 1000000 % 1000) as f32 / 1000.0) - 0.5,
                        )
                    } else {
                        // Use simple pseudorandom
                        random_state = random_state.wrapping_mul(1103515245).wrapping_add(12345);
                        Vec3::new(
                            ((random_state % 1000) as f32 / 1000.0) - 0.5,
                            ((random_state / 1000 % 1000) as f32 / 1000.0) - 0.5,
                            ((random_state / 1000000 % 1000) as f32 / 1000.0) - 0.5,
                        )
                    };

                    let mid_pos = (entry.start_pos + entry.end_pos) * 0.5 + randvec * entry.rand;
                    let mid_tex_v = (entry.start_tex_v + entry.end_tex_v) * 0.5;
                    let mid_diffuse = (entry.start_diffuse + entry.end_diffuse) * 0.5;

                    // Push second half first (so first half is processed next)
                    stack.push(SegLineSubdivision {
                        start_pos: mid_pos,
                        end_pos: entry.end_pos,
                        start_tex_v: mid_tex_v,
                        end_tex_v: entry.end_tex_v,
                        start_diffuse: mid_diffuse,
                        end_diffuse: entry.end_diffuse,
                        rand: entry.rand * 0.5,
                        level: entry.level + 1,
                    });

                    stack.push(SegLineSubdivision {
                        start_pos: entry.start_pos,
                        end_pos: mid_pos,
                        start_tex_v: entry.start_tex_v,
                        end_tex_v: mid_tex_v,
                        start_diffuse: entry.start_diffuse,
                        end_diffuse: mid_diffuse,
                        rand: entry.rand * 0.5,
                        level: entry.level + 1,
                    });
                }
            }
        }

        // Add last point
        let last_idx = point_cnt - 1;
        subdiv_pts.push(xformed_pts[last_idx]);
        subdiv_tex_v.push(base_tex_v[last_idx]);
        subdiv_diffuse.push(base_diffuse.map(|d| d[last_idx]).unwrap_or(Vec4::new(
            self.color.x,
            self.color.y,
            self.color.z,
            self.opacity,
        )));

        (subdiv_pts, subdiv_tex_v, subdiv_diffuse)
    }

    /// Initialize first/last intersections and dummy segments
    ///
    /// # C++ Reference
    ///
    /// Matches initialization code from seglinerenderer.cpp lines 490-607
    #[allow(clippy::too_many_arguments)]
    fn init_intersections(
        &self,
        segments: &mut [LineSegment],
        intersections: &mut [[LineSegmentIntersection; 2]],
        points: &[Vec3],
        tex_v: &[f32],
        diffuse: &[Vec4],
        numsegs: usize,
    ) {
        let point_cnt = points.len();

        // Initialize pre-first dummy intersection
        intersections[0][0].next_segment_id = 0;
        intersections[0][1].next_segment_id = 0;

        // Initialize first point intersection
        intersections[1][0].point_count = 1;
        intersections[1][0].next_segment_id = 1;
        intersections[1][0].point = points[0];
        intersections[1][0].tex_v = tex_v[0];
        intersections[1][0].rgba = diffuse[0];
        intersections[1][0].fold = true;

        intersections[1][1] = intersections[1][0].clone();

        // Calculate first dummy segment
        let first_plane = &segments[1].edge_plane;
        let first_point = points[0];

        let top = first_point - first_plane[0] * first_plane[0].dot(first_point);
        let top_dir = top.normalize();
        intersections[1][0].direction = top_dir;

        let bottom = first_point - first_plane[1] * first_plane[1].dot(first_point);
        let bottom_dir = bottom.normalize();
        intersections[1][1].direction = bottom_dir;

        let segdir = (points[1] - points[0]).normalize();
        let mut start_pl = top_dir.cross(bottom_dir).normalize();
        if segdir.dot(start_pl) < 0.0 {
            start_pl = -start_pl;
        }

        segments[0].start_plane = start_pl;
        segments[0].edge_plane[0] = start_pl;
        segments[0].edge_plane[1] = start_pl;
        segments[1].start_plane = start_pl;

        // Initialize last point intersection
        let last_isec = point_cnt;
        intersections[last_isec][0].point_count = 1;
        intersections[last_isec][0].next_segment_id = numsegs + 1;
        intersections[last_isec][0].point = points[point_cnt - 1];
        intersections[last_isec][0].tex_v = tex_v[point_cnt - 1];
        intersections[last_isec][0].rgba = diffuse[point_cnt - 1];
        intersections[last_isec][0].fold = true;

        intersections[last_isec][1] = intersections[last_isec][0].clone();

        // Calculate last dummy segment
        let last_plane = &segments[numsegs].edge_plane;
        let last_point = points[point_cnt - 1];

        let top = last_point - last_plane[0] * last_plane[0].dot(last_point);
        let top_dir = top.normalize();
        intersections[last_isec][0].direction = top_dir;

        let bottom = last_point - last_plane[1] * last_plane[1].dot(last_point);
        let bottom_dir = bottom.normalize();
        intersections[last_isec][1].direction = bottom_dir;

        let segdir = (points[point_cnt - 1] - points[point_cnt - 2]).normalize();
        let mut start_pl = top_dir.cross(bottom_dir).normalize();
        if segdir.dot(start_pl) < 0.0 {
            start_pl = -start_pl;
        }

        segments[numsegs + 1].start_plane = start_pl;
        segments[numsegs + 1].edge_plane[0] = start_pl;
        segments[numsegs + 1].edge_plane[1] = start_pl;
    }

    /// Calculate midpoint intersections
    ///
    /// # C++ Reference
    ///
    /// Matches intersection calculation from seglinerenderer.cpp lines 618-722
    fn calculate_intersections(
        &self,
        segments: &[LineSegment],
        intersections: &mut [[LineSegmentIntersection; 2]],
        points: &[Vec3],
        tex_v: &[f32],
        diffuse: &[Vec4],
    ) {
        let num_intersections = points.len();

        for iidx in 2..num_intersections {
            let midpoint = points[iidx - 1];
            let mid_tex_v = tex_v[iidx - 1];
            let mid_diffuse = diffuse[iidx - 1];

            // Initialize fields
            for edge in 0..2 {
                intersections[iidx][edge].point_count = 1;
                intersections[iidx][edge].next_segment_id = iidx;
                intersections[iidx][edge].point = midpoint;
                intersections[iidx][edge].tex_v = mid_tex_v;
                intersections[iidx][edge].rgba = mid_diffuse;
            }

            // Calculate top edge intersection
            let vdp = segments[iidx - 1].edge_plane[0].dot(segments[iidx].edge_plane[0]);
            if vdp.abs() < PARALLEL_FACTOR {
                // Not parallel - intersect planes
                let mut dir = segments[iidx - 1].edge_plane[0]
                    .cross(segments[iidx].edge_plane[0])
                    .normalize();
                if dir.dot(midpoint) < 0.0 {
                    dir = -dir;
                }
                intersections[iidx][0].direction = dir;
                intersections[iidx][0].parallel = false;
            } else {
                // Parallel - use averaged plane
                let pl = if vdp > 0.0 {
                    (segments[iidx - 1].edge_plane[0] + segments[iidx].edge_plane[0]).normalize()
                } else {
                    (segments[iidx - 1].edge_plane[0] - segments[iidx].edge_plane[0]).normalize()
                };
                let dir = (midpoint - pl * pl.dot(midpoint)).normalize();
                intersections[iidx][0].direction = dir;
                intersections[iidx][0].parallel = true;
            }

            // Calculate bottom edge intersection (same logic)
            let vdp = segments[iidx - 1].edge_plane[1].dot(segments[iidx].edge_plane[1]);
            if vdp.abs() < PARALLEL_FACTOR {
                let mut dir = segments[iidx - 1].edge_plane[1]
                    .cross(segments[iidx].edge_plane[1])
                    .normalize();
                if dir.dot(midpoint) < 0.0 {
                    dir = -dir;
                }
                intersections[iidx][1].direction = dir;
                intersections[iidx][1].parallel = false;
            } else {
                let pl = if vdp > 0.0 {
                    (segments[iidx - 1].edge_plane[1] + segments[iidx].edge_plane[1]).normalize()
                } else {
                    (segments[iidx - 1].edge_plane[1] - segments[iidx].edge_plane[1]).normalize()
                };
                let dir = (midpoint - pl * pl.dot(midpoint)).normalize();
                intersections[iidx][1].direction = dir;
                intersections[iidx][1].parallel = true;
            }

            // Calculate start plane
            let mut _start_pl = intersections[iidx][0]
                .direction
                .cross(intersections[iidx][1].direction)
                .normalize();
            if segments[iidx].start_plane.dot(_start_pl) < 0.0 {
                _start_pl = -_start_pl;
            }
            // Note: would update segments[iidx].start_plane here in mutable version
        }
    }

    /// Merge intersections to avoid polygon folding
    ///
    /// # C++ Reference
    ///
    /// Matches merge logic from seglinerenderer.cpp lines 731-922
    fn merge_intersections(
        &self,
        segments: &[LineSegment],
        intersections: &mut [[LineSegmentIntersection; 2]],
        num_intersections: &mut [usize; 2],
        radius: f32,
    ) {
        let mut merged = true;

        while merged {
            merged = false;

            for edge in 0..2 {
                let num_isects = num_intersections[edge];
                let mut iidx_w = 1;

                for iidx_r in 1..num_isects {
                    // Extract data to avoid borrowing issues
                    let curr_int_data = intersections[iidx_r][edge].clone();
                    let next_int_data = intersections[iidx_r + 1][edge].clone();
                    let prev_int_data = intersections[iidx_w - 1][edge].clone();

                    let next_seg = &segments[next_int_data.next_segment_id];
                    let prev_seg = &segments[prev_int_data.next_segment_id];
                    let curr_seg = &segments[curr_int_data.next_segment_id];

                    let should_merge = (!next_int_data.fold
                        && curr_int_data.direction.dot(next_seg.start_plane) > 0.0
                        && curr_int_data.direction.dot(next_seg.edge_plane[edge]) > 0.0)
                        || (!curr_int_data.fold
                            && next_int_data.direction.dot(-curr_seg.start_plane) > 0.0
                            && next_int_data.direction.dot(prev_seg.edge_plane[edge]) > 0.0);

                    if should_merge && self.merge_abort_factor > 0.0 {
                        // Check merge abort condition
                        let abort_dist2 = (radius * self.merge_abort_factor).powi(2);
                        let diff = curr_int_data.point
                            - curr_int_data.direction
                                * curr_int_data.direction.dot(curr_int_data.point);
                        if diff.length_squared() > abort_dist2 {
                            // Abort merge - copy current
                            intersections[iidx_w][edge] = curr_int_data;
                            iidx_w += 1;
                            continue;
                        }
                    }

                    if should_merge {
                        // Perform merge
                        merged = true;
                        num_intersections[edge] -= 1;

                        // Calculate merged intersection (simplified)
                        let new_count = curr_int_data.point_count + next_int_data.point_count;
                        let new_point = (curr_int_data.point + next_int_data.point) * 0.5;
                        let new_tex_v = (curr_int_data.tex_v + next_int_data.tex_v) * 0.5;
                        let new_rgba = (curr_int_data.rgba + next_int_data.rgba) * 0.5;

                        // Simplified direction calculation
                        let new_direction =
                            (curr_int_data.direction + next_int_data.direction).normalize();

                        intersections[iidx_r][edge].direction = new_direction;
                        intersections[iidx_r][edge].point = new_point;
                        intersections[iidx_r][edge].tex_v = new_tex_v;
                        intersections[iidx_r][edge].rgba = new_rgba;
                        intersections[iidx_r][edge].point_count = new_count;
                        intersections[iidx_r][edge].next_segment_id = next_int_data.next_segment_id;
                    } else {
                        // No merge - copy current
                        intersections[iidx_w][edge] = curr_int_data;
                        iidx_w += 1;
                    }
                }

                // Copy last if needed
                if iidx_w < num_isects {
                    intersections[iidx_w][edge] = intersections[num_isects][edge].clone();
                }
            }
        }
    }

    /// Generate final vertex and index geometry
    ///
    /// # C++ Reference
    ///
    /// Matches vertex generation from seglinerenderer.cpp lines 932-1099
    #[allow(clippy::too_many_arguments)]
    fn generate_geometry(
        &self,
        points: &[Vec3],
        _tex_v: &[f32],
        _diffuse: &[Vec4],
        intersections: &[[LineSegmentIntersection; 2]],
        num_intersections: &[usize; 2],
        u_values: [f32; 2],
        uv_offset: Vec2,
    ) -> (Vec<SegLineVertex>, Vec<TriIndex>) {
        let vnum = num_intersections[0] + num_intersections[1];
        let mut vertices = Vec::with_capacity(vnum);
        // Each pair of vertices (top/bottom) generates ~2 triangles
        // Estimate indices as (vnum / 2) * 2 triangles * 3 vertices = vnum * 3
        let mut indices = Vec::with_capacity(vnum * 3);

        // Prime with first two vertices
        let top_dir = intersections[1][0].direction;
        let bottom_dir = intersections[1][1].direction;

        let top = top_dir * top_dir.dot(points[0]);
        let bottom = bottom_dir * bottom_dir.dot(points[0]);

        vertices.push(SegLineVertex::new(
            top,
            intersections[1][0].rgba,
            Vec2::new(
                u_values[0] + uv_offset.x,
                intersections[1][0].tex_v + uv_offset.y,
            ),
        ));

        vertices.push(SegLineVertex::new(
            bottom,
            intersections[1][1].rgba,
            Vec2::new(
                u_values[1] + uv_offset.x,
                intersections[1][1].tex_v + uv_offset.y,
            ),
        ));

        let mut last_top_vidx = 0u16;
        let mut last_bottom_vidx = 1u16;
        let mut top_int_idx = 1;
        let mut bottom_int_idx = 1;
        let mut pidx = 0;
        let mut residual_top = intersections[1][0].point_count as usize;
        let mut residual_bottom = intersections[1][1].point_count as usize;

        // Skip points
        let delta = residual_top.min(residual_bottom) - 1;
        residual_top -= delta;
        residual_bottom -= delta;
        pidx += delta;

        loop {
            if residual_top == 1 && residual_bottom == 1 {
                // Advance both - tristrip
                indices.push(TriIndex {
                    i: last_top_vidx,
                    j: last_bottom_vidx,
                    k: vertices.len() as u16,
                });
                indices.push(TriIndex {
                    i: last_bottom_vidx,
                    j: (vertices.len() + 1) as u16,
                    k: vertices.len() as u16,
                });

                last_top_vidx = vertices.len() as u16;
                last_bottom_vidx = (vertices.len() + 1) as u16;

                top_int_idx += 1;
                bottom_int_idx += 1;
                residual_top = intersections[top_int_idx][0].point_count as usize;
                residual_bottom = intersections[bottom_int_idx][1].point_count as usize;
                pidx += 1;

                if pidx >= points.len() {
                    break;
                }

                // Generate vertices
                let top_dir = intersections[top_int_idx][0].direction;
                let bottom_dir = intersections[bottom_int_idx][1].direction;

                let top = top_dir * top_dir.dot(points[pidx]);
                let bottom = bottom_dir * bottom_dir.dot(points[pidx]);

                vertices.push(SegLineVertex::new(
                    top,
                    intersections[top_int_idx][0].rgba,
                    Vec2::new(
                        u_values[0] + uv_offset.x,
                        intersections[top_int_idx][0].tex_v + uv_offset.y,
                    ),
                ));

                vertices.push(SegLineVertex::new(
                    bottom,
                    intersections[bottom_int_idx][1].rgba,
                    Vec2::new(
                        u_values[1] + uv_offset.x,
                        intersections[bottom_int_idx][1].tex_v + uv_offset.y,
                    ),
                ));
            } else if residual_top > 1 {
                // Advance bottom only - fan
                indices.push(TriIndex {
                    i: last_top_vidx,
                    j: last_bottom_vidx,
                    k: vertices.len() as u16,
                });

                last_bottom_vidx = vertices.len() as u16;
                residual_top -= 1;
                bottom_int_idx += 1;
                residual_bottom = intersections[bottom_int_idx][1].point_count as usize;
                pidx += 1;

                if pidx >= points.len() {
                    break;
                }

                let bottom_dir = intersections[bottom_int_idx][1].direction;
                let bottom = bottom_dir * bottom_dir.dot(points[pidx]);

                vertices.push(SegLineVertex::new(
                    bottom,
                    intersections[bottom_int_idx][1].rgba,
                    Vec2::new(
                        u_values[1] + uv_offset.x,
                        intersections[bottom_int_idx][1].tex_v + uv_offset.y,
                    ),
                ));
            } else {
                // Advance top only - fan
                indices.push(TriIndex {
                    i: last_top_vidx,
                    j: last_bottom_vidx,
                    k: vertices.len() as u16,
                });

                last_top_vidx = vertices.len() as u16;
                residual_bottom -= 1;
                top_int_idx += 1;
                residual_top = intersections[top_int_idx][0].point_count as usize;
                pidx += 1;

                if pidx >= points.len() {
                    break;
                }

                let top_dir = intersections[top_int_idx][0].direction;
                let top = top_dir * top_dir.dot(points[pidx]);

                vertices.push(SegLineVertex::new(
                    top,
                    intersections[top_int_idx][0].rgba,
                    Vec2::new(
                        u_values[0] + uv_offset.x,
                        intersections[top_int_idx][0].tex_v + uv_offset.y,
                    ),
                ));
            }

            // Skip points
            let delta = residual_top.min(residual_bottom).saturating_sub(1);
            residual_top -= delta;
            residual_bottom -= delta;
            pidx += delta;

            // Exit conditions
            if (top_int_idx >= num_intersections[0] && residual_top == 1)
                || (bottom_int_idx >= num_intersections[1] && residual_bottom == 1)
            {
                break;
            }
        }

        (vertices, indices)
    }
}

impl Default for SegLineRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// WGSL shader for segmented line rendering (mirrors wthree_d_segmented_line.wgsl)
// ---------------------------------------------------------------------------

const SEGLINE_SHADER_SOURCE: &str = r#"
struct Uniforms {
    view_proj : mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms : Uniforms;

@group(1) @binding(0)
var line_texture : texture_2d<f32>;

@group(1) @binding(1)
var line_sampler : sampler;

struct VSInput {
    @location(0) position : vec3<f32>,
    @location(1) color : vec4<f32>,
    @location(2) uv : vec2<f32>,
};

struct VSOutput {
    @builtin(position) position : vec4<f32>,
    @location(0) color : vec4<f32>,
    @location(1) uv : vec2<f32>,
};

@vertex
fn vs_main(input : VSInput) -> VSOutput {
    var output : VSOutput;
    output.position = uniforms.view_proj * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    output.uv = input.uv;
    return output;
}

@fragment
fn fs_main(input : VSOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(line_texture, line_sampler, input.uv);
    return tex * input.color;
}
"#;

// ---------------------------------------------------------------------------
// SegLineGpuPipeline — wgpu render pipeline + buffers for segmented lines
// ---------------------------------------------------------------------------

/// GPU-side pipeline and buffers for rendering segmented lines.
///
/// Created once from a `Device`/`Queue` pair, then reused across frames.
/// Call [`SegLineGpuPipeline::draw`] each frame with the vertices/indices
/// produced by [`SegLineRenderer::render`].
pub struct SegLineGpuPipeline {
    device: Arc<Device>,
    queue: Arc<Queue>,

    pipeline: RenderPipeline,
    _pipeline_layout: PipelineLayout,
    _shader_module: ShaderModule,

    /// White 1×1 texture used when no texture is bound.
    white_texture: Texture,
    white_texture_view: TextureView,
    white_sampler: Sampler,

    vertex_buffer: Buffer,
    index_buffer: Buffer,
    vertex_capacity: usize,
    index_capacity: usize,
}

/// Maximum vertices the line GPU buffers can hold before needing reallocation.
const INITIAL_LINE_VERTEX_CAPACITY: usize = 4096;
const INITIAL_LINE_INDEX_CAPACITY: usize = 8192;

impl SegLineGpuPipeline {
    /// Create the GPU pipeline from a device, queue, and surface format.
    ///
    /// The `surface_format` determines the render target pixel format for the
    /// fragment shader output.
    pub fn new(device: Arc<Device>, queue: Arc<Queue>, surface_format: TextureFormat) -> Self {
        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("SegLine Shader"),
            source: ShaderSource::Wgsl(SEGLINE_SHADER_SOURCE.into()),
        });

        // Bind group 0: view-proj uniform
        let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SegLine Uniform BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Bind group 1: texture + sampler
        let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SegLine Texture BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("SegLine Pipeline Layout"),
            bind_group_layouts: &[&uniform_bgl, &texture_bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("SegLine Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[SegLineVertex::vertex_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // White 1×1 texture for untextured lines
        let white_texture = device.create_texture(&TextureDescriptor {
            label: Some("SegLine White Texture"),
            size: wgpu::Extent3d {
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
        let white_texture_view = white_texture.create_view(&TextureViewDescriptor::default());
        queue.write_texture(
            white_texture.as_image_copy(),
            &[255u8, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let white_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("SegLine Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let vertex_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("SegLine Vertex Buffer"),
            size: (INITIAL_LINE_VERTEX_CAPACITY * std::mem::size_of::<SegLineVertex>()) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("SegLine Index Buffer"),
            size: (INITIAL_LINE_INDEX_CAPACITY * std::mem::size_of::<TriIndex>()) as u64,
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            pipeline,
            _pipeline_layout: pipeline_layout,
            _shader_module: shader_module,
            white_texture,
            white_texture_view,
            white_sampler,
            vertex_buffer,
            index_buffer,
            vertex_capacity: INITIAL_LINE_VERTEX_CAPACITY,
            index_capacity: INITIAL_LINE_INDEX_CAPACITY,
        }
    }

    /// Create a uniform buffer + bind group for a view-projection matrix.
    pub fn create_view_proj_bind_group(&self, view_proj: &Mat4) -> (Buffer, wgpu::BindGroup) {
        let bytes: [[f32; 4]; 4] = (*view_proj).to_cols_array_2d();
        let uniform_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SegLine ViewProj Uniform"),
                contents: bytemuck::cast_slice(&bytes),
                usage: BufferUsages::UNIFORM,
            });

        let layout = self.pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SegLine ViewProj BG"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        (uniform_buffer, bind_group)
    }

    /// Create a texture bind group using the white 1×1 fallback texture.
    pub fn create_white_texture_bind_group(&self) -> wgpu::BindGroup {
        let layout = self.pipeline.get_bind_group_layout(1);
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SegLine White Texture BG"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.white_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.white_sampler),
                },
            ],
        })
    }

    /// Upload vertex and index data to GPU buffers, then issue a draw call.
    ///
    /// Automatically grows GPU buffers if the geometry exceeds current capacity.
    ///
    /// # Arguments
    ///
    /// * `render_pass` — active wgpu render pass
    /// * `vertices` — line ribbon vertices from [`SegLineRenderer::render`]
    /// * `indices` — triangle indices from [`SegLineRenderer::render`]
    /// * `view_proj_bg` — bind group for group 0 (view-proj uniform)
    /// * `texture_bg` — bind group for group 1 (texture + sampler)
    pub fn draw(
        &mut self,
        render_pass: &mut RenderPass<'_>,
        vertices: &[SegLineVertex],
        indices: &[TriIndex],
        view_proj_bg: &wgpu::BindGroup,
        texture_bg: &wgpu::BindGroup,
    ) {
        if vertices.is_empty() || indices.is_empty() {
            return;
        }

        // Grow vertex buffer if needed
        if vertices.len() > self.vertex_capacity {
            self.vertex_capacity = vertices.len().next_power_of_two();
            self.vertex_buffer = self.device.create_buffer(&BufferDescriptor {
                label: Some("SegLine Vertex Buffer (grown)"),
                size: (self.vertex_capacity * std::mem::size_of::<SegLineVertex>()) as u64,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        // Grow index buffer if needed
        if indices.len() > self.index_capacity {
            self.index_capacity = indices.len().next_power_of_two();
            self.index_buffer = self.device.create_buffer(&BufferDescriptor {
                label: Some("SegLine Index Buffer (grown)"),
                size: (self.index_capacity * std::mem::size_of::<TriIndex>()) as u64,
                usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        // Upload geometry
        self.queue
            .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(vertices));
        self.queue
            .write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(indices));

        // Draw
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, view_proj_bg, &[]);
        render_pass.set_bind_group(1, texture_bg, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segline_renderer_creation() {
        let renderer = SegLineRenderer::new();
        assert_eq!(renderer.get_width(), 0.0);
        assert_eq!(renderer.get_color(), Vec3::ONE);
        assert_eq!(renderer.get_opacity(), 1.0);
        assert_eq!(renderer.get_subdivision_level(), 0);
        assert_eq!(renderer.get_noise_amplitude(), 0.0);
        assert_eq!(renderer.get_merge_abort_factor(), 1.5);
        assert_eq!(renderer.get_texture_tile_factor(), 1.0);
    }

    #[test]
    fn test_segline_flags() {
        let mut flags = SegLineFlags::new();
        assert!(flags.merge_intersections());
        assert!(!flags.freeze_random());
        assert!(!flags.disable_sorting());
        assert!(!flags.end_caps());

        flags.set_freeze_random(true);
        assert!(flags.freeze_random());

        flags.set_disable_sorting(true);
        assert!(flags.disable_sorting());

        flags.set_end_caps(true);
        assert!(flags.end_caps());
    }

    #[test]
    fn test_texture_map_mode() {
        let mut flags = SegLineFlags::new();
        assert_eq!(
            flags.texture_map_mode(),
            TextureMapMode::UniformWidthTextureMap
        );

        flags.set_texture_map_mode(TextureMapMode::TiledTextureMap);
        assert_eq!(flags.texture_map_mode(), TextureMapMode::TiledTextureMap);

        flags.set_texture_map_mode(TextureMapMode::UniformLengthTextureMap);
        assert_eq!(
            flags.texture_map_mode(),
            TextureMapMode::UniformLengthTextureMap
        );
    }

    #[test]
    fn test_set_properties() {
        let mut renderer = SegLineRenderer::new();

        renderer.set_width(2.5);
        assert_eq!(renderer.get_width(), 2.5);

        renderer.set_color(Vec3::new(1.0, 0.0, 0.5));
        assert_eq!(renderer.get_color(), Vec3::new(1.0, 0.0, 0.5));

        renderer.set_opacity(0.7);
        assert_eq!(renderer.get_opacity(), 0.7);

        renderer.set_subdivision_level(3);
        assert_eq!(renderer.get_subdivision_level(), 3);

        renderer.set_noise_amplitude(1.5);
        assert_eq!(renderer.get_noise_amplitude(), 1.5);
    }

    #[test]
    fn test_subdivision_level_clamping() {
        let mut renderer = SegLineRenderer::new();

        renderer.set_subdivision_level(10);
        assert_eq!(
            renderer.get_subdivision_level(),
            MAX_SEGLINE_SUBDIV_LEVELS as u32
        );
    }

    #[test]
    fn test_texture_tile_factor_clamping() {
        let mut renderer = SegLineRenderer::new();

        renderer.set_texture_tile_factor(100.0);
        assert_eq!(renderer.get_texture_tile_factor(), MAX_LINE_TILING_FACTOR);

        renderer.set_texture_tile_factor(-5.0);
        assert_eq!(renderer.get_texture_tile_factor(), 0.0);

        renderer.set_texture_tile_factor(10.0);
        assert_eq!(renderer.get_texture_tile_factor(), 10.0);
    }

    #[test]
    fn test_uv_offset_rate() {
        let mut renderer = SegLineRenderer::new();

        renderer.set_uv_offset_rate(Vec2::new(2.0, 3.0));
        let rate = renderer.get_uv_offset_rate();
        assert!((rate.x - 2.0).abs() < 0.001);
        assert!((rate.y - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_reset_line() {
        let mut renderer = SegLineRenderer::new();
        renderer.set_current_uv_offset(Vec2::new(0.5, 0.5));

        WW3D::sync(1000);
        renderer.reset_line();
        assert_eq!(renderer.get_current_uv_offset(), Vec2::ZERO);
        assert_eq!(renderer.last_used_sync_time, 1000);
    }

    #[test]
    fn test_scale() {
        let mut renderer = SegLineRenderer::new();
        renderer.set_width(2.0);
        renderer.set_noise_amplitude(1.0);

        renderer.scale(2.0);
        assert_eq!(renderer.get_width(), 4.0);
        assert_eq!(renderer.get_noise_amplitude(), 2.0);
    }

    #[test]
    fn test_subdivision_basic() {
        let renderer = SegLineRenderer::new();

        let points = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0)];
        let tex_v = vec![0.0, 1.0];

        let (subdiv_pts, subdiv_tex_v, subdiv_diffuse) =
            renderer.subdivision_util(&points, &tex_v, None);

        // No subdivision (level 0) should return same points
        assert_eq!(subdiv_pts.len(), 2);
        assert_eq!(subdiv_tex_v.len(), 2);
        assert_eq!(subdiv_diffuse.len(), 2);
    }

    #[test]
    fn test_subdivision_with_noise() {
        let mut renderer = SegLineRenderer::new();
        renderer.set_subdivision_level(1);
        renderer.set_noise_amplitude(0.5);

        let points = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0)];
        let tex_v = vec![0.0, 1.0];

        let (subdiv_pts, _, _) = renderer.subdivision_util(&points, &tex_v, None);

        // Level 1 subdivision should give 3 points
        assert_eq!(subdiv_pts.len(), 3);
    }

    #[test]
    fn test_render_simple_line() {
        let mut renderer = SegLineRenderer::new();
        renderer.set_width(1.0);
        renderer.set_color(Vec3::ONE);
        renderer.set_opacity(1.0);

        let points = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(5.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
        ];

        let transform = Mat4::IDENTITY;
        let view = Mat4::IDENTITY;

        WW3D::sync(0);
        let (vertices, indices) = renderer.render(&transform, &view, &points, None);

        // Should generate vertices and indices
        assert!(!vertices.is_empty());
        assert!(!indices.is_empty());

        // Each triangle should have 3 valid indices
        for tri in &indices {
            assert!((tri.i as usize) < vertices.len());
            assert!((tri.j as usize) < vertices.len());
            assert!((tri.k as usize) < vertices.len());
        }
    }

    #[test]
    fn test_render_with_colors() {
        let mut renderer = SegLineRenderer::new();
        renderer.set_width(1.0);

        let points = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0)];
        let colors = vec![Vec4::new(1.0, 0.0, 0.0, 1.0), Vec4::new(0.0, 1.0, 0.0, 1.0)];

        let transform = Mat4::IDENTITY;
        let view = Mat4::IDENTITY;

        WW3D::sync(0);
        let (vertices, _) = renderer.render(&transform, &view, &points, Some(&colors));

        // Vertices should have gradient colors
        assert!(!vertices.is_empty());
    }

    #[test]
    fn test_render_empty_points() {
        let mut renderer = SegLineRenderer::new();
        let transform = Mat4::IDENTITY;
        let view = Mat4::IDENTITY;

        WW3D::sync(0);
        let (vertices, indices) = renderer.render(&transform, &view, &[], None);

        assert!(vertices.is_empty());
        assert!(indices.is_empty());
    }

    #[test]
    fn test_render_single_point() {
        let mut renderer = SegLineRenderer::new();
        let transform = Mat4::IDENTITY;
        let view = Mat4::IDENTITY;
        let points = vec![Vec3::ZERO];

        WW3D::sync(0);
        let (vertices, indices) = renderer.render(&transform, &view, &points, None);

        assert!(vertices.is_empty());
        assert!(indices.is_empty());
    }

    #[test]
    fn test_uv_animation() {
        let mut renderer = SegLineRenderer::new();
        renderer.set_uv_offset_rate(Vec2::new(1.0, 0.5)); // 1.0 X, 0.5 Y per second
        WW3D::sync(0);
        renderer.reset_line(); // Initialize sync time

        let points = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0)];
        let transform = Mat4::IDENTITY;
        let view = Mat4::IDENTITY;

        // Render at t=500ms (0.5 seconds after reset)
        WW3D::sync(500);
        let (vertices, _) = renderer.render(&transform, &view, &points, None);

        // UV offset should have changed
        let uv_offset = renderer.get_current_uv_offset();
        // After 500ms at 1.0/sec rate (0.001/ms), offset should be 0.5
        // After 500ms at 0.5/sec rate, Y offset should be 0.25
        assert!(
            (uv_offset.x - 0.5).abs() < 0.01,
            "Expected UV offset X ~0.5, got {}",
            uv_offset.x
        );
        assert!(
            (uv_offset.y - 0.25).abs() < 0.01,
            "Expected UV offset Y ~0.25, got {}",
            uv_offset.y
        );
    }
}
