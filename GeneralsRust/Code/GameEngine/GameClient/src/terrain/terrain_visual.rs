//! # Terrain Visual System
//!
//! Core terrain rendering system that matches the C++ TerrainVisual implementation exactly.
//! Handles heightmaps, texturing, water, roads, and all visual terrain features.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::f32::consts::PI;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use glam::{Mat4, Vec2, Vec3, Vec4Swizzles};
use log::{debug, warn};
use wgpu::util::DeviceExt;
use wgpu::{
    BindGroup, BindGroupLayout, Buffer, RenderPass, Sampler, SamplerDescriptor, Texture,
    TextureView,
};

use crate::display::image::GameImageError;
use crate::system::SubsystemInterface;
// use crate::display::{RenderDevice, WgpuRenderer}; // These don't exist yet
use super::chunk::{ChunkId, ChunkManager, ViewFrustum};
use super::roads::{
    RoadCondition, RoadMinimapSample, RoadSyntheticIntersectionKind, RoadType, StoneType,
};
use super::textures::{
    TerrainTexture, TerrainTextures, TextureId, TextureKind, TextureRule, MAX_BLEND_WEIGHTS,
};
use super::{
    calculate_terrain_lod, HeightMap, RoadSystem, TerrainConfig, TerrainError, TerrainLOD,
    TerrainModification, TerrainResult, TerrainStats, TerrainVertex, TerrainVisual, WaterSystem,
};
use bytemuck::cast_slice;
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::global_data;
use game_engine::common::ini::get_global_data;
use game_engine::common::ini::ini_terrain;
use game_engine::common::ini::ini_terrain::{TerrainSurface, TerrainType};
use game_engine::common::ini::ini_webpage_url::get_registry_language;
use game_engine::common::system::file::FileAccess;
use game_engine::common::system::file_system::get_file_system;
use game_engine::common::system::file_system::paths::{
    MAP_PREVIEW_DIR_PATH, TERRAIN_TGA_DIR_PATH, USER_TGA_DIR_PATH,
};
use image::GenericImageView;
use image::ImageFormat;

/// Water handle for terrain water systems
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WaterHandle(pub u32);

/// Runtime road segment descriptor passed from game-logic map parsing.
#[derive(Debug, Clone)]
pub struct RuntimeRoadVisualSegment {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub width: f32,
    pub template_name: String,
    pub width_in_texture: f32,
    pub road_type_id: u32,
    pub start_is_angled: bool,
    pub start_is_join: bool,
    pub end_is_angled: bool,
    pub end_is_join: bool,
    pub curve_radius: f32,
}

#[derive(Debug, Clone, Copy)]
struct RuntimeRoadEndpointTopology {
    start_count: u32,
    end_count: u32,
    start_last: bool,
    end_last: bool,
}

impl Default for RuntimeRoadEndpointTopology {
    fn default() -> Self {
        Self {
            start_count: 0,
            end_count: 0,
            start_last: true,
            end_last: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeRoadIntersectionKind {
    Tee,
    FourWay,
}

impl RuntimeRoadIntersectionKind {
    fn from_endpoint_count(count: u32) -> Option<Self> {
        match count {
            2 => Some(Self::Tee),
            3 => Some(Self::FourWay),
            _ => None,
        }
    }

    fn max(self, other: Self) -> Self {
        match (self, other) {
            (Self::FourWay, _) | (_, Self::FourWay) => Self::FourWay,
            _ => Self::Tee,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Tee => "Tee",
            Self::FourWay => "FourWay",
        }
    }
}

#[derive(Debug, Clone)]
struct RuntimeRoadIntersectionCandidate {
    road_type_id: u32,
    kind: RuntimeRoadIntersectionKind,
    anchor_sum: Vec3,
    contribution_count: usize,
    road_width: f32,
    width_in_texture: f32,
    direction_sum: Vec3,
    fallback_direction: Option<Vec3>,
}

impl RuntimeRoadIntersectionCandidate {
    fn new(
        road_type_id: u32,
        kind: RuntimeRoadIntersectionKind,
        anchor: Vec3,
        road_width: f32,
        width_in_texture: f32,
        direction: Vec3,
    ) -> Self {
        Self {
            road_type_id,
            kind,
            anchor_sum: anchor,
            contribution_count: 1,
            road_width,
            width_in_texture,
            direction_sum: direction,
            fallback_direction: Some(direction),
        }
    }

    fn add_contribution(
        &mut self,
        anchor: Vec3,
        road_width: f32,
        width_in_texture: f32,
        direction: Vec3,
        kind: RuntimeRoadIntersectionKind,
    ) {
        self.kind = self.kind.max(kind);
        self.anchor_sum += anchor;
        self.contribution_count += 1;
        self.road_width = self.road_width.max(road_width);
        self.width_in_texture = self.width_in_texture.max(width_in_texture);
        self.direction_sum += direction;
        if self.fallback_direction.is_none() {
            self.fallback_direction = Some(direction);
        }
    }

    fn into_runtime_segment(self) -> Option<RuntimeRoadVisualSegment> {
        if self.contribution_count == 0 {
            return None;
        }

        let anchor = self.anchor_sum / self.contribution_count as f32;
        let mut direction = if self.direction_sum.length_squared() > 1.0e-6 {
            self.direction_sum.normalize()
        } else {
            self.fallback_direction.unwrap_or(Vec3::ZERO)
        };
        direction.y = 0.0;
        direction = direction.normalize_or_zero();
        if direction.length_squared() <= 1.0e-6 {
            return None;
        }

        let total_length = (self.road_width.max(1.0)
            * match self.kind {
                RuntimeRoadIntersectionKind::Tee => 0.35,
                RuntimeRoadIntersectionKind::FourWay => 0.5,
            })
        .max(1.0);
        let offset = direction * (total_length * 0.5);
        let start = anchor - offset;
        let end = anchor + offset;
        if (end - start).length_squared() <= 1.0e-4 {
            return None;
        }

        Some(RuntimeRoadVisualSegment {
            start: start.to_array(),
            end: end.to_array(),
            width: self.road_width.max(0.1),
            template_name: format!(
                "SyntheticIntersection_{}_{}",
                self.road_type_id,
                self.kind.label()
            ),
            width_in_texture: self.width_in_texture.max(0.0),
            road_type_id: self.road_type_id,
            start_is_angled: false,
            start_is_join: true,
            end_is_angled: false,
            end_is_join: true,
            curve_radius: 0.0,
        })
    }
}

/// Terrain LOD levels matching C++ implementation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainVisualLOD {
    Invalid = 0,
    Min = 1,
    StretchNoClouds = 2,
    HalfClouds = 3,
    NoClouds = 4,
    StretchClouds = 5,
    NoWater = 6,
    Max = 7,
    Automatic = 8,
    Disable = 9,
}

impl Default for TerrainVisualLOD {
    fn default() -> Self {
        TerrainVisualLOD::Automatic
    }
}

/// Seismic simulation for dynamic terrain effects
#[derive(Debug, Clone)]
pub struct SeismicSimulationNode {
    pub center: Vec3,
    pub radius: f32,
    pub region: (Vec3, Vec3), // min, max
    pub clean: bool,
    pub magnitude: f32,
    pub life: u32,
}

impl SeismicSimulationNode {
    pub fn new(center: Vec3, radius: f32, magnitude: f32) -> Self {
        let region_size = radius;
        Self {
            center,
            radius: (radius - 1.0),
            region: (
                Vec3::new(center.x - region_size, center.y, center.z - region_size),
                Vec3::new(center.x + region_size, center.y, center.z + region_size),
            ),
            clean: false,
            magnitude,
            life: 0,
        }
    }
}

/// Main terrain visual implementation matching C++ TerrainVisual
pub struct TerrainVisualImpl {
    /// Configuration settings
    config: TerrainConfig,

    /// Performance statistics
    stats: TerrainStats,

    /// Terrain enabled/disabled
    enabled: bool,

    /// Current LOD setting
    lod_setting: TerrainVisualLOD,

    /// Terrain filename
    filename: String,

    /// Terrain definition sources currently loaded
    loaded_terrain_sources: Vec<PathBuf>,

    /// Height map data
    height_map: Option<HeightMap>,

    /// Chunk management system
    chunk_manager: ChunkManager,

    /// Texture management
    texture_system: TerrainTextures,

    /// Water rendering system
    water_system: WaterSystem,

    /// Road rendering system
    road_system: RoadSystem,

    /// Sun direction for lighting
    sun_direction: Vec3,
    /// Sun color
    sun_color: [f32; 3],
    /// Ambient lighting color
    ambient_color: [f32; 3],
    /// Fog color
    fog_color: [f32; 3],
    /// Fog start distance
    fog_start: f32,
    /// Fog end distance
    fog_end: f32,
    /// Accumulated time for simple day/night effects
    time: f32,

    /// WGPU rendering resources
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,

    /// Terrain uniform buffer
    uniform_buffer: Option<Buffer>,

    /// Terrain shaders
    terrain_pipeline: Option<wgpu::RenderPipeline>,
    terrain_depth_pipeline: Option<wgpu::RenderPipeline>,
    water_pipeline: Option<wgpu::RenderPipeline>,
    road_pipeline: Option<wgpu::RenderPipeline>,

    /// Terrain textures
    heightmap_texture: Option<Texture>,
    blend_texture: Option<Texture>,
    detail_textures: Vec<Texture>,
    skybox_textures: [Option<Texture>; 5],
    skybox_background_view: Option<TextureView>,
    skybox_background_bind_group: Option<BindGroup>,
    skybox_background_pipeline: Option<wgpu::RenderPipeline>,
    skybox_background_bind_group_layout: Option<Arc<wgpu::BindGroupLayout>>,
    skybox_sampler: Option<Sampler>,

    /// Seismic simulation
    seismic_simulations: Vec<SeismicSimulationNode>,

    /// Water grid enabled
    water_grid_enabled: bool,

    /// Static water handle
    grid_water_handle: WaterHandle,

    /// Cached GPU meshes for terrain chunks
    chunk_meshes: HashMap<ChunkId, GpuChunkMesh>,

    /// Rule set for procedural texture selection
    texture_rules: Vec<TextureRule>,

    /// Global C++-style water plane rendered for the active map.
    water_plane: Option<GpuWaterPlane>,

    /// Cached GPU meshes for visible road surfaces.
    road_meshes: Vec<GpuRoadMesh>,

    /// Camera bind group layout used by the terrain pipeline
    terrain_camera_bind_group_layout: Option<Arc<wgpu::BindGroupLayout>>,

    /// Texture bind group layout used by the terrain pipeline
    terrain_texture_bind_group_layout: Option<Arc<wgpu::BindGroupLayout>>,

    /// Camera bind group providing view/projection matrices
    terrain_camera_bind_group: Option<wgpu::BindGroup>,

    /// Terrain texture sampler used by the shader
    terrain_sampler: Option<wgpu::Sampler>,

    /// Current terrain sampler mode mirrored from GlobalData settings.
    terrain_sampler_mode: Option<TerrainSamplerMode>,

    /// Per-chunk texture bind groups and slot maps
    chunk_texture_bindings: HashMap<ChunkId, ChunkTextureBinding>,

    /// Shared visible-terrain texture set used to keep adjacent chunks on the same slot map.
    active_chunk_texture_ids: Option<[TextureId; MAX_TEXTURES_PER_CHUNK]>,

    /// Current oversize amount (in tiles).
    oversize_amount: i32,

    /// Current terrain draw dimensions in map samples.
    draw_width: i32,
    draw_height: i32,

    /// Current terrain draw origin in map samples.
    draw_origin_x: i32,
    draw_origin_y: i32,
}

struct GpuChunkMesh {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    index_count: u32,
    revision: u64,
}

struct ChunkTextureBinding {
    bind_group: BindGroup,
    slot_map: HashMap<TextureId, usize>,
    texture_ids: [TextureId; MAX_TEXTURES_PER_CHUNK],
    diffuse_views: Vec<Arc<wgpu::TextureView>>,
}

struct GpuWaterPlane {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    index_count: u32,
}

struct GpuRoadMesh {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    index_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerrainSamplerMode {
    texture_lod_bias: u32,
}

impl TerrainSamplerMode {
    fn current() -> Self {
        let texture_lod_bias = get_global_data()
            .map(|global_data| {
                let data = global_data.read();
                data.texture_reduction_factor.clamp(0, 4) as u32
            })
            .unwrap_or(0);

        Self { texture_lod_bias }
    }

    fn to_descriptor(self) -> SamplerDescriptor<'static> {
        SamplerDescriptor {
            label: Some("Terrain Texture Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: self.texture_lod_bias as f32,
            lod_max_clamp: 32.0,
            ..Default::default()
        }
    }
}

const DEFAULT_TERRAIN_COLORS: [[u8; 4]; 4] = [
    [60, 120, 60, 255],   // Grass
    [120, 120, 120, 255], // Cliff
    [240, 240, 240, 255], // Snow
    [194, 162, 96, 255],  // Sand
];

const NORMAL_DRAW_WIDTH: i32 = 129;
const NORMAL_DRAW_HEIGHT: i32 = 129;
const OVERSIZE_TILES_STEP: i32 = 32;
const MAX_OVERSIZE_TILES: i32 = 4;

// The current live terrain path uses four diffuse terrain layers with four
// blend weights active per vertex.
const MAX_TEXTURES_PER_CHUNK: usize = 4;

fn matrix4_to_array(matrix: &Mat4) -> [[f32; 4]; 4] {
    matrix.to_cols_array_2d()
}

impl TerrainVisualImpl {
    pub fn new() -> Self {
        let mut instance = Self {
            config: TerrainConfig::default(),
            stats: TerrainStats::default(),
            enabled: true,
            lod_setting: TerrainVisualLOD::default(),
            filename: String::new(),
            loaded_terrain_sources: Vec::new(),
            height_map: None,
            chunk_manager: ChunkManager::new(),
            texture_system: TerrainTextures::new(),
            water_system: WaterSystem::new(),
            road_system: RoadSystem::new(),
            device: None,
            queue: None,
            uniform_buffer: None,
            terrain_pipeline: None,
            terrain_depth_pipeline: None,
            water_pipeline: None,
            road_pipeline: None,
            heightmap_texture: None,
            blend_texture: None,
            detail_textures: Vec::new(),
            skybox_textures: [None, None, None, None, None],
            skybox_background_view: None,
            skybox_background_bind_group: None,
            skybox_background_pipeline: None,
            skybox_background_bind_group_layout: None,
            skybox_sampler: None,
            seismic_simulations: Vec::new(),
            water_grid_enabled: true,
            grid_water_handle: WaterHandle(0),
            chunk_meshes: HashMap::new(),
            texture_rules: Vec::new(),
            water_plane: None,
            road_meshes: Vec::new(),
            terrain_camera_bind_group_layout: None,
            terrain_texture_bind_group_layout: None,
            terrain_camera_bind_group: None,
            terrain_sampler: None,
            terrain_sampler_mode: None,
            chunk_texture_bindings: HashMap::new(),
            active_chunk_texture_ids: None,
            sun_direction: Vec3::new(0.0, -1.0, 0.0),
            sun_color: [1.0, 0.9, 0.8],
            ambient_color: [0.2, 0.2, 0.2],
            fog_color: [0.5, 0.6, 0.7],
            fog_start: 800.0,
            fog_end: 3000.0,
            time: 0.0,
            oversize_amount: 0,
            draw_width: NORMAL_DRAW_WIDTH,
            draw_height: NORMAL_DRAW_HEIGHT,
            draw_origin_x: 0,
            draw_origin_y: 0,
        };

        instance
    }

    /// Expose chunk manager for renderer passes.
    pub fn chunk_manager(&self) -> &ChunkManager {
        &self.chunk_manager
    }

    /// Number of visible chunks; used to accumulate draw-call stats.
    pub fn chunk_draw_count(&self) -> usize {
        self.visible_chunk_ids_for_draw_area().len()
    }

    /// Apply texture-LOD side effects immediately after a runtime LOD adjustment.
    ///
    /// Matches the intent of C++ `TheTerrainRenderObject->setTextureLOD(...)` called from
    /// `W3DGameClient::adjustLOD`.
    pub fn apply_texture_lod_reduction(&mut self, _reduction: i32) {
        self.terrain_sampler = None;
        self.terrain_sampler_mode = None;
        self.chunk_texture_bindings.clear();
    }

    fn map_sample_dimensions(&self) -> Option<(i32, i32)> {
        self.height_map
            .as_ref()
            .map(|height_map| (height_map.width as i32, height_map.height as i32))
    }

    fn map_scale(&self) -> f32 {
        self.map_sample_dimensions()
            .map(|(width, _height)| {
                (self.config.world_size.0 / width.max(1) as f32).max(f32::EPSILON)
            })
            .or_else(|| {
                self.height_map
                    .as_ref()
                    .map(|height_map| height_map.scale.max(f32::EPSILON))
            })
            .unwrap_or(1.0)
    }

    fn reset_draw_area_state(&mut self) {
        self.oversize_amount = 0;

        if let Some((map_width, map_height)) = self.map_sample_dimensions() {
            self.draw_width = NORMAL_DRAW_WIDTH.min(map_width).max(1);
            self.draw_height = NORMAL_DRAW_HEIGHT.min(map_height).max(1);
            self.draw_origin_x = ((map_width - self.draw_width) / 2).max(0);
            self.draw_origin_y = ((map_height - self.draw_height) / 2).max(0);
        } else {
            self.draw_width = NORMAL_DRAW_WIDTH;
            self.draw_height = NORMAL_DRAW_HEIGHT;
            self.draw_origin_x = 0;
            self.draw_origin_y = 0;
        }

        self.clamp_draw_area_to_map();
    }

    fn recenter_draw_area_on_world_position(&mut self, world_x: f32, world_z: f32) {
        let Some((_map_width, _map_height)) = self.map_sample_dimensions() else {
            return;
        };

        let scale = self.map_scale().max(f32::EPSILON);
        let sample_x = (world_x / scale).floor() as i32;
        let sample_y = (world_z / scale).floor() as i32;
        self.draw_origin_x = sample_x - (self.draw_width / 2);
        self.draw_origin_y = sample_y - (self.draw_height / 2);
        self.clamp_draw_area_to_map();
    }

    fn clamp_draw_area_to_map(&mut self) {
        if let Some((map_width, map_height)) = self.map_sample_dimensions() {
            if map_width > 0 && map_height > 0 {
                self.draw_width = self
                    .draw_width
                    .clamp(0, map_width)
                    .max(1)
                    .min(map_width.max(1));
                self.draw_height = self
                    .draw_height
                    .clamp(0, map_height)
                    .max(1)
                    .min(map_height.max(1));

                let max_origin_x = (map_width - self.draw_width).max(0);
                let max_origin_y = (map_height - self.draw_height).max(0);
                if self.draw_origin_x < 0 {
                    self.draw_origin_x = 0;
                }
                if self.draw_origin_y < 0 {
                    self.draw_origin_y = 0;
                }
                if self.draw_origin_x > max_origin_x {
                    self.draw_origin_x = max_origin_x;
                }
                if self.draw_origin_y > max_origin_y {
                    self.draw_origin_y = max_origin_y;
                }
            }
        }
    }

    fn draw_area_bounds_world(&self) -> (f32, f32, f32, f32) {
        let Some((map_width, map_height)) = self.map_sample_dimensions() else {
            return (0.0, 0.0, self.config.world_size.0, self.config.world_size.1);
        };

        let scale = self.map_scale();
        let width = (map_width.max(1) as f32) * scale;
        let height = (map_height.max(1) as f32) * scale;

        let origin_x = (self.draw_origin_x.max(0) as f32) * self.map_scale();
        let origin_y = (self.draw_origin_y.max(0) as f32) * self.map_scale();
        let max_x = (((self.draw_origin_x + self.draw_width).max(self.draw_origin_x) as f32)
            * scale)
            .max(0.0)
            .min(width);
        let max_y = (((self.draw_origin_y + self.draw_height).max(self.draw_origin_y) as f32)
            * scale)
            .max(0.0)
            .min(height);
        (origin_x, origin_y, max_x, max_y)
    }

    fn chunk_intersects_draw_area(&self, chunk: &crate::terrain::chunk::TerrainChunk) -> bool {
        let (min_x, min_y, max_x, max_y) = self.draw_area_bounds_world();
        chunk.bounds.max.x > min_x
            && chunk.bounds.min.x < max_x
            && chunk.bounds.max.z > min_y
            && chunk.bounds.min.z < max_y
    }

    fn visible_chunk_ids_for_draw_area(&self) -> Vec<ChunkId> {
        let chunks = self.chunk_manager.get_visible_chunks();
        let mut chunk_ids: Vec<ChunkId> = match self.map_sample_dimensions() {
            Some(_) => chunks
                .into_iter()
                .filter(|chunk| self.chunk_intersects_draw_area(chunk))
                .map(|chunk| chunk.id)
                .collect(),
            None => chunks.into_iter().map(|chunk| chunk.id).collect(),
        };
        chunk_ids.sort_unstable();
        chunk_ids
    }

    /// Current world size in world units.
    pub fn world_size(&self) -> (f32, f32) {
        self.config.world_size
    }

    pub fn set_world_size(&mut self, width: f32, height: f32) {
        self.config.world_size = (width.max(1.0), height.max(1.0));
        self.chunk_manager.set_config(self.config.clone());
        self.reset_draw_area_state();
    }

    pub fn debug_heightmap_loaded(&self) -> bool {
        self.height_map.is_some()
    }

    /// C++ `W3DTerrainVisual::getRawMapHeight`.
    pub fn get_raw_map_height(&self, grid_x: i32, grid_y: i32) -> i32 {
        let Some(height_map) = self.height_map.as_ref() else {
            return 0;
        };
        let x = grid_x + height_map.border_size;
        let y = grid_y + height_map.border_size;
        height_map.get_raw_height(x, y) as i32
    }

    /// C++ `W3DTerrainVisual::setRawMapHeight`.
    pub fn set_raw_map_height(&mut self, grid_x: i32, grid_y: i32, height: i32) {
        let Some(height_map) = self.height_map.as_mut() else {
            return;
        };
        let x = grid_x + height_map.border_size;
        let y = grid_y + height_map.border_size;
        if height_map.get_raw_height(x, y) as i32 > height {
            height_map.set_raw_height(x, y, height.clamp(0, u8::MAX as i32) as u8);
        }
    }

    pub fn debug_total_chunk_count(&self) -> usize {
        self.chunk_manager.total_chunk_count()
    }

    pub fn debug_visible_chunk_count(&self) -> usize {
        self.chunk_manager.get_visible_chunks().len()
    }

    pub fn debug_renderable_visible_chunk_count(&self) -> usize {
        self.chunk_manager.renderable_chunk_count()
    }

    pub fn debug_pending_visible_chunk_count(&self) -> usize {
        self.chunk_manager.pending_visible_chunk_count()
    }

    pub fn debug_chunk_summary(&self) -> String {
        self.chunk_manager.render_diagnostic_summary()
    }

    /// Export minimap-ready road samples for static map overlays.
    pub fn minimap_road_samples(&self, samples_per_segment: u32) -> Vec<RoadMinimapSample> {
        self.road_system
            .snapshot_minimap_samples(samples_per_segment)
    }

    fn quantize_runtime_road_coord(value: f32) -> i32 {
        (value * 100.0).round() as i32
    }

    fn runtime_points_equal(a: Vec3, b: Vec3) -> bool {
        Self::quantize_runtime_road_coord(a.x) == Self::quantize_runtime_road_coord(b.x)
            && Self::quantize_runtime_road_coord(a.y) == Self::quantize_runtime_road_coord(b.y)
            && Self::quantize_runtime_road_coord(a.z) == Self::quantize_runtime_road_coord(b.z)
    }

    fn runtime_endpoint_match_count(
        ordered_segments: &[RuntimeRoadVisualSegment],
        road_type_id: u32,
        point: Vec3,
    ) -> usize {
        ordered_segments
            .iter()
            .filter(|segment| segment.road_type_id == road_type_id)
            .map(|segment| {
                let start = Vec3::from_array(segment.start);
                let end = Vec3::from_array(segment.end);
                let mut matches = 0usize;
                if Self::runtime_points_equal(start, point) {
                    matches += 1;
                }
                if Self::runtime_points_equal(end, point) {
                    matches += 1;
                }
                matches
            })
            .sum()
    }

    fn runtime_ground_from_point(point: Vec3) -> Vec2 {
        // Runtime road shaping follows the C++ XY map plane; in Rust terrain space this is XZ.
        Vec2::new(point.x, point.z)
    }

    fn runtime_point_from_ground(ground: Vec2, height: f32) -> [f32; 3] {
        [ground.x, height, ground.y]
    }

    fn runtime_rotate_2d(vector: Vec2, angle: f32) -> Vec2 {
        let sin = angle.sin();
        let cos = angle.cos();
        Vec2::new(
            vector.x * cos - vector.y * sin,
            vector.x * sin + vector.y * cos,
        )
    }

    fn runtime_rotate_about(point: Vec2, center: Vec2, angle: f32) -> Vec2 {
        center + Self::runtime_rotate_2d(point - center, angle)
    }

    fn runtime_line_intersection(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> Option<Vec2> {
        let r = a2 - a1;
        let s = b2 - b1;
        let denom = r.perp_dot(s);
        if denom.abs() <= 1.0e-6 {
            return None;
        }
        let t = (b1 - a1).perp_dot(s) / denom;
        Some(a1 + r * t)
    }

    fn append_runtime_curve_segment(
        curves: &mut Vec<RuntimeRoadVisualSegment>,
        source: &RuntimeRoadVisualSegment,
        start: Vec2,
        end: Vec2,
        height: f32,
    ) {
        if (end - start).length_squared() <= 1.0e-5 {
            return;
        }

        curves.push(RuntimeRoadVisualSegment {
            start: Self::runtime_point_from_ground(start, height),
            end: Self::runtime_point_from_ground(end, height),
            width: source.width.max(0.1),
            template_name: source.template_name.clone(),
            width_in_texture: source.width_in_texture.max(0.0),
            road_type_id: source.road_type_id,
            start_is_angled: false,
            start_is_join: false,
            end_is_angled: false,
            end_is_join: false,
            curve_radius: source.curve_radius,
        });
    }

    fn insert_runtime_curve_segment_at(
        segment_a: &mut RuntimeRoadVisualSegment,
        segment_b: &mut RuntimeRoadVisualSegment,
    ) -> Vec<RuntimeRoadVisualSegment> {
        const DOT_LIMIT: f32 = 0.5;

        if segment_a.road_type_id != segment_b.road_type_id {
            return Vec::new();
        }

        let radius = segment_a.curve_radius.max(0.0) * segment_a.width.max(0.0);
        if radius <= 1.0e-3 {
            return Vec::new();
        }

        let source_a = segment_a.clone();
        let shared_height = (segment_a.start[1] + segment_b.end[1]) * 0.5;

        let seg_a_start = Self::runtime_ground_from_point(Vec3::from_array(segment_a.start));
        let seg_a_end = Self::runtime_ground_from_point(Vec3::from_array(segment_a.end));
        let seg_b_start = Self::runtime_ground_from_point(Vec3::from_array(segment_b.start));
        let seg_b_end = Self::runtime_ground_from_point(Vec3::from_array(segment_b.end));

        let original_corner = seg_a_start;

        let mut line1_dir = (seg_a_end - seg_a_start).normalize_or_zero();
        let mut line2_dir = (seg_b_end - seg_b_start).normalize_or_zero();
        if line1_dir.length_squared() <= 1.0e-6 || line2_dir.length_squared() <= 1.0e-6 {
            return Vec::new();
        }

        let turn_cross = line1_dir.perp_dot(line2_dir);
        let turn_right = turn_cross > 0.0;

        let mut pr1;
        let pr2;
        let pr3;
        let mut pr4;
        if turn_right {
            pr1 = seg_a_start;
            pr2 = seg_a_end;
            pr3 = seg_b_start;
            pr4 = seg_b_end;
        } else {
            pr4 = seg_a_start;
            pr3 = seg_a_end;
            pr2 = seg_b_start;
            pr1 = seg_b_end;
            line1_dir = (pr2 - pr1).normalize_or_zero();
            line2_dir = (pr4 - pr3).normalize_or_zero();
        }

        if line1_dir.length_squared() <= 1.0e-6 || line2_dir.length_squared() <= 1.0e-6 {
            return Vec::new();
        }

        let cur_sin = line1_dir.dot(line2_dir).clamp(-1.0, 1.0);
        let angle = cur_sin.acos();
        let count = angle / (PI / 6.0);
        if count < 0.9 || segment_a.start_is_angled {
            return Vec::new();
        }

        let offset1 = Self::runtime_rotate_2d(line1_dir * radius, -PI / 2.0);
        let offset2 = Self::runtime_rotate_2d(line2_dir * radius, -PI / 2.0);

        let offset_intersection = Self::runtime_line_intersection(
            pr1 + offset1,
            pr2 + offset1,
            pr3 + offset2,
            pr4 + offset2,
        );
        let Some(mut p_int1) = offset_intersection else {
            return Vec::new();
        };

        let cross1_intersection =
            Self::runtime_line_intersection(p_int1, p_int1 - offset2, pr3, pr4);
        let cross2_intersection =
            Self::runtime_line_intersection(p_int1, p_int1 - offset1, pr1, pr2);
        let (Some(cross1), Some(p_int3)) = (cross1_intersection, cross2_intersection) else {
            return Vec::new();
        };
        p_int1 = cross1;

        let dot1 = line2_dir.dot(p_int1 - pr3);
        let dot2 = line1_dir.dot(pr2 - p_int3);
        if dot1 < DOT_LIMIT || dot2 < DOT_LIMIT {
            segment_a.start = Self::runtime_point_from_ground(original_corner, segment_a.start[1]);
            segment_b.end = Self::runtime_point_from_ground(original_corner, segment_b.end[1]);
            return Vec::new();
        }

        pr4 = p_int1;
        let mut curves = Vec::new();

        let angle_step = -PI / 6.0;
        let mut pt2 = pr4;
        let mut pt1 = pr3;
        let mut direction = pt1 - pt2;
        let mut center_of_curve = Vec2::new(-direction.y, direction.x).normalize_or_zero();
        if center_of_curve.length_squared() <= 1.0e-6 {
            return Vec::new();
        }
        center_of_curve *= radius;
        center_of_curve += pt2;

        pt2 = Self::runtime_rotate_about(pt2, center_of_curve, angle_step);
        direction = Self::runtime_rotate_2d(direction, angle_step);
        pt1 = pt2 + direction;
        Self::append_runtime_curve_segment(&mut curves, &source_a, pt2, pt1, shared_height);

        if count > 2.0 {
            let mut i = 2_i32;
            while (i as f32) < count {
                direction = Self::runtime_rotate_2d(direction, angle_step);
                pt2 = Self::runtime_rotate_about(pt2, center_of_curve, angle_step);
                pt1 = pt2 + direction;
                Self::append_runtime_curve_segment(&mut curves, &source_a, pt2, pt1, shared_height);
                i += 1;
            }
        }

        pr1 = p_int3;
        if count > 1.0 {
            pt2 = pr1;
            pt1 = pr1 + pr1 - pr2;
            direction = pt1 - pt2;
            pt1 = pt2 + direction;
            Self::append_runtime_curve_segment(&mut curves, &source_a, pt2, pt1, shared_height);
        }

        if turn_right {
            segment_a.start = Self::runtime_point_from_ground(pr1, segment_a.start[1]);
            segment_b.end = Self::runtime_point_from_ground(pr4, segment_b.end[1]);
        } else {
            segment_a.start = Self::runtime_point_from_ground(pr4, segment_a.start[1]);
            segment_b.end = Self::runtime_point_from_ground(pr1, segment_b.end[1]);
        }

        curves
    }

    fn insert_runtime_curve_segments(
        ordered_segments: &mut Vec<RuntimeRoadVisualSegment>,
        topology: &[RuntimeRoadEndpointTopology],
    ) {
        let original_segment_count = ordered_segments.len().min(topology.len());
        if original_segment_count <= 1 {
            return;
        }

        let mut segment_start_index: Option<usize> = None;
        for i in 0..original_segment_count {
            let mut try_insert_pair = None::<(usize, usize)>;
            let adjacent_match = if i + 1 < original_segment_count {
                let current_start = Vec3::from_array(ordered_segments[i].start);
                let next_end = Vec3::from_array(ordered_segments[i + 1].end);
                Self::runtime_points_equal(current_start, next_end)
            } else {
                false
            };

            if adjacent_match {
                if topology[i + 1].end_count == 1 && topology[i].start_count == 1 {
                    try_insert_pair = Some((i, i + 1));
                    if segment_start_index.is_none() {
                        segment_start_index = Some(i);
                    }
                }
            } else if let Some(start_index) = segment_start_index {
                let current_start = Vec3::from_array(ordered_segments[i].start);
                let start_end = Vec3::from_array(ordered_segments[start_index].end);
                if Self::runtime_points_equal(current_start, start_end)
                    && topology[start_index].end_count == 1
                    && topology[i].start_count == 1
                {
                    try_insert_pair = Some((i, start_index));
                }
                segment_start_index = None;
            }

            let Some((a, b)) = try_insert_pair else {
                continue;
            };
            if a == b {
                continue;
            }

            let curves = if a < b {
                let (left, right) = ordered_segments.split_at_mut(b);
                Self::insert_runtime_curve_segment_at(&mut left[a], &mut right[0])
            } else {
                let (left, right) = ordered_segments.split_at_mut(a);
                Self::insert_runtime_curve_segment_at(&mut right[0], &mut left[b])
            };
            if !curves.is_empty() {
                ordered_segments.extend(curves);
            }
        }
    }

    fn runtime_xp_sign(v1: Vec2, v2: Vec2) -> i32 {
        let cross = v1.perp_dot(v2);
        if cross < 0.0 {
            -1
        } else if cross > 0.0 {
            1
        } else {
            0
        }
    }

    fn runtime_closest_point_on_segment(point: Vec2, a: Vec2, b: Vec2) -> Vec2 {
        let ab = b - a;
        let denom = ab.length_squared();
        if denom <= 1.0e-6 {
            return a;
        }
        let t = ((point - a).dot(ab) / denom).clamp(0.0, 1.0);
        a + ab * t
    }

    fn runtime_adjust_stacking(
        stackings: &mut HashMap<u32, i32>,
        top_unique_id: u32,
        bottom_unique_id: u32,
    ) {
        let Some(top_stacking) = stackings.get(&top_unique_id).copied() else {
            return;
        };
        let Some(bottom_stacking) = stackings.get(&bottom_unique_id).copied() else {
            return;
        };
        if top_stacking > bottom_stacking {
            return;
        }

        let new_stacking = bottom_stacking;
        for stacking in stackings.values_mut() {
            if *stacking > new_stacking {
                *stacking += 1;
            }
        }
        if let Some(stacking) = stackings.get_mut(&top_unique_id) {
            *stacking = new_stacking + 1;
        }
    }

    fn runtime_find_cross_type_join_vector(
        loc: Vec2,
        join_vector: Vec2,
        unique_id: u32,
        ordered_segments: &[RuntimeRoadVisualSegment],
        base_segment_count: usize,
    ) -> Option<(u32, Vec2)> {
        let mut new_vector = join_vector;
        for segment in ordered_segments.iter().take(base_segment_count) {
            if segment.road_type_id == unique_id {
                continue;
            }

            let loc1 = Self::runtime_ground_from_point(Vec3::from_array(segment.start));
            let loc2 = Self::runtime_ground_from_point(Vec3::from_array(segment.end));
            let half_width = segment.width.max(0.1) * 0.5;
            let bounds_min = Vec2::new(
                loc1.x.min(loc2.x) - half_width,
                loc1.y.min(loc2.y) - half_width,
            );
            let bounds_max = Vec2::new(
                loc1.x.max(loc2.x) + half_width,
                loc1.y.max(loc2.y) + half_width,
            );
            if loc.x < bounds_min.x
                || loc.y < bounds_min.y
                || loc.x > bounds_max.x
                || loc.y > bounds_max.y
            {
                continue;
            }

            let closest = Self::runtime_closest_point_on_segment(loc, loc1, loc2);
            let dist = (closest - loc).length();
            if dist >= segment.width.max(0.1) * 0.55 {
                continue;
            }

            let mut road_vec = loc2 - loc1;
            if Self::runtime_xp_sign(road_vec, join_vector) == 1 {
                road_vec = Self::runtime_rotate_2d(road_vec, PI / 2.0);
            } else {
                road_vec = Self::runtime_rotate_2d(road_vec, -PI / 2.0);
            }
            new_vector = road_vec;
            return Some((segment.road_type_id, new_vector));
        }
        None
    }

    fn synthesize_runtime_cross_type_join_segments(
        ordered_segments: &[RuntimeRoadVisualSegment],
        topology: &[RuntimeRoadEndpointTopology],
    ) -> (Vec<RuntimeRoadVisualSegment>, HashMap<u32, i32>) {
        let base_segment_count = ordered_segments.len().min(topology.len());
        let mut stackings: HashMap<u32, i32> = HashMap::new();
        let mut next_stacking = 0_i32;
        for segment in ordered_segments.iter() {
            stackings.entry(segment.road_type_id).or_insert_with(|| {
                let stacking = next_stacking;
                next_stacking += 1;
                stacking
            });
        }

        let mut joins = Vec::new();
        for (index, segment) in ordered_segments.iter().take(base_segment_count).enumerate() {
            let Some(endpoint) = topology.get(index).copied() else {
                continue;
            };

            let (is_start_endpoint, loc1, loc2) = if endpoint.end_count == 0 && segment.end_is_join
            {
                (
                    false,
                    Vec3::from_array(segment.end),
                    Vec3::from_array(segment.start),
                )
            } else if endpoint.start_count == 0 && segment.start_is_join {
                (
                    true,
                    Vec3::from_array(segment.start),
                    Vec3::from_array(segment.end),
                )
            } else {
                continue;
            };

            let loc1_2d = Self::runtime_ground_from_point(loc1);
            let loc2_2d = Self::runtime_ground_from_point(loc2);
            let mut road_vector = (loc2_2d - loc1_2d).normalize_or_zero();
            if road_vector.length_squared() <= 1.0e-6 {
                continue;
            }

            let mut join_vector = road_vector;
            let other_id = Self::runtime_find_cross_type_join_vector(
                loc1_2d,
                join_vector,
                segment.road_type_id,
                ordered_segments,
                base_segment_count,
            );
            if let Some((other_unique_id, resolved)) = other_id {
                join_vector = resolved;
                Self::runtime_adjust_stacking(
                    &mut stackings,
                    segment.road_type_id,
                    other_unique_id,
                );
            } else {
                join_vector *= 100.0;
            }

            let road_normal = Vec2::new(-road_vector.y, road_vector.x);
            let join_normal = Vec2::new(-join_vector.y, join_vector.x);
            let half_effective_width =
                (segment.width.max(0.1) * segment.width_in_texture.max(0.1)) * 0.5;

            let upper_start = loc1_2d + road_normal * half_effective_width;
            let upper_end = loc2_2d + road_normal * half_effective_width;
            let lower_start = loc1_2d - road_normal * half_effective_width;
            let lower_end = loc2_2d - road_normal * half_effective_width;

            let join_line_end = loc1_2d + join_normal;
            let top_intersection =
                Self::runtime_line_intersection(loc1_2d, join_line_end, upper_start, upper_end)
                    .unwrap_or(upper_start);
            let bottom_intersection =
                Self::runtime_line_intersection(loc1_2d, join_line_end, lower_start, lower_end)
                    .unwrap_or(lower_start);

            let width_denom = (segment.width.max(0.1) * segment.width_in_texture.max(0.1)).max(0.1);
            let scale_adjustment = (bottom_intersection - top_intersection).length() / width_denom;
            let join_width = (segment.width * scale_adjustment.max(0.1)).max(0.1);
            let join_width_in_texture = (segment.width * scale_adjustment.max(0.1)).max(0.1);

            let join_start_height = if is_start_endpoint {
                segment.start[1]
            } else {
                segment.end[1]
            };
            joins.push(RuntimeRoadVisualSegment {
                start: Self::runtime_point_from_ground(loc1_2d, join_start_height),
                end: Self::runtime_point_from_ground(loc1_2d + join_vector, join_start_height),
                width: join_width,
                template_name: "__SYNTH_ALPHA_JOIN__".to_string(),
                width_in_texture: join_width_in_texture,
                road_type_id: segment.road_type_id,
                start_is_angled: false,
                start_is_join: false,
                end_is_angled: false,
                end_is_join: false,
                curve_radius: segment.curve_radius,
            });
        }

        (joins, stackings)
    }

    fn runtime_road_priority_for_type(
        stackings: &HashMap<u32, i32>,
        road_type_id: u32,
        base_priority: u8,
    ) -> u8 {
        let stacking = stackings.get(&road_type_id).copied().unwrap_or_default();
        base_priority.saturating_add(stacking.max(0).min(i32::from(u8::MAX - base_priority)) as u8)
    }

    fn insert_runtime_road_segment_ordered(
        ordered_segments: &mut Vec<RuntimeRoadVisualSegment>,
        mut candidate: RuntimeRoadVisualSegment,
    ) {
        let mut start = Vec3::from_array(candidate.start);
        let mut end = Vec3::from_array(candidate.end);

        // Match C++ duplicate segment rejection (same type, either orientation).
        for existing in ordered_segments.iter() {
            if existing.road_type_id != candidate.road_type_id {
                continue;
            }
            let existing_start = Vec3::from_array(existing.start);
            let existing_end = Vec3::from_array(existing.end);
            let same_orientation = Self::runtime_points_equal(existing_start, start)
                && Self::runtime_points_equal(existing_end, end);
            let flipped_orientation = Self::runtime_points_equal(existing_start, end)
                && Self::runtime_points_equal(existing_end, start);
            if same_orientation || flipped_orientation {
                return;
            }
        }

        let pt1_count =
            Self::runtime_endpoint_match_count(ordered_segments, candidate.road_type_id, start);
        let pt2_count =
            Self::runtime_endpoint_match_count(ordered_segments, candidate.road_type_id, end);

        let mut flip = false;
        let mut add_before = false;
        let mut add_after = false;
        let mut add_index = ordered_segments.len();

        for (index, existing) in ordered_segments.iter().enumerate() {
            if existing.road_type_id != candidate.road_type_id {
                continue;
            }
            let existing_start = Vec3::from_array(existing.start);
            let existing_end = Vec3::from_array(existing.end);

            if pt1_count == 1 {
                if Self::runtime_points_equal(existing_start, start) {
                    flip = true;
                    add_after = true;
                    add_index = index;
                }
                if Self::runtime_points_equal(existing_end, start) {
                    flip = false;
                    add_before = true;
                    add_index = index;
                }
            }
            if pt2_count == 1 {
                if Self::runtime_points_equal(existing_start, end) {
                    flip = false;
                    add_after = true;
                    add_index = index;
                }
                if Self::runtime_points_equal(existing_end, end) {
                    flip = true;
                    add_before = true;
                    add_index = index;
                }
            }

            if add_before || add_after {
                break;
            }
        }

        if flip {
            std::mem::swap(&mut start, &mut end);
            std::mem::swap(&mut candidate.start_is_angled, &mut candidate.end_is_angled);
            std::mem::swap(&mut candidate.start_is_join, &mut candidate.end_is_join);
        }

        candidate.start = start.to_array();
        candidate.end = end.to_array();

        if add_after {
            add_index += 1;
        }
        add_index = add_index.min(ordered_segments.len());
        ordered_segments.insert(add_index, candidate);
    }

    fn compute_runtime_road_topology(
        ordered_segments: &[RuntimeRoadVisualSegment],
    ) -> Vec<RuntimeRoadEndpointTopology> {
        let mut topology = vec![RuntimeRoadEndpointTopology::default(); ordered_segments.len()];
        if ordered_segments.len() <= 1 {
            return topology;
        }

        // Mirrors C++ W3DRoadBuffer::updateCountsAndFlags ordering semantics.
        for j in (1..ordered_segments.len()).rev() {
            let seg_j = &ordered_segments[j];
            let j_start = Vec3::from_array(seg_j.start);
            let j_end = Vec3::from_array(seg_j.end);
            for i in 0..j {
                let seg_i = &ordered_segments[i];
                if seg_i.road_type_id != seg_j.road_type_id {
                    continue;
                }

                let i_start = Vec3::from_array(seg_i.start);
                let i_end = Vec3::from_array(seg_i.end);

                if Self::runtime_points_equal(i_start, j_start) {
                    topology[i].start_last = false;
                    topology[i].start_count += 1;
                    topology[j].start_count += 1;
                }
                if Self::runtime_points_equal(i_start, j_end) {
                    topology[i].start_last = false;
                    topology[i].start_count += 1;
                    topology[j].end_count += 1;
                }
                if Self::runtime_points_equal(i_end, j_start) {
                    topology[i].end_last = false;
                    topology[i].end_count += 1;
                    topology[j].start_count += 1;
                }
                if Self::runtime_points_equal(i_end, j_end) {
                    topology[i].end_last = false;
                    topology[i].end_count += 1;
                    topology[j].end_count += 1;
                }
            }
        }

        topology
    }

    fn synthesize_runtime_intersection_segments(
        ordered_segments: &[RuntimeRoadVisualSegment],
        topology: &[RuntimeRoadEndpointTopology],
    ) -> Vec<RuntimeRoadVisualSegment> {
        let mut candidates: BTreeMap<(u32, i32, i32, i32), RuntimeRoadIntersectionCandidate> =
            BTreeMap::new();
        for (index, segment) in ordered_segments.iter().enumerate() {
            let Some(topology) = topology.get(index).copied() else {
                continue;
            };

            let road_width = if segment.width.is_finite() && segment.width > 0.1 {
                segment.width
            } else {
                8.0
            };
            let width_in_texture = segment.width_in_texture.max(0.0);

            let start = Vec3::from_array(segment.start);
            if let Some(kind) =
                RuntimeRoadIntersectionKind::from_endpoint_count(topology.start_count)
            {
                let mut direction = Vec3::from_array(segment.end) - start;
                direction.y = 0.0;
                if direction.length_squared() > 1.0e-6 {
                    direction = direction.normalize();
                    let key = (
                        segment.road_type_id,
                        Self::quantize_runtime_road_coord(start.x),
                        Self::quantize_runtime_road_coord(start.y),
                        Self::quantize_runtime_road_coord(start.z),
                    );
                    candidates
                        .entry(key)
                        .and_modify(|entry| {
                            entry.add_contribution(
                                start,
                                road_width,
                                width_in_texture,
                                direction,
                                kind,
                            );
                        })
                        .or_insert_with(|| {
                            RuntimeRoadIntersectionCandidate::new(
                                segment.road_type_id,
                                kind,
                                start,
                                road_width,
                                width_in_texture,
                                direction,
                            )
                        });
                }
            }

            let end = Vec3::from_array(segment.end);
            if let Some(kind) = RuntimeRoadIntersectionKind::from_endpoint_count(topology.end_count)
            {
                let mut direction = Vec3::from_array(segment.start) - end;
                direction.y = 0.0;
                if direction.length_squared() > 1.0e-6 {
                    direction = direction.normalize();
                    let key = (
                        segment.road_type_id,
                        Self::quantize_runtime_road_coord(end.x),
                        Self::quantize_runtime_road_coord(end.y),
                        Self::quantize_runtime_road_coord(end.z),
                    );
                    candidates
                        .entry(key)
                        .and_modify(|entry| {
                            entry.add_contribution(
                                end,
                                road_width,
                                width_in_texture,
                                direction,
                                kind,
                            );
                        })
                        .or_insert_with(|| {
                            RuntimeRoadIntersectionCandidate::new(
                                segment.road_type_id,
                                kind,
                                end,
                                road_width,
                                width_in_texture,
                                direction,
                            )
                        });
                }
            }
        }

        let mut synthesized = Vec::new();
        for candidate in candidates.into_values() {
            let Some(synthetic) = candidate.into_runtime_segment() else {
                continue;
            };

            synthesized.push(RuntimeRoadVisualSegment {
                template_name: if synthetic.template_name.ends_with("_FourWay") {
                    "__SYNTH_FOUR_WAY__".to_string()
                } else {
                    "__SYNTH_TEE__".to_string()
                },
                width_in_texture: synthetic.width_in_texture,
                ..synthetic
            });
        }

        synthesized
    }

    /// Replace runtime road mesh sources with map-bridge spans parsed by TerrainLogic.
    pub fn set_runtime_bridge_segments(
        &mut self,
        bridge_segments: &[([f32; 3], [f32; 3], f32, String)],
    ) -> TerrainResult<()> {
        self.set_runtime_map_road_segments(&[], bridge_segments)
    }

    /// Replace runtime road mesh sources with map roads and bridges parsed from map objects.
    pub fn set_runtime_map_road_segments(
        &mut self,
        road_segments: &[RuntimeRoadVisualSegment],
        bridge_segments: &[([f32; 3], [f32; 3], f32, String)],
    ) -> TerrainResult<()> {
        self.road_system.clear();
        self.road_meshes.clear();
        let mut ordered_road_segments = Vec::new();
        for road_segment in road_segments.iter().cloned() {
            Self::insert_runtime_road_segment_ordered(&mut ordered_road_segments, road_segment);
        }
        let endpoint_topology = Self::compute_runtime_road_topology(&ordered_road_segments);
        let synthetic_road_segments = Self::synthesize_runtime_intersection_segments(
            &ordered_road_segments,
            &endpoint_topology,
        );
        Self::insert_runtime_curve_segments(&mut ordered_road_segments, &endpoint_topology);
        let (alpha_join_segments, road_stackings) =
            Self::synthesize_runtime_cross_type_join_segments(
                &ordered_road_segments,
                &endpoint_topology,
            );

        for (index, road_segment) in ordered_road_segments.iter().enumerate() {
            let start = Vec3::from_array(road_segment.start);
            let end = Vec3::from_array(road_segment.end);
            let span = end - start;
            if !span.is_finite() || span.length_squared() <= 1.0e-4 {
                continue;
            }

            let resolved_width = if road_segment.width.is_finite() && road_segment.width > 0.1 {
                road_segment.width
            } else {
                8.0
            };
            let road_id = self.road_system.create_road(
                format!("TerrainRoad_{index}"),
                RoadType::AsphaltRoad {
                    condition: RoadCondition::Good,
                    lane_markings: !(road_segment.start_is_join || road_segment.end_is_join),
                },
            );
            let segment_id =
                self.road_system
                    .create_segment(road_id, start, end, Some(resolved_width))?;
            if let Some(road) = self.road_system.get_road_mut(road_id) {
                if !road_segment.template_name.is_empty() {
                    road.name = road_segment.template_name.clone();
                }
                road.priority = Self::runtime_road_priority_for_type(
                    &road_stackings,
                    road_segment.road_type_id,
                    20,
                );
            }
            if let Some(segment) = self.road_system.get_segment_mut(segment_id) {
                let topology = endpoint_topology.get(index).copied().unwrap_or_default();
                segment.properties.texture_override = Some(format!(
                    "RoadTypeId={} WidthInTexture={:.3} Kind={}",
                    road_segment.road_type_id,
                    road_segment.width_in_texture.max(0.0),
                    if road_segment.template_name == "__SYNTH_TEE__" {
                        "TEE"
                    } else if road_segment.template_name == "__SYNTH_FOUR_WAY__" {
                        "FOUR_WAY"
                    } else if road_segment.template_name == "__SYNTH_ALPHA_JOIN__" {
                        "ALPHA_JOIN"
                    } else if road_segment.curve_radius > 0.0 && index >= endpoint_topology.len() {
                        "CURVE"
                    } else {
                        "SEGMENT"
                    }
                ));
                segment.properties.endpoint_start_count =
                    topology.start_count.min(u8::MAX as u32) as u8;
                segment.properties.endpoint_end_count =
                    topology.end_count.min(u8::MAX as u32) as u8;
                segment.properties.endpoint_start_last = topology.start_last;
                segment.properties.endpoint_end_last = topology.end_last;
                segment.properties.endpoint_start_multi = topology.start_count > 1;
                segment.properties.endpoint_end_multi = topology.end_count > 1;
            }
        }

        for (index, road_segment) in synthetic_road_segments.iter().enumerate() {
            let start = Vec3::from_array(road_segment.start);
            let end = Vec3::from_array(road_segment.end);
            let span = end - start;
            if !span.is_finite() || span.length_squared() <= 1.0e-4 {
                continue;
            }

            let resolved_width = if road_segment.width.is_finite() && road_segment.width > 0.1 {
                road_segment.width
            } else {
                8.0
            };
            let road_id = self.road_system.create_road(
                format!("SyntheticRoad_{index}"),
                RoadType::AsphaltRoad {
                    condition: RoadCondition::Good,
                    lane_markings: false,
                },
            );
            let segment_id =
                self.road_system
                    .create_segment(road_id, start, end, Some(resolved_width))?;
            if let Some(road) = self.road_system.get_road_mut(road_id) {
                if !road_segment.template_name.is_empty() {
                    road.name = road_segment.template_name.clone();
                }
                road.priority = Self::runtime_road_priority_for_type(
                    &road_stackings,
                    road_segment.road_type_id,
                    20,
                );
            }
            if let Some(segment) = self.road_system.get_segment_mut(segment_id) {
                let synthetic_kind = if road_segment.template_name == "__SYNTH_FOUR_WAY__" {
                    RoadSyntheticIntersectionKind::FourWay
                } else {
                    RoadSyntheticIntersectionKind::Tee
                };
                segment.properties.texture_override = Some(format!(
                    "RoadTypeId={} WidthInTexture={:.3} SyntheticIntersection={:?}",
                    road_segment.road_type_id,
                    road_segment.width_in_texture.max(0.0),
                    synthetic_kind
                ));
                segment.properties.synthetic_intersection = Some(synthetic_kind);
            }
        }

        for (index, road_segment) in alpha_join_segments.iter().enumerate() {
            let start = Vec3::from_array(road_segment.start);
            let end = Vec3::from_array(road_segment.end);
            let span = end - start;
            if !span.is_finite() || span.length_squared() <= 1.0e-4 {
                continue;
            }

            let resolved_width = if road_segment.width.is_finite() && road_segment.width > 0.1 {
                road_segment.width
            } else {
                8.0
            };
            let road_id = self.road_system.create_road(
                format!("AlphaJoinRoad_{index}"),
                RoadType::AsphaltRoad {
                    condition: RoadCondition::Good,
                    lane_markings: false,
                },
            );
            let segment_id =
                self.road_system
                    .create_segment(road_id, start, end, Some(resolved_width))?;
            if let Some(road) = self.road_system.get_road_mut(road_id) {
                road.name = "__SYNTH_ALPHA_JOIN__".to_string();
                road.priority = Self::runtime_road_priority_for_type(
                    &road_stackings,
                    road_segment.road_type_id,
                    20,
                );
            }
            if let Some(segment) = self.road_system.get_segment_mut(segment_id) {
                segment.properties.texture_override = Some(format!(
                    "RoadTypeId={} WidthInTexture={:.3} Kind=ALPHA_JOIN",
                    road_segment.road_type_id,
                    road_segment.width_in_texture.max(0.0)
                ));
            }
        }

        for (index, (start, end, width, template_name)) in bridge_segments.iter().enumerate() {
            let start = Vec3::from_array(*start);
            let end = Vec3::from_array(*end);
            let span = end - start;
            if !span.is_finite() || span.length_squared() <= 1.0e-4 {
                continue;
            }

            let resolved_width = if width.is_finite() && *width > 0.1 {
                *width
            } else {
                6.0
            };
            let road_id = self.road_system.create_road(
                format!("BridgeRoad_{index}"),
                RoadType::StoneBridge {
                    arch_count: 1,
                    stone_type: StoneType::Granite,
                },
            );
            self.road_system
                .create_segment(road_id, start, end, Some(resolved_width))?;
            if let Some(road) = self.road_system.get_road_mut(road_id) {
                if !template_name.is_empty() {
                    road.name = template_name.clone();
                }
                road.priority = 40;
            }
        }

        Ok(())
    }

    pub fn oversize_terrain(&mut self, amount: i32) {
        let Some((map_width, map_height)) = self.map_sample_dimensions() else {
            return;
        };
        if map_width <= 0 || map_height <= 0 {
            return;
        }

        let mut width = NORMAL_DRAW_WIDTH;
        let mut height = NORMAL_DRAW_HEIGHT;

        if amount > 0 && amount < MAX_OVERSIZE_TILES {
            width += OVERSIZE_TILES_STEP * amount;
            height += OVERSIZE_TILES_STEP * amount;
            width = width.min(map_width).max(1);
            height = height.min(map_height).max(1);
        }

        let dx = width - self.draw_width;
        let dy = height - self.draw_height;

        self.draw_width = width;
        self.draw_height = height;

        let origin_dx = dx / 2;
        let origin_dy = dy / 2;

        self.draw_origin_x -= origin_dx;
        self.draw_origin_y -= origin_dy;

        if self.draw_origin_x < 0 {
            self.draw_origin_x = 0;
        }
        if self.draw_origin_y < 0 {
            self.draw_origin_y = 0;
        }

        // Keep draw area state consistent with map bounds.
        if self.draw_width > map_width {
            self.draw_width = map_width;
        }
        if self.draw_height > map_height {
            self.draw_height = map_height;
        }

        self.oversize_amount = amount;

        // Full rebuild behavior.
        self.chunk_meshes.clear();
        self.chunk_texture_bindings.clear();
        self.road_meshes.clear();
        self.stats.rendered_chunks = 0;
        self.stats.triangles_rendered = 0;
        self.stats.update_time_ms = 0.0;
    }

    pub fn set_lighting(
        &mut self,
        sun_dir: Option<[f32; 3]>,
        sun_color: Option<[f32; 3]>,
        ambient: Option<[f32; 3]>,
        fog_color: Option<[f32; 3]>,
        fog_range: Option<(f32, f32)>,
    ) {
        if let Some(d) = sun_dir {
            self.sun_direction = Vec3::from_array(d);
        }
        if let Some(c) = sun_color {
            self.sun_color = c;
        }
        if let Some(a) = ambient {
            self.ambient_color = a;
        }
        if let Some(f) = fog_color {
            self.fog_color = f;
        }
        if let Some((start, end)) = fog_range {
            self.fog_start = start;
            self.fog_end = end.max(start + 1.0);
        }
    }

    fn ensure_terrain_definitions(&mut self, reference: Option<&Path>) -> TerrainResult<()> {
        let reference = reference.or_else(|| {
            if self.filename.is_empty() {
                None
            } else {
                Some(Path::new(&self.filename))
            }
        });

        let sources = Self::collect_terrain_ini_sources(reference);
        if sources.is_empty() {
            return Ok(());
        }

        if sources == self.loaded_terrain_sources {
            return Ok(());
        }

        debug!(
            "Loading terrain definitions from: {}",
            sources
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        let count = ini_terrain::load_terrain_definitions(&sources)?;
        if count == 0 {
            warn!(
                "No terrain definitions were loaded from the resolved sources: {}",
                sources
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        self.loaded_terrain_sources = sources;

        let using_fallback_defaults = !self.texture_rules.is_empty()
            && self.texture_rules.iter().all(|rule| {
                self.texture_system
                    .get_texture(rule.texture_id)
                    .map(|texture| texture.diffuse_path.starts_with("Data/Terrain/"))
                    .unwrap_or(false)
            });

        if using_fallback_defaults {
            self.texture_rules.clear();
            self.chunk_texture_bindings.clear();
            self.chunk_meshes.clear();
            self.road_meshes.clear();
            self.active_chunk_texture_ids = None;
            self.ensure_default_textures();
        }

        Ok(())
    }

    fn collect_terrain_ini_sources(reference: Option<&Path>) -> Vec<PathBuf> {
        let mut sources = Vec::new();
        let mut seen: HashSet<PathBuf> = HashSet::new();

        let base_relatives = [
            "Data/INI/Default/Terrain.ini",
            "Data/INI/Default/Terrain.INI",
            "Data/INI/Terrain.ini",
            "Data/INI/Terrain.INI",
        ];

        if let Some(reference_path) = reference {
            Self::collect_from_ancestors(reference_path, &base_relatives, &mut sources, &mut seen);
        }

        if let Ok(cwd) = std::env::current_dir() {
            Self::collect_from_root(&cwd, &base_relatives, &mut sources, &mut seen);
            for ancestor in cwd.ancestors() {
                Self::collect_from_root(ancestor, &base_relatives, &mut sources, &mut seen);
            }
        }

        let fallback_paths = [
            "windows_game/Command & Conquer Generals Zero Hour/Data/INI/Default/Terrain.ini",
            "windows_game/Command & Conquer Generals Zero Hour/Data/INI/Terrain.ini",
            "windows_game/Command & Conquer Generals/Data/INI/Default/Terrain.ini",
            "windows_game/Command & Conquer Generals/Data/INI/Terrain.ini",
            "../windows_game/Command & Conquer Generals Zero Hour/Data/INI/Default/Terrain.ini",
            "../windows_game/Command & Conquer Generals Zero Hour/Data/INI/Terrain.ini",
            "../windows_game/Command & Conquer Generals/Data/INI/Default/Terrain.ini",
            "../windows_game/Command & Conquer Generals/Data/INI/Terrain.ini",
        ];

        for fallback in fallback_paths {
            Self::push_if_exists(&mut sources, &mut seen, PathBuf::from(fallback));
        }

        if let Some(reference_path) = reference {
            if let Some(map_dir) = reference_path.parent() {
                Self::collect_map_specific_sources(map_dir, &mut sources, &mut seen);
            }
        }

        sources
    }

    fn collect_from_ancestors(
        reference: &Path,
        relatives: &[&str],
        sources: &mut Vec<PathBuf>,
        seen: &mut HashSet<PathBuf>,
    ) {
        let mut dirs = Vec::new();

        if reference.is_dir() {
            dirs.push(reference.to_path_buf());
        } else if let Some(parent) = reference.parent() {
            dirs.push(parent.to_path_buf());
        }

        let mut current = reference.parent();
        while let Some(dir) = current {
            dirs.push(dir.to_path_buf());
            current = dir.parent();
        }

        for dir in dirs {
            Self::collect_from_root(&dir, relatives, sources, seen);
        }
    }

    fn collect_from_root(
        root: &Path,
        relatives: &[&str],
        sources: &mut Vec<PathBuf>,
        seen: &mut HashSet<PathBuf>,
    ) {
        for rel in relatives {
            Self::push_if_exists(sources, seen, root.join(rel));
            Self::push_if_exists(sources, seen, root.join("INIZH").join(rel));
        }
    }

    fn collect_map_specific_sources(
        map_dir: &Path,
        sources: &mut Vec<PathBuf>,
        seen: &mut HashSet<PathBuf>,
    ) {
        let candidates = [
            "Terrain.ini",
            "terrain.ini",
            "Terrain.INI",
            "Data/INI/Terrain.ini",
            "Data/INI/Terrain.INI",
        ];

        for candidate in &candidates {
            Self::push_if_exists(sources, seen, map_dir.join(candidate));
        }
    }

    fn push_if_exists(sources: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, candidate: PathBuf) {
        if !candidate.exists() {
            return;
        }

        let canonical = candidate
            .canonicalize()
            .unwrap_or_else(|_| candidate.clone());

        if seen.insert(canonical.clone()) {
            sources.push(canonical);
        }
    }

    fn ensure_default_textures(&mut self) {
        if !self.texture_rules.is_empty() {
            return;
        }

        let mut defaults = Vec::new();

        if let Some(registry) = ini_terrain::get_terrain_types() {
            let guard = registry.read();
            let desired_surfaces = [
                TerrainSurface::Grass,
                TerrainSurface::Rock,
                TerrainSurface::Snow,
                TerrainSurface::Sand,
            ];
            let mut used_textures = HashSet::new();

            for surface in desired_surfaces {
                let terrains = guard.get_terrains_by_surface(&surface);
                for terrain in terrains {
                    let texture = terrain.texture_name.as_str().trim();
                    if texture.is_empty() {
                        continue;
                    }
                    let Some(normalized_texture) = Self::normalize_terrain_texture_path(texture)
                    else {
                        continue;
                    };

                    let texture_key = normalized_texture.to_ascii_lowercase();
                    if !used_textures.insert(texture_key) {
                        continue;
                    }

                    if !TerrainTextures::is_available_terrain_texture_path(&normalized_texture) {
                        continue;
                    }

                    let texture_id = self.texture_system.register_texture(TerrainTexture::new(
                        0,
                        terrain.name.as_str().to_string(),
                        normalized_texture,
                    ));
                    defaults.push(texture_id);
                    break;
                }
            }
        }

        // If no INI terrain types resolve, leave rules empty (mirror C++ missing-terrain behavior).

        // Startup parity/perf: decode default terrain textures before first menu frame so
        // terrain does not appear to stream tile-by-tile after shell-map load.
        for texture_id in &defaults {
            if let Err(err) = self.texture_system.load_texture(*texture_id) {
                warn!(
                    "Failed to preload startup terrain texture {}: {}",
                    texture_id, err
                );
            }
        }

        self.build_rules_from_textures(&defaults);
    }

    fn build_rules_from_textures(&mut self, texture_ids: &[TextureId]) {
        let mut rules = Vec::new();

        for &texture_id in texture_ids {
            if rules
                .iter()
                .any(|rule: &TextureRule| rule.texture_id == texture_id)
            {
                continue;
            }

            if let Some(texture) = self.texture_system.get_texture(texture_id) {
                if let Some(terrain_type) = self.find_terrain_type_for_texture(texture) {
                    rules.push(Self::rule_from_terrain_type(texture_id, &terrain_type));
                } else {
                    rules.push(Self::derive_rule_for_texture(texture_id, &texture.name));
                }
            }
        }

        if rules.is_empty() {
            return;
        }

        rules.sort_by_key(|rule| rule.priority);
        if rules.len() > 4 {
            rules.truncate(4);
        }

        self.texture_rules = rules;
        self.chunk_texture_bindings.clear();
        self.chunk_meshes.clear();
        self.road_meshes.clear();
        self.active_chunk_texture_ids = None;
    }

    fn derive_rule_for_texture(texture_id: TextureId, name: &str) -> TextureRule {
        Self::base_rule_for(texture_id, None, name)
    }

    fn base_rule_for(
        texture_id: TextureId,
        surface: Option<&TerrainSurface>,
        name: &str,
    ) -> TextureRule {
        let profile = if let Some(surface) = surface {
            match surface {
                TerrainSurface::Sand => (-500.0, 120.0, 0.0, 0.55, 5),
                TerrainSurface::Rock | TerrainSurface::Metal => {
                    (-500.0, 500.0, 0.6, std::f32::consts::FRAC_PI_2, 25)
                }
                TerrainSurface::Snow => (150.0, 500.0, 0.0, std::f32::consts::FRAC_PI_2, 30),
                TerrainSurface::Water => (-50.0, 20.0, 0.0, 0.4, 2),
                TerrainSurface::Dirt | TerrainSurface::Pavement | TerrainSurface::Concrete => {
                    (-500.0, 220.0, 0.0, 0.75, 12)
                }
                TerrainSurface::Wood => (-500.0, 220.0, 0.0, 0.7, 12),
                TerrainSurface::Grass => (-500.0, 200.0, 0.0, 0.7, 10),
                TerrainSurface::Custom(_) => Self::heuristic_profile(name),
            }
        } else {
            Self::heuristic_profile(name)
        };

        let (preferred_gradient, gradient_tolerance) = Self::gradient_profile(surface, name);

        TextureRule {
            texture_id,
            height_min: profile.0,
            height_max: profile.1,
            slope_min: profile.2,
            slope_max: profile.3,
            priority: profile.4,
            preferred_gradient,
            gradient_tolerance,
        }
    }

    fn heuristic_profile(name: &str) -> (f32, f32, f32, f32, u8) {
        let lower = name.to_lowercase();
        if lower.contains("sand") || lower.contains("desert") {
            (-500.0, 120.0, 0.0, 0.55, 5)
        } else if lower.contains("cliff") || lower.contains("rock") {
            (-500.0, 500.0, 0.6, std::f32::consts::FRAC_PI_2, 20)
        } else if lower.contains("snow") || lower.contains("ice") {
            (150.0, 500.0, 0.0, std::f32::consts::FRAC_PI_2, 30)
        } else if lower.contains("water") || lower.contains("sea") {
            (-50.0, 20.0, 0.0, 0.4, 2)
        } else if lower.contains("mud") || lower.contains("dirt") {
            (-500.0, 180.0, 0.0, 0.75, 15)
        } else {
            (-500.0, 200.0, 0.0, 0.7, 10)
        }
    }

    fn gradient_profile(surface: Option<&TerrainSurface>, name: &str) -> (f32, f32) {
        let from_surface = surface.and_then(|surface| match surface {
            TerrainSurface::Sand => Some((0.15, 0.3)),
            TerrainSurface::Rock | TerrainSurface::Metal => Some((0.8, 0.25)),
            TerrainSurface::Snow => Some((0.35, 0.35)),
            TerrainSurface::Water => Some((0.05, 0.2)),
            TerrainSurface::Dirt => Some((0.2, 0.35)),
            TerrainSurface::Pavement | TerrainSurface::Concrete | TerrainSurface::Wood => {
                Some((0.1, 0.3))
            }
            TerrainSurface::Grass => Some((0.18, 0.35)),
            TerrainSurface::Custom(_) => None,
        });

        if let Some(profile) = from_surface {
            return profile;
        }

        let lower = name.to_lowercase();
        if lower.contains("cliff") || lower.contains("rock") || lower.contains("ridge") {
            (0.8, 0.25)
        } else if lower.contains("sand") || lower.contains("dune") {
            (0.12, 0.3)
        } else if lower.contains("snow") || lower.contains("ice") {
            (0.35, 0.35)
        } else if lower.contains("water") || lower.contains("sea") || lower.contains("river") {
            (0.05, 0.2)
        } else if lower.contains("mud") || lower.contains("dirt") || lower.contains("soil") {
            (0.2, 0.35)
        } else if lower.contains("asphalt") || lower.contains("road") || lower.contains("pave") {
            (0.1, 0.3)
        } else {
            (-1.0, 0.4)
        }
    }

    fn find_terrain_type_for_texture(&self, texture: &TerrainTexture) -> Option<TerrainType> {
        let registry = ini_terrain::get_terrain_types()?;
        let guard = registry.read();

        let mut candidates: Vec<AsciiString> = Vec::new();
        if !texture.name.is_empty() {
            candidates.push(AsciiString::from(texture.name.as_str()));
        }

        if let Some(file_name) = Path::new(&texture.diffuse_path)
            .file_name()
            .and_then(|n| n.to_str())
        {
            candidates.push(AsciiString::from(file_name));
            if let Some(stem) = Path::new(file_name).file_stem().and_then(|n| n.to_str()) {
                candidates.push(AsciiString::from(stem));
            }
        }

        for candidate in &candidates {
            if let Some(terrain) = guard.find_terrain(candidate) {
                return Some(terrain.clone());
            }
        }

        if let Some(file_name) = Path::new(&texture.diffuse_path)
            .file_name()
            .and_then(|n| n.to_str())
        {
            let file_lower = file_name.to_ascii_lowercase();
            for terrain_name in guard.get_terrain_names() {
                let key = AsciiString::from(terrain_name.as_str());
                if let Some(terrain) = guard.find_terrain(&key) {
                    let texture_path = terrain.texture_name.as_str();
                    if !texture_path.is_empty() {
                        let terrain_file = Path::new(texture_path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(texture_path)
                            .to_ascii_lowercase();
                        if terrain_file == file_lower {
                            return Some(terrain.clone());
                        }
                    }
                }
            }
        }

        None
    }

    fn rule_from_terrain_type(texture_id: TextureId, terrain: &TerrainType) -> TextureRule {
        let mut rule = Self::base_rule_for(
            texture_id,
            Some(&terrain.surface_type),
            terrain.name.as_str(),
        );

        if let Some(value) =
            Self::parse_f32_property(&terrain.properties, &["HeightMin", "MinHeight"])
        {
            rule.height_min = value;
        }
        if let Some(value) =
            Self::parse_f32_property(&terrain.properties, &["HeightMax", "MaxHeight"])
        {
            rule.height_max = value;
        }
        if let Some(value) = Self::parse_f32_property(
            &terrain.properties,
            &["SlopeMin", "MinSlope", "SlopeMinDegrees"],
        ) {
            rule.slope_min = Self::normalize_slope(value);
        }
        if let Some(value) = Self::parse_f32_property(
            &terrain.properties,
            &["SlopeMax", "MaxSlope", "SlopeMaxDegrees"],
        ) {
            rule.slope_max = Self::normalize_slope(value);
        }
        if let Some(value) = Self::parse_f32_property(
            &terrain.properties,
            &["PreferredGradient", "GradientPreference"],
        ) {
            rule.preferred_gradient = value.clamp(-1.0, 1.0);
        }
        if let Some(value) =
            Self::parse_f32_property(&terrain.properties, &["GradientTolerance", "GradientRange"])
        {
            rule.gradient_tolerance = value.abs().max(0.05);
        }
        if let Some(priority) =
            Self::parse_u8_property(&terrain.properties, &["Priority", "PriorityWeight"])
        {
            rule.priority = priority;
        }

        rule
    }

    fn parse_f32_property(map: &HashMap<String, String>, keys: &[&str]) -> Option<f32> {
        for key in keys {
            if let Some(value) = map.get(*key) {
                if let Ok(parsed) = value.parse::<f32>() {
                    return Some(parsed);
                }
            }
        }
        None
    }

    fn parse_u8_property(map: &HashMap<String, String>, keys: &[&str]) -> Option<u8> {
        for key in keys {
            if let Some(value) = map.get(*key) {
                if let Ok(parsed) = value.parse::<i32>() {
                    return Some(parsed.clamp(0, 255) as u8);
                }
            }
        }
        None
    }

    fn normalize_slope(value: f32) -> f32 {
        if value > std::f32::consts::PI {
            value.to_radians()
        } else {
            value
        }
    }

    fn normalize_terrain_texture_path(path: &str) -> Option<String> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return None;
        }

        let normalized = trimmed
            .replace('\\', "/")
            .chars()
            .filter(|c| *c != ' ')
            .collect::<String>();
        if normalized.contains('/') {
            Some(normalized)
        } else {
            Some(format!("{TERRAIN_TGA_DIR_PATH}{normalized}"))
        }
    }

    fn prepare_chunk_texture_binding(
        &mut self,
        chunk_id: ChunkId,
        texture_ids: &[TextureId],
        slot_map: &HashMap<TextureId, usize>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> TerrainResult<()> {
        let started = std::time::Instant::now();
        let layout = match &self.terrain_texture_bind_group_layout {
            Some(layout) => Arc::clone(layout),
            None => return Ok(()),
        };

        let sampler_mode = TerrainSamplerMode::current();
        let sampler_changed = self.terrain_sampler_mode != Some(sampler_mode);
        if self.terrain_sampler.is_none() || sampler_changed {
            self.terrain_sampler = Some(device.create_sampler(&sampler_mode.to_descriptor()));
            self.terrain_sampler_mode = Some(sampler_mode);
            if sampler_changed {
                self.chunk_texture_bindings.clear();
            }
        }

        let sampler = self
            .terrain_sampler
            .as_ref()
            .expect("terrain sampler should be initialised");

        let mut final_ids = [0; MAX_TEXTURES_PER_CHUNK];
        let fallback_texture_id = self.texture_system.first_texture_id();

        for idx in 0..MAX_TEXTURES_PER_CHUNK {
            let mut texture_id = *texture_ids.get(idx).unwrap_or(&0);
            if texture_id == 0 || self.texture_system.get_texture(texture_id).is_none() {
                texture_id = fallback_texture_id.unwrap_or(0);
            }
            final_ids[idx] = texture_id;
        }

        if let Some(existing) = self
            .chunk_texture_bindings
            .values()
            .find(|binding| binding.texture_ids == final_ids && binding.slot_map == *slot_map)
        {
            self.chunk_texture_bindings.insert(
                chunk_id,
                ChunkTextureBinding {
                    bind_group: existing.bind_group.clone(),
                    slot_map: existing.slot_map.clone(),
                    texture_ids: existing.texture_ids,
                    diffuse_views: existing.diffuse_views.clone(),
                },
            );
            let elapsed = started.elapsed();
            if elapsed >= std::time::Duration::from_millis(50) {
                warn!(
                    "Terrain chunk texture binding reuse slow: chunk={} elapsed={:?}",
                    chunk_id, elapsed
                );
            }
            return Ok(());
        }

        for texture_id in final_ids.iter() {
            if *texture_id == 0 {
                continue;
            }
            if let Err(err) = self.texture_system.load_texture(*texture_id) {
                warn!("Failed to load terrain texture {}: {}", texture_id, err);
            }
        }

        let mut diffuse_views = Vec::with_capacity(MAX_TEXTURES_PER_CHUNK);

        for (binding, texture_id) in final_ids.iter().enumerate() {
            let diffuse_fallback = DEFAULT_TERRAIN_COLORS[binding % DEFAULT_TERRAIN_COLORS.len()];
            let diffuse_view = if *texture_id == 0 {
                self.texture_system.acquire_texture_view(
                    0,
                    TextureKind::Diffuse,
                    device,
                    queue,
                    diffuse_fallback,
                )?
            } else {
                self.texture_system.acquire_texture_view(
                    *texture_id,
                    TextureKind::Diffuse,
                    device,
                    queue,
                    diffuse_fallback,
                )?
            };
            diffuse_views.push(diffuse_view);
        }

        let mut entries: Vec<wgpu::BindGroupEntry> = Vec::with_capacity(MAX_TEXTURES_PER_CHUNK + 1);

        for binding in 0..MAX_TEXTURES_PER_CHUNK {
            entries.push(wgpu::BindGroupEntry {
                binding: binding as u32,
                resource: wgpu::BindingResource::TextureView(diffuse_views[binding].as_ref()),
            });
        }
        entries.push(wgpu::BindGroupEntry {
            binding: MAX_TEXTURES_PER_CHUNK as u32,
            resource: wgpu::BindingResource::Sampler(sampler),
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("Terrain Chunk {} Texture Bind Group", chunk_id)),
            layout: layout.as_ref(),
            entries: &entries,
        });

        self.chunk_texture_bindings.insert(
            chunk_id,
            ChunkTextureBinding {
                bind_group,
                slot_map: slot_map.clone(),
                texture_ids: final_ids,
                diffuse_views,
            },
        );

        let elapsed = started.elapsed();
        if elapsed >= std::time::Duration::from_millis(50) {
            let texture_paths: Vec<String> = final_ids
                .iter()
                .map(|texture_id| {
                    self.texture_system
                        .get_texture(*texture_id)
                        .map(|texture| texture.diffuse_path.clone())
                        .unwrap_or_else(|| format!("<unknown:{}>", texture_id))
                })
                .collect();
            warn!(
                "Terrain chunk texture binding create slow: chunk={} elapsed={:?} textures={:?} paths={:?}",
                chunk_id, elapsed, final_ids, texture_paths
            );
        }

        Ok(())
    }

    /// Initialize WGPU resources
    pub fn init_gpu_resources(
        &mut self,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
    ) -> TerrainResult<()> {
        self.device = Some(Arc::clone(&device));
        self.queue = Some(Arc::clone(&queue));

        // Create terrain render pipeline
        self.create_terrain_pipeline(device.as_ref())?;

        // Create skybox background pipeline before terrain/water draws.
        self.create_skybox_background_pipeline(device.as_ref())?;

        // Create water render pipeline
        self.create_water_pipeline(device.as_ref())?;

        // Create road render pipeline
        self.create_road_pipeline(device.as_ref())?;

        self.sync_global_water_plane(device.as_ref())?;

        // Create uniform buffer
        self.uniform_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Terrain Uniform Buffer"),
            size: std::mem::size_of::<TerrainUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));

        if let (Some(layout), Some(buffer)) =
            (&self.terrain_camera_bind_group_layout, &self.uniform_buffer)
        {
            self.terrain_camera_bind_group =
                Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Terrain Camera Bind Group"),
                    layout: layout.as_ref(),
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    }],
                }));
        }

        Ok(())
    }

    fn update_chunk_meshes(&mut self) -> TerrainResult<()> {
        let started = std::time::Instant::now();
        let device = match &self.device {
            Some(device) => Arc::clone(device),
            None => return Ok(()),
        };

        let queue = match &self.queue {
            Some(queue) => Arc::clone(queue),
            None => return Ok(()),
        };

        let visible_chunk_ids = self.visible_chunk_ids_for_draw_area();
        let refresh_texture_slots = self.active_chunk_texture_ids.is_none();

        let select_slots_started = std::time::Instant::now();
        let stable_texture_ids = if refresh_texture_slots {
            let ids = self.select_stable_chunk_texture_ids(&visible_chunk_ids);
            self.active_chunk_texture_ids = Some(ids);
            ids
        } else {
            self.active_chunk_texture_ids
                .unwrap_or([0; MAX_TEXTURES_PER_CHUNK])
        };
        let select_slots_elapsed = select_slots_started.elapsed();

        let texture_slots_changed = refresh_texture_slots;

        let mut shared_slot_map: HashMap<TextureId, usize> = HashMap::new();
        for (slot, texture_id) in stable_texture_ids.iter().enumerate() {
            shared_slot_map.entry(*texture_id).or_insert(slot);
        }

        let mut binding_updates = 0usize;
        let mut mesh_uploads = 0usize;
        let mut vertices_uploaded = 0usize;
        let mut indices_uploaded = 0usize;
        let mut binding_prep_elapsed = std::time::Duration::ZERO;
        let mut mesh_upload_elapsed = std::time::Duration::ZERO;
        for &chunk_id in &visible_chunk_ids {
            let (chunk_revision, has_chunk_geometry) = match self.chunk_manager.get_chunk(chunk_id)
            {
                Some(chunk) => (
                    chunk.geometry_revision,
                    !(chunk.vertices.is_empty() || chunk.indices.is_empty()),
                ),
                None => continue,
            };

            let binding_up_to_date = self
                .chunk_texture_bindings
                .get(&chunk_id)
                .map(|binding| binding.texture_ids == stable_texture_ids)
                .unwrap_or(false);
            if !binding_up_to_date {
                let binding_started = std::time::Instant::now();
                self.prepare_chunk_texture_binding(
                    chunk_id,
                    &stable_texture_ids,
                    &shared_slot_map,
                    device.as_ref(),
                    queue.as_ref(),
                )?;
                binding_prep_elapsed += binding_started.elapsed();
                binding_updates += 1;
            }

            let needs_mesh_upload = match self.chunk_meshes.get(&chunk_id) {
                Some(mesh) => mesh.revision != chunk_revision || texture_slots_changed,
                None => true,
            };

            if needs_mesh_upload {
                if !has_chunk_geometry {
                    continue;
                }
                let upload_started = std::time::Instant::now();

                let (chunk_vertices, chunk_indices) = match self.chunk_manager.get_chunk(chunk_id) {
                    Some(chunk) => (chunk.vertices.clone(), chunk.indices.clone()),
                    None => continue,
                };

                let mut gpu_vertices = chunk_vertices;
                let mut vertex_weights = Vec::with_capacity(gpu_vertices.len());

                for vertex in &gpu_vertices {
                    let position =
                        Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
                    let normal_vec =
                        Vec3::new(vertex.normal[0], vertex.normal[1], vertex.normal[2]);
                    let mut normal = if normal_vec.length_squared() > f32::EPSILON {
                        normal_vec.normalize()
                    } else {
                        Vec3::Y
                    };
                    if !normal.y.is_finite() {
                        normal = Vec3::Y;
                    }

                    let height = position.y;
                    let slope = normal.dot(Vec3::Y).clamp(-1.0, 1.0).acos();

                    let base_weights = self.texture_system.generate_texture_weights(
                        height,
                        slope,
                        vertex.tex_coords,
                        &self.texture_rules,
                    );
                    let blended = self.texture_system.blend_textures_at_position(
                        position,
                        height,
                        normal,
                        vertex.tex_coords,
                        &base_weights,
                        &self.texture_rules,
                    );

                    vertex_weights.push(blended);
                }

                for (vertex, blended) in gpu_vertices.iter_mut().zip(vertex_weights.iter()) {
                    let mut packed_indices = [0u16; MAX_BLEND_WEIGHTS];
                    let mut packed_weights = [0.0f32; MAX_BLEND_WEIGHTS];

                    let mut insert_count = 0usize;
                    for (texture_id, weight) in blended.iter_pairs() {
                        if weight <= 0.0 {
                            break;
                        }

                        let slot_idx = shared_slot_map
                            .get(&texture_id)
                            .copied()
                            .or_else(|| {
                                shared_slot_map
                                    .get(stable_texture_ids.first().unwrap())
                                    .copied()
                            })
                            .unwrap_or(0);

                        if insert_count < MAX_BLEND_WEIGHTS {
                            packed_indices[insert_count] = slot_idx as u16;
                            packed_weights[insert_count] = weight;
                            insert_count += 1;
                        } else {
                            let mut weakest = 0usize;
                            let mut weakest_weight = packed_weights[0];
                            for idx in 1..MAX_BLEND_WEIGHTS {
                                if packed_weights[idx] < weakest_weight {
                                    weakest_weight = packed_weights[idx];
                                    weakest = idx;
                                }
                            }

                            if weight > weakest_weight {
                                packed_indices[weakest] = slot_idx as u16;
                                packed_weights[weakest] = weight;
                            }
                        }
                    }

                    let sum: f32 = packed_weights.iter().sum();
                    if sum > f32::EPSILON {
                        for weight in &mut packed_weights {
                            *weight /= sum;
                        }
                    } else {
                        packed_weights[0] = 1.0;
                        packed_indices[0] = shared_slot_map
                            .get(stable_texture_ids.first().unwrap())
                            .copied()
                            .unwrap_or(0) as u16;
                    }

                    vertex.blend_indices = packed_indices;
                    vertex.blend_weights = packed_weights;
                }

                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Terrain Chunk {} Vertex Buffer", chunk_id)),
                    contents: cast_slice(&gpu_vertices),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Terrain Chunk {} Index Buffer", chunk_id)),
                    contents: cast_slice(&chunk_indices),
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                });

                self.chunk_meshes.insert(
                    chunk_id,
                    GpuChunkMesh {
                        vertex_buffer,
                        index_buffer,
                        index_count: chunk_indices.len() as u32,
                        revision: chunk_revision,
                    },
                );
                mesh_upload_elapsed += upload_started.elapsed();
                mesh_uploads += 1;
                vertices_uploaded += gpu_vertices.len();
                indices_uploaded += chunk_indices.len();
            }
        }

        self.chunk_meshes
            .retain(|id, _| self.chunk_manager.has_chunk(*id));

        self.chunk_texture_bindings
            .retain(|id, _| self.chunk_manager.has_chunk(*id));

        self.stats.rendered_chunks = visible_chunk_ids.len();
        self.stats.triangles_rendered = visible_chunk_ids
            .iter()
            .filter_map(|chunk_id| self.chunk_manager.get_chunk(*chunk_id))
            .map(|chunk| chunk.stats.triangle_count as usize)
            .sum();

        let elapsed = started.elapsed();
        if elapsed >= std::time::Duration::from_millis(200) {
            warn!(
                "TerrainVisual::update_chunk_meshes breakdown: total={:?} visible={} refresh_texture_slots={} select_slots={:?} binding_updates={} binding_prep={:?} mesh_uploads={} uploaded_vertices={} uploaded_indices={} mesh_upload={:?} pending_visible={}",
                elapsed,
                visible_chunk_ids.len(),
                refresh_texture_slots,
                select_slots_elapsed,
                binding_updates,
                binding_prep_elapsed,
                mesh_uploads,
                vertices_uploaded,
                indices_uploaded,
                mesh_upload_elapsed,
                self.chunk_manager.pending_visible_chunk_count()
            );
        }

        Ok(())
    }

    fn select_stable_chunk_texture_ids(
        &mut self,
        _visible_chunk_ids: &[ChunkId],
    ) -> [TextureId; MAX_TEXTURES_PER_CHUNK] {
        let mut selected_textures: Vec<TextureId> = Vec::new();
        for rule in &self.texture_rules {
            let texture_id = rule.texture_id;
            if texture_id == 0 || self.texture_system.get_texture(texture_id).is_none() {
                continue;
            }
            if selected_textures
                .iter()
                .all(|candidate| *candidate != texture_id)
            {
                selected_textures.push(texture_id);
                if selected_textures.len() == MAX_TEXTURES_PER_CHUNK {
                    break;
                }
            }
        }

        if selected_textures.is_empty() {
            if let Some(rule) = self.texture_rules.first() {
                selected_textures.push(rule.texture_id);
            } else if let Some(texture_id) = self.texture_system.first_texture_id() {
                selected_textures.push(texture_id);
            }
        }

        if selected_textures.is_empty() {
            return [0; MAX_TEXTURES_PER_CHUNK];
        }

        while selected_textures.len() < MAX_TEXTURES_PER_CHUNK {
            if let Some(&fallback) = selected_textures.first() {
                selected_textures.push(fallback);
            } else {
                break;
            }
        }

        let mut stable_texture_ids = [0; MAX_TEXTURES_PER_CHUNK];
        for (idx, texture_id) in selected_textures
            .iter()
            .enumerate()
            .take(MAX_TEXTURES_PER_CHUNK)
        {
            stable_texture_ids[idx] = *texture_id;
        }
        stable_texture_ids
    }

    fn update_road_meshes(&mut self) -> TerrainResult<()> {
        let Some(device) = self.device.as_ref().cloned() else {
            self.road_meshes.clear();
            return Ok(());
        };

        let mut meshes = Vec::new();
        self.road_system
            .for_each_visible_surface_geometry(|segment_width, vertices, indices| {
                if vertices.is_empty() || indices.is_empty() {
                    return;
                }

                let road_alpha = if segment_width > 0.0 { 1.0 } else { 0.0 };
                let gpu_vertices: Vec<RoadVertex> = vertices
                    .iter()
                    .map(|vertex| RoadVertex {
                        position: vertex.position,
                        normal: vertex.normal,
                        tex_coords: vertex.tex_coords,
                        road_width: road_alpha,
                    })
                    .collect();

                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Road Mesh Vertex Buffer"),
                    contents: cast_slice(&gpu_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Road Mesh Index Buffer"),
                    contents: cast_slice(indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

                meshes.push(GpuRoadMesh {
                    vertex_buffer,
                    index_buffer,
                    index_count: indices.len() as u32,
                });
            });

        self.road_meshes = meshes;
        Ok(())
    }

    pub fn record_chunk_draws<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        if !self.enabled {
            return;
        }

        self.record_skybox_background_draw(pass);

        if let Some(pipeline) = &self.terrain_pipeline {
            pass.set_pipeline(pipeline);
            if let Some(camera_bg) = &self.terrain_camera_bind_group {
                pass.set_bind_group(0, camera_bg, &[]);
            }

            let chunk_meshes = &self.chunk_meshes;
            let chunk_texture_bindings = &self.chunk_texture_bindings;
            let visible_chunk_ids = self.visible_chunk_ids_for_draw_area();

            let _ = self.chunk_manager.render_pass_draw(
                pass,
                |chunk_id| {
                    chunk_texture_bindings
                        .get(&chunk_id)
                        .map(|binding| binding.bind_group.clone())
                },
                |chunk_id| {
                    let mesh = chunk_meshes.get(&chunk_id)?;
                    if !visible_chunk_ids.contains(&chunk_id) {
                        return None;
                    }
                    Some((
                        mesh.vertex_buffer.slice(..),
                        mesh.index_buffer.slice(..),
                        mesh.index_count,
                    ))
                },
            );
        }

        self.record_road_draws(pass);
        self.record_water_draws(pass);
    }

    pub fn record_chunk_depth_draws<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        if !self.enabled {
            return;
        }

        if let Some(pipeline) = &self.terrain_depth_pipeline {
            pass.set_pipeline(pipeline);
            if let Some(camera_bg) = &self.terrain_camera_bind_group {
                pass.set_bind_group(0, camera_bg, &[]);
            }

            let chunk_meshes = &self.chunk_meshes;
            let visible_chunk_ids = self.visible_chunk_ids_for_draw_area();

            let _ = self.chunk_manager.render_pass_draw(
                pass,
                |_| None,
                |chunk_id| {
                    let mesh = chunk_meshes.get(&chunk_id)?;
                    if !visible_chunk_ids.contains(&chunk_id) {
                        return None;
                    }
                    Some((
                        mesh.vertex_buffer.slice(..),
                        mesh.index_buffer.slice(..),
                        mesh.index_count,
                    ))
                },
            );
        }
    }

    fn record_skybox_background_draw<'pass>(&self, pass: &mut RenderPass<'pass>) {
        let (Some(pipeline), Some(bind_group)) = (
            self.skybox_background_pipeline.as_ref(),
            self.skybox_background_bind_group.as_ref(),
        ) else {
            return;
        };

        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    fn record_water_draws<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        let (Some(water_plane), Some(water_pipeline), Some(camera_bg)) = (
            self.water_plane.as_ref(),
            self.water_pipeline.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
        ) else {
            return;
        };

        pass.set_pipeline(water_pipeline);
        pass.set_bind_group(0, camera_bg, &[]);

        let vertex_buffer = &water_plane.vertex_buffer;
        let index_buffer = &water_plane.index_buffer;
        let index_count = water_plane.index_count;

        let _ = self.water_system.render_pass_draw(pass, || {
            Some((vertex_buffer.slice(..), index_buffer.slice(..), index_count))
        });
    }

    fn record_road_draws<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        let (Some(road_pipeline), Some(camera_bg)) = (
            self.road_pipeline.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
        ) else {
            return;
        };
        if self.road_meshes.is_empty() {
            return;
        }

        pass.set_pipeline(road_pipeline);
        pass.set_bind_group(0, camera_bg, &[]);

        let road_meshes = &self.road_meshes;
        let mut mesh_index = 0;
        let _ = self.road_system.render_pass_draw(pass, || {
            let mesh = road_meshes.get(mesh_index)?;
            mesh_index += 1;
            Some((
                mesh.vertex_buffer.slice(..),
                mesh.index_buffer.slice(..),
                mesh.index_count,
            ))
        });
    }

    fn sync_global_water_plane(&mut self, device: &wgpu::Device) -> TerrainResult<()> {
        let Some(global_data) = get_global_data() else {
            self.water_plane = None;
            return Ok(());
        };
        let global = global_data.read();

        if !self.config.water_enabled
            || global.water_extent_x <= 0.0
            || global.water_extent_y <= 0.0
        {
            self.water_plane = None;
            return Ok(());
        }

        let water_z = global.water_position_z;
        let half_extent_x = global.water_extent_x * 0.5;
        let half_extent_y = global.water_extent_y * 0.5;
        let min_x = global.water_position_x - half_extent_x;
        let min_y = global.water_position_y - half_extent_y;
        let max_x = global.water_position_x + half_extent_x;
        let max_y = global.water_position_y + half_extent_y;

        let vertices = [
            WaterVertex {
                position: [min_x, water_z, min_y],
                normal: [0.0, 1.0, 0.0],
                tex_coords: [0.0, 0.0],
                flow_direction: [0.0, 0.0],
            },
            WaterVertex {
                position: [max_x, water_z, min_y],
                normal: [0.0, 1.0, 0.0],
                tex_coords: [1.0, 0.0],
                flow_direction: [0.0, 0.0],
            },
            WaterVertex {
                position: [max_x, water_z, max_y],
                normal: [0.0, 1.0, 0.0],
                tex_coords: [1.0, 1.0],
                flow_direction: [0.0, 0.0],
            },
            WaterVertex {
                position: [min_x, water_z, max_y],
                normal: [0.0, 1.0, 0.0],
                tex_coords: [0.0, 1.0],
                flow_direction: [0.0, 0.0],
            },
        ];
        let indices: [u32; 6] = [0, 1, 2, 0, 2, 3];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Global Water Plane Vertex Buffer"),
            contents: cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Global Water Plane Index Buffer"),
            contents: cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        self.water_plane = Some(GpuWaterPlane {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        });

        Ok(())
    }

    /// Create terrain render pipeline
    fn create_terrain_pipeline(&mut self, device: &wgpu::Device) -> TerrainResult<()> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Terrain Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/terrain.wgsl").into()),
        });

        let camera_layout = Arc::new(device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("Terrain Camera Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            },
        ));

        let mut texture_entries: Vec<wgpu::BindGroupLayoutEntry> = (0..MAX_TEXTURES_PER_CHUNK)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding: binding as u32,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            })
            .collect();

        texture_entries.push(wgpu::BindGroupLayoutEntry {
            binding: MAX_TEXTURES_PER_CHUNK as u32,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });

        let texture_layout = Arc::new(device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("Terrain Texture Bind Group Layout"),
                entries: &texture_entries,
            },
        ));

        self.terrain_camera_bind_group_layout = Some(Arc::clone(&camera_layout));
        self.terrain_texture_bind_group_layout = Some(Arc::clone(&texture_layout));

        let bind_group_layouts = [camera_layout.as_ref(), texture_layout.as_ref()];
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Terrain Pipeline Layout"),
            bind_group_layouts: &bind_group_layouts,
            push_constant_ranges: &[],
        });

        self.terrain_pipeline = Some(device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("Terrain Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[TerrainVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8UnormSrgb,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            },
        ));

        let depth_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Terrain Depth Pipeline Layout"),
                bind_group_layouts: &[camera_layout.as_ref()],
                push_constant_ranges: &[],
            });

        self.terrain_depth_pipeline = Some(device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("Terrain Depth Pipeline"),
                layout: Some(&depth_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[TerrainVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            },
        ));

        Ok(())
    }

    fn create_skybox_background_pipeline(&mut self, device: &wgpu::Device) -> TerrainResult<()> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Terrain Skybox Background Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/skybox_background.wgsl").into()),
        });

        let bind_group_layout = Arc::new(device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("Terrain Skybox Background Bind Group Layout"),
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
            },
        ));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Terrain Skybox Background Pipeline Layout"),
            bind_group_layouts: &[bind_group_layout.as_ref()],
            push_constant_ranges: &[],
        });

        self.skybox_background_pipeline = Some(device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("Terrain Skybox Background Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8UnormSrgb,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
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
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Always,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            },
        ));

        self.skybox_background_bind_group_layout = Some(bind_group_layout);
        Ok(())
    }

    /// Create water render pipeline
    fn create_water_pipeline(&mut self, device: &wgpu::Device) -> TerrainResult<()> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Water Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/water.wgsl").into()),
        });

        let Some(camera_layout) = self.terrain_camera_bind_group_layout.as_ref() else {
            return Err(TerrainError::GPUError(
                "terrain camera bind group layout must exist before water pipeline creation"
                    .to_string(),
            ));
        };

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Water Pipeline Layout"),
            bind_group_layouts: &[camera_layout.as_ref()],
            push_constant_ranges: &[],
        });

        self.water_pipeline = Some(device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("Water Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[WaterVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8UnormSrgb,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None, // Water should be visible from both sides
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: false, // Water shouldn't write to depth buffer
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            },
        ));

        Ok(())
    }

    /// Create road render pipeline
    fn create_road_pipeline(&mut self, device: &wgpu::Device) -> TerrainResult<()> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Road Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/road.wgsl").into()),
        });

        let Some(camera_layout) = self.terrain_camera_bind_group_layout.as_ref() else {
            return Err(TerrainError::InitializationError(
                "Terrain camera bind group layout must exist before road pipeline creation"
                    .to_string(),
            ));
        };

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Road Pipeline Layout"),
            bind_group_layouts: &[camera_layout.as_ref()],
            push_constant_ranges: &[],
        });

        self.road_pipeline = Some(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Road Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[RoadVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8UnormSrgb,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual, // Roads should render on top of terrain
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            }),
        );

        Ok(())
    }

    /// Load terrain heightmap from file
    pub fn load_heightmap(&mut self, path: &str) -> TerrainResult<()> {
        self.load_heightmap_with_world_size(path, None)
    }

    /// Load terrain heightmap from runtime map data (C++ parity fallback when no external hint exists).
    pub fn load_heightmap_from_data(
        &mut self,
        mut heightmap: HeightMap,
        source_hint: Option<&Path>,
        world_size: Option<(f32, f32)>,
    ) -> TerrainResult<()> {
        if heightmap.width == 0 || heightmap.height == 0 {
            return Err(TerrainError::HeightmapError(
                "Runtime heightmap has invalid dimensions".to_string(),
            ));
        }
        if heightmap.heights.len()
            != (heightmap.width as usize).saturating_mul(heightmap.height as usize)
        {
            return Err(TerrainError::HeightmapError(
                "Runtime heightmap sample count does not match dimensions".to_string(),
            ));
        }

        self.filename = source_hint
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<runtime_heightmap>".to_string());
        self.ensure_terrain_definitions(source_hint)?;
        if self.texture_rules.is_empty() {
            self.ensure_default_textures();
        }

        if let Some((world_width, world_height)) = world_size {
            let sample_width = heightmap.width.saturating_sub(1).max(1) as f32;
            let sample_height = heightmap.height.saturating_sub(1).max(1) as f32;
            let scale_x = world_width / sample_width;
            let scale_z = world_height / sample_height;
            heightmap.scale = ((scale_x + scale_z) * 0.5).max(f32::EPSILON);
        }

        self.config.heightmap_resolution = (heightmap.width, heightmap.height);
        self.config.world_size = world_size.unwrap_or((
            heightmap.width as f32 * heightmap.scale,
            heightmap.height as f32 * heightmap.scale,
        ));
        self.chunk_manager.set_config(self.config.clone());
        self.chunk_manager
            .load_heightmap(&heightmap, &self.config)?;
        self.height_map = Some(heightmap);
        self.reset_draw_area_state();
        Ok(())
    }

    pub fn load_heightmap_with_world_size(
        &mut self,
        path: &str,
        world_size: Option<(f32, f32)>,
    ) -> TerrainResult<()> {
        log::info!("Loading terrain heightmap: {}", path);
        self.filename = path.to_string();
        self.ensure_terrain_definitions(Some(Path::new(path)))?;
        if self.texture_rules.is_empty() {
            self.ensure_default_textures();
        }

        // Load heightmap using the appropriate loader based on file extension
        let mut heightmap = if path.ends_with(".hmp") {
            HeightMap::load_hmp(path)?
        } else if path.ends_with(".tga") {
            HeightMap::load_tga(path)?
        } else if path.ends_with(".raw") {
            HeightMap::load_raw(path)?
        } else {
            return Err(TerrainError::HeightmapError(format!(
                "Unsupported heightmap format: {}",
                path
            )));
        };

        if let Some((world_width, world_height)) = world_size {
            let sample_width = heightmap.width.saturating_sub(1).max(1) as f32;
            let sample_height = heightmap.height.saturating_sub(1).max(1) as f32;
            let scale_x = world_width / sample_width;
            let scale_z = world_height / sample_height;
            heightmap.scale = ((scale_x + scale_z) * 0.5).max(f32::EPSILON);
        }

        // Update terrain configuration based on heightmap
        self.config.heightmap_resolution = (heightmap.width, heightmap.height);
        self.config.world_size = world_size.unwrap_or((
            heightmap.width as f32 * heightmap.scale,
            heightmap.height as f32 * heightmap.scale,
        ));

        self.chunk_manager.set_config(self.config.clone());

        // Initialize chunk system with heightmap data
        self.chunk_manager
            .load_heightmap(&heightmap, &self.config)?;

        self.height_map = Some(heightmap);
        self.reset_draw_area_state();

        log::info!("Terrain heightmap loaded successfully");
        Ok(())
    }

    /// Load terrain textures
    pub fn load_textures(&mut self, texture_paths: &[&str]) -> TerrainResult<()> {
        self.ensure_terrain_definitions(None)?;
        let normalized_paths: Vec<String> = texture_paths
            .iter()
            .filter_map(|path| Self::normalize_terrain_texture_path(path))
            .collect();
        let normalized_refs: Vec<&str> = normalized_paths.iter().map(String::as_str).collect();
        let ids = self.texture_system.load_textures(&normalized_refs)?;

        if ids.is_empty() {
            if self.texture_rules.is_empty() {
                self.ensure_default_textures();
            }
            return Ok(());
        }

        self.build_rules_from_textures(&ids);
        Ok(())
    }

    /// Update seismic simulations
    pub fn update_seismic_simulations(&mut self) {
        let mut active_simulations = Vec::new();
        let simulations = std::mem::take(&mut self.seismic_simulations);
        for mut simulation in simulations {
            simulation.life += 1;

            // Apply seismic effects to terrain
            if let Some(heightmap) = self.height_map.as_mut() {
                let chunk_manager = &mut self.chunk_manager;
                Self::apply_seismic_effect(chunk_manager, &simulation, heightmap);
            }

            // Keep simulation if it's still active
            if simulation.life < 15 {
                active_simulations.push(simulation);
            }
        }

        self.seismic_simulations = active_simulations;
    }

    /// Apply seismic effect to heightmap
    fn apply_seismic_effect(
        chunk_manager: &mut ChunkManager,
        simulation: &SeismicSimulationNode,
        heightmap: &mut HeightMap,
    ) {
        let center_x = simulation.center.x;
        let center_z = simulation.center.z;
        let radius = simulation.radius;
        let magnitude = simulation.magnitude;

        if simulation.life == 0 || simulation.life >= 15 {
            return;
        }

        let effect_magnitude = magnitude / simulation.life as f32;

        // Apply dome-style seismic effect
        for y in 0..heightmap.height as i32 {
            for x in 0..heightmap.width as i32 {
                let world_x = x as f32 * heightmap.scale;
                let world_z = y as f32 * heightmap.scale;

                let dx = world_x - center_x;
                let dz = world_z - center_z;
                let distance = (dx * dx + dz * dz).sqrt();

                if distance < radius {
                    let distance_factor = (1.0 - distance / radius).max(0.0);
                    let height_offset = effect_magnitude
                        * distance_factor
                        * (std::f32::consts::PI * distance / radius / 2.0).cos();

                    // Modify heightmap
                    let index = (y as u32 * heightmap.width + x as u32) as usize;
                    if index < heightmap.heights.len() {
                        heightmap.heights[index] += height_offset;
                        heightmap.heights[index] =
                            heightmap.heights[index].clamp(0.0, heightmap.max_height);
                    }
                }
            }
        }

        // Mark affected chunks as dirty
        chunk_manager.mark_region_dirty(
            simulation.region.0.x,
            simulation.region.0.z,
            simulation.region.1.x,
            simulation.region.1.z,
        );
        chunk_manager.refresh_dirty_chunks(heightmap);
    }

    /// Add seismic simulation
    pub fn add_seismic_simulation(&mut self, simulation: SeismicSimulationNode) {
        self.seismic_simulations.push(simulation);
    }

    /// Get terrain color at position
    pub fn get_terrain_color_at(&self, x: f32, y: f32) -> Result<[f32; 3], TerrainError> {
        // Sample terrain textures at position
        self.texture_system.sample_color_at(x, y)
    }

    /// Get terrain tile type at position
    pub fn get_terrain_tile(&self, x: f32, y: f32) -> Result<u32, TerrainError> {
        // Return terrain type based on texture blending weights
        self.texture_system.get_terrain_type_at(x, y)
    }

    /// Ray-terrain intersection
    pub fn intersect_terrain(&self, ray_start: Vec3, ray_end: Vec3) -> Option<Vec3> {
        if let Some(ref heightmap) = self.height_map {
            heightmap.intersect_ray(ray_start, ray_end)
        } else {
            None
        }
    }

    /// Enable/disable water grid
    pub fn enable_water_grid(&mut self, enable: bool) {
        self.water_grid_enabled = enable;
        self.water_system.set_enabled(enable);
    }

    /// Replace skybox textures
    pub fn replace_skybox_textures(
        &mut self,
        _old_names: &[&str; 5],
        new_names: &[&str; 5],
    ) -> TerrainResult<()> {
        if let Some(device) = self.device.as_ref().cloned() {
            for (i, texture_path) in new_names.iter().enumerate() {
                // Load new skybox texture
                let texture = self.load_texture_from_path(device.as_ref(), texture_path)?;
                self.skybox_textures[i] = Some(texture);
            }
            self.refresh_skybox_background_binding(device.as_ref())?;
        }
        Ok(())
    }

    fn refresh_skybox_background_binding(&mut self, device: &wgpu::Device) -> TerrainResult<()> {
        let Some(layout) = self.skybox_background_bind_group_layout.as_ref() else {
            return Ok(());
        };

        if self.skybox_sampler.is_none() {
            self.skybox_sampler = Some(device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Terrain Skybox Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }));
        }

        let selected_index = [0usize, 4usize, 1usize, 2usize, 3usize]
            .into_iter()
            .find(|idx| self.skybox_textures[*idx].is_some());

        let Some(selected_index) = selected_index else {
            self.skybox_background_view = None;
            self.skybox_background_bind_group = None;
            return Ok(());
        };

        let view = self.skybox_textures[selected_index]
            .as_ref()
            .expect("selected skybox texture must exist")
            .create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self
            .skybox_sampler
            .as_ref()
            .expect("skybox sampler should be initialised");

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Terrain Skybox Background Bind Group"),
            layout: layout.as_ref(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        self.skybox_background_view = Some(view);
        self.skybox_background_bind_group = Some(bind_group);
        Ok(())
    }

    /// Load texture from path
    fn load_texture_from_path(
        &self,
        device: &wgpu::Device,
        path: &str,
    ) -> TerrainResult<wgpu::Texture> {
        let queue = self.queue.as_ref().ok_or_else(|| {
            TerrainError::GPUError("TerrainVisual queue not initialised for texture upload".into())
        })?;

        let dyn_image = self.load_runtime_texture_image(path)?;
        let rgba = dyn_image.to_rgba8();
        let (width, height) = rgba.dimensions();

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Texture: {}", path)),
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

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        Ok(texture)
    }

    fn load_runtime_texture_image(&self, path: &str) -> TerrainResult<image::DynamicImage> {
        for candidate in self.runtime_texture_candidates(path) {
            if let Some(image) = self.try_load_image_from_filesystem(&candidate)? {
                return Ok(image);
            }
        }

        Err(TerrainError::TextureError(GameImageError::LoadError {
            path: path.to_string(),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("runtime texture '{}' not found", path),
            )),
        }))
    }

    fn try_load_image_from_filesystem(
        &self,
        candidate: &Path,
    ) -> TerrainResult<Option<image::DynamicImage>> {
        let resource_name = candidate.to_string_lossy().replace('\\', "/");
        let fs = get_file_system();
        let bytes = {
            let Ok(mut guard) = fs.lock() else {
                return Ok(None);
            };
            let Some(mut file) = guard.open_file(resource_name.as_str(), FileAccess::READ) else {
                return Ok(None);
            };
            match file.read_entire_and_close() {
                Ok(bytes) => bytes,
                Err(err) => {
                    return Err(TerrainError::TextureError(GameImageError::LoadError {
                        path: resource_name,
                        source: Box::new(err),
                    }));
                }
            }
        };

        let extension = candidate
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase());
        let decoded = match extension.as_deref() {
            Some("tga") => image::load_from_memory_with_format(&bytes, ImageFormat::Tga),
            Some("dds") => image::load_from_memory_with_format(&bytes, ImageFormat::Dds),
            Some("png") => image::load_from_memory_with_format(&bytes, ImageFormat::Png),
            Some("jpg") | Some("jpeg") => {
                image::load_from_memory_with_format(&bytes, ImageFormat::Jpeg)
            }
            Some("bmp") => image::load_from_memory_with_format(&bytes, ImageFormat::Bmp),
            _ => image::load_from_memory(&bytes),
        }
        .map_err(|err| {
            TerrainError::TextureError(GameImageError::LoadError {
                path: candidate.display().to_string(),
                source: Box::new(err),
            })
        })?;

        Ok(Some(decoded))
    }

    fn runtime_texture_candidates(&self, path: &str) -> Vec<PathBuf> {
        let normalized = path.replace('\\', "/");
        let bare = normalized.trim_start_matches("./").to_string();
        if bare.is_empty() {
            return Vec::new();
        }

        let mut candidates = Vec::<PathBuf>::new();
        let mut seen: HashSet<PathBuf> = HashSet::new();
        let mut push_unique = |candidate: PathBuf| {
            if seen.insert(candidate.clone()) {
                candidates.push(candidate);
            }
        };

        let extension = Path::new(&bare)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase());
        let is_tga_or_dds = matches!(extension.as_deref(), Some("tga") | Some("dds"));

        let language = get_registry_language().as_str().to_string();
        if !language.is_empty() {
            push_unique(PathBuf::from(format!(
                "Data/{language}/Art/Textures/{bare}"
            )));
        }

        push_unique(PathBuf::from(format!("Art/Textures/{bare}")));

        let global_data = global_data::read();
        let user_data = global_data.get_user_data_dir();
        if !user_data.is_empty() {
            let user_textures = Path::new(&user_data)
                .join(USER_TGA_DIR_PATH.replace("%s", ""))
                .join(&bare);
            push_unique(user_textures);

            if is_tga_or_dds {
                let user_previews = Path::new(&user_data)
                    .join(MAP_PREVIEW_DIR_PATH.replace("%s", ""))
                    .join(&bare);
                push_unique(user_previews);
            }
        }

        candidates
    }
}

impl Default for TerrainVisualImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for TerrainVisualImpl {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Initializing TerrainVisual subsystem");

        self.ensure_default_textures();

        // Initialize subsystems
        self.texture_system.init()?;
        self.water_system.init()?;
        self.road_system.init()?;
        self.chunk_manager.init()?;

        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Resetting TerrainVisual subsystem");

        self.filename.clear();
        self.loaded_terrain_sources.clear();
        self.height_map = None;
        self.reset_draw_area_state();
        self.seismic_simulations.clear();

        // Reset subsystems
        self.texture_system.reset()?;
        self.water_system.reset()?;
        self.road_system.reset()?;
        self.chunk_manager.reset()?;
        self.chunk_meshes.clear();
        self.texture_rules.clear();
        self.chunk_texture_bindings.clear();
        self.active_chunk_texture_ids = None;
        self.terrain_sampler = None;
        self.terrain_sampler_mode = None;
        self.water_plane = None;
        self.road_meshes.clear();
        self.skybox_background_view = None;
        self.skybox_background_bind_group = None;
        self.ensure_default_textures();

        self.stats = TerrainStats::default();

        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.enabled {
            return Ok(());
        }

        let update_started = std::time::Instant::now();

        // Update seismic simulations
        self.update_seismic_simulations();

        // Update subsystems
        let texture_started = std::time::Instant::now();
        self.texture_system.update()?;
        let texture_elapsed = texture_started.elapsed();

        let water_started = std::time::Instant::now();
        self.water_system.update()?;
        let water_elapsed = water_started.elapsed();

        let road_started = std::time::Instant::now();
        self.road_system.update()?;
        let road_elapsed = road_started.elapsed();

        if let Some(height_map) = self.height_map.as_ref() {
            if self.road_system.needs_terrain_normal_reprojection() {
                self.road_system.apply_terrain_heights_and_normals(
                    |pos| height_map.get_height_at(pos.x, pos.z),
                    |pos| height_map.get_normal_at(pos.x, pos.z),
                );
            }
        }

        let road_meshes_started = std::time::Instant::now();
        self.update_road_meshes()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        let road_meshes_elapsed = road_meshes_started.elapsed();

        let chunk_manager_started = std::time::Instant::now();
        self.chunk_manager.update()?;
        let chunk_manager_elapsed = chunk_manager_started.elapsed();

        let chunk_meshes_started = std::time::Instant::now();
        self.update_chunk_meshes()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        let chunk_meshes_elapsed = chunk_meshes_started.elapsed();

        self.stats.update_time_ms =
            self.chunk_manager.get_stats().update_time.as_secs_f64() * 1000.0;

        let update_elapsed = update_started.elapsed();
        if update_elapsed >= std::time::Duration::from_millis(200) {
            warn!(
                "TerrainVisual::update breakdown: total={:?} texture={:?} water={:?} roads={:?} road_meshes={:?} chunk_manager={:?} chunk_meshes={:?} visible={} pending_visible={} total_chunks={}",
                update_elapsed,
                texture_elapsed,
                water_elapsed,
                road_elapsed,
                road_meshes_elapsed,
                chunk_manager_elapsed,
                chunk_meshes_elapsed,
                self.chunk_manager.get_visible_chunks().len(),
                self.chunk_manager.pending_visible_chunk_count(),
                self.chunk_manager.total_chunk_count()
            );
        }

        Ok(())
    }
}

impl TerrainVisual for TerrainVisualImpl {
    fn render(&mut self, view_matrix: &Mat4, projection_matrix: &Mat4) -> Result<(), TerrainError> {
        if !self.enabled {
            return Ok(());
        }

        if self.water_plane.is_none() {
            if let Some(device) = self.device.as_ref().cloned() {
                self.sync_global_water_plane(device.as_ref())?;
            }
        }

        let view_proj = *projection_matrix * *view_matrix;
        let camera_inverse = view_matrix.inverse();
        let camera_position = camera_inverse.transform_point3(Vec3::ZERO);
        self.recenter_draw_area_on_world_position(camera_position.x, camera_position.z);
        self.chunk_manager.set_camera(camera_position);
        self.chunk_manager.set_view_frustum(ViewFrustum {
            planes: [Vec3::ZERO; 6],
            view_matrix: *view_matrix,
            projection_matrix: *projection_matrix,
        });

        // Update uniforms
        if let (Some(queue), Some(uniform_buffer)) = (self.queue.as_ref(), &self.uniform_buffer) {
            let uniforms = TerrainUniforms {
                view_proj: matrix4_to_array(&view_proj),
                view_matrix: matrix4_to_array(view_matrix),
                projection_matrix: matrix4_to_array(projection_matrix),
                camera_position: [camera_position.x, camera_position.y, camera_position.z, 1.0],
                time: self.time,
                sun_direction: self.sun_direction.to_array(),
                sun_color: self.sun_color,
                ambient_color: self.ambient_color,
                fog_color: self.fog_color,
                fog_start: self.fog_start,
                fog_end: self.fog_end,
                _padding: [0.0; 2],
            };

            queue.write_buffer(uniform_buffer, 0, cast_slice(&[uniforms]));
        }

        // Render water
        self.water_system.render(view_matrix, projection_matrix)?;

        // Render roads
        self.road_system.render(view_matrix, projection_matrix)?;

        Ok(())
    }

    fn get_height_at(&self, x: f32, y: f32) -> Result<f32, TerrainError> {
        if let Some(ref heightmap) = self.height_map {
            Ok(heightmap.get_height_at(x, y))
        } else {
            Ok(0.0)
        }
    }

    fn get_normal_at(&self, x: f32, y: f32) -> Result<Vec3, TerrainError> {
        if let Some(ref heightmap) = self.height_map {
            Ok(heightmap.get_normal_at(x, y))
        } else {
            Ok(Vec3::new(0.0, 0.0, 1.0))
        }
    }

    fn is_valid_position(&self, x: f32, y: f32) -> bool {
        x >= 0.0 && y >= 0.0 && x < self.config.world_size.0 && y < self.config.world_size.1
    }

    fn chunk_manager(&self) -> &ChunkManager {
        &self.chunk_manager
    }

    fn chunk_draw_count(&self) -> usize {
        self.chunk_draw_count()
    }

    fn oversize_terrain(&mut self, amount: i32) {
        TerrainVisualImpl::oversize_terrain(self, amount);
    }
}

/// Terrain uniform data for shaders
#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct TerrainUniforms {
    view_proj: [[f32; 4]; 4],
    view_matrix: [[f32; 4]; 4],
    projection_matrix: [[f32; 4]; 4],
    camera_position: [f32; 4],
    time: f32,
    sun_direction: [f32; 3],
    sun_color: [f32; 3],
    ambient_color: [f32; 3],
    fog_color: [f32; 3],
    fog_start: f32,
    fog_end: f32,
    _padding: [f32; 2],
}

unsafe impl bytemuck::Pod for TerrainUniforms {}
unsafe impl bytemuck::Zeroable for TerrainUniforms {}

/// Water vertex for rendering
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct WaterVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coords: [f32; 2],
    pub flow_direction: [f32; 2],
}

impl WaterVertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<WaterVertex>() as wgpu::BufferAddress,
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
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

unsafe impl bytemuck::Pod for WaterVertex {}
unsafe impl bytemuck::Zeroable for WaterVertex {}

/// Road vertex for rendering
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct RoadVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coords: [f32; 2],
    pub road_width: f32,
}

impl RoadVertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RoadVertex>() as wgpu::BufferAddress,
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
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}

unsafe impl bytemuck::Pod for RoadVertex {}
unsafe impl bytemuck::Zeroable for RoadVertex {}

// Re-export the main implementation
pub use TerrainVisualImpl as TerrainVisualSystem;

// Global singleton instance (matching C++ pattern)
lazy_static::lazy_static! {
    pub static ref THE_TERRAIN_VISUAL: std::sync::Mutex<Option<TerrainVisualImpl>> = std::sync::Mutex::new(None);
}

/// Initialize the global terrain visual instance
pub fn init_terrain_visual() -> TerrainResult<()> {
    let mut global_instance = THE_TERRAIN_VISUAL.lock().unwrap_or_else(|e| e.into_inner());
    *global_instance = Some(TerrainVisualImpl::new());
    Ok(())
}

/// Get reference to global terrain visual instance
pub fn get_terrain_visual(
) -> Result<std::sync::MutexGuard<'static, Option<TerrainVisualImpl>>, TerrainError> {
    THE_TERRAIN_VISUAL.lock().map_err(|_| {
        TerrainError::InitializationError("Failed to lock terrain visual mutex".to_string())
    })
}
