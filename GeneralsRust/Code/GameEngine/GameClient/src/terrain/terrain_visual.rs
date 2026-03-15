//! # Terrain Visual System
//!
//! Core terrain rendering system that matches the C++ TerrainVisual implementation exactly.
//! Handles heightmaps, texturing, water, roads, and all visual terrain features.

use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use glam::{Mat4, Vec3, Vec4Swizzles};
use log::{debug, warn};
use wgpu::util::DeviceExt;
use wgpu::{BindGroup, BindGroupLayout, Buffer, RenderPass, Sampler, Texture, TextureView};

use crate::display::image::GameImageError;
use crate::system::SubsystemInterface;
// use crate::display::{RenderDevice, WgpuRenderer}; // These don't exist yet
use super::chunk::{ChunkId, ChunkManager, ViewFrustum};
use super::roads::RoadMinimapSample;
use super::textures::{
    TerrainTexture, TerrainTextures, TextureId, TextureKind, TextureRule, MAX_BLEND_WEIGHTS,
};
use super::{
    calculate_terrain_lod, HeightMap, RoadSystem, TerrainConfig, TerrainError, TerrainLOD,
    TerrainModification, TerrainResult, TerrainStats, TerrainVertex, TerrainVisual, WaterSystem,
};
use bytemuck::cast_slice;
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::ini::get_global_data;
use game_engine::common::ini::ini_terrain;
use game_engine::common::ini::ini_terrain::{TerrainSurface, TerrainType};
use game_engine::common::system::file::FileAccess;
use game_engine::common::system::file_system::get_file_system;
use image::GenericImageView;
use image::ImageFormat;

/// Water handle for terrain water systems
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WaterHandle(pub u32);

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
                Vec3::new(
                    center.x - region_size,
                    center.y - region_size,
                    center.z - region_size,
                ),
                Vec3::new(
                    center.x + region_size,
                    center.y + region_size,
                    center.z + region_size,
                ),
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

    /// Camera bind group layout used by the terrain pipeline
    terrain_camera_bind_group_layout: Option<Arc<wgpu::BindGroupLayout>>,

    /// Texture bind group layout used by the terrain pipeline
    terrain_texture_bind_group_layout: Option<Arc<wgpu::BindGroupLayout>>,

    /// Camera bind group providing view/projection matrices
    terrain_camera_bind_group: Option<wgpu::BindGroup>,

    /// Terrain texture sampler used by the shader
    terrain_sampler: Option<wgpu::Sampler>,

    /// Per-chunk texture bind groups and slot maps
    chunk_texture_bindings: HashMap<ChunkId, ChunkTextureBinding>,

    /// Shared visible-terrain texture set used to keep adjacent chunks on the same slot map.
    active_chunk_texture_ids: Option<[TextureId; MAX_TEXTURES_PER_CHUNK]>,
    /// Signature of the visible chunk set used to avoid recomputing texture slots every frame.
    last_visible_chunk_signature: u64,
    /// Per-frame budget for expensive CPU/GPU chunk mesh uploads.
    max_chunk_mesh_uploads_per_frame: usize,

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
const MAX_TEXTURE_SAMPLE_CHUNKS: usize = 24;
const MAX_TEXTURE_SAMPLES_PER_CHUNK: usize = 128;

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
            terrain_camera_bind_group_layout: None,
            terrain_texture_bind_group_layout: None,
            terrain_camera_bind_group: None,
            terrain_sampler: None,
            chunk_texture_bindings: HashMap::new(),
            active_chunk_texture_ids: None,
            last_visible_chunk_signature: 0,
            max_chunk_mesh_uploads_per_frame: 2,
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

        // The active Rust shell path was still clipping terrain to a small draw rectangle
        // pinned at the map origin. That produced visible black cutouts even though the
        // terrain chunks and frustum culling were otherwise valid. Prefer the whole loaded
        // terrain surface until the legacy draw-area paging behavior is ported faithfully.
        if let Some((map_width, map_height)) = self.map_sample_dimensions() {
            self.draw_width = map_width.max(1);
            self.draw_height = map_height.max(1);
            self.draw_origin_x = 0;
            self.draw_origin_y = 0;
        } else {
            self.draw_width = NORMAL_DRAW_WIDTH;
            self.draw_height = NORMAL_DRAW_HEIGHT;
            self.draw_origin_x = 0;
            self.draw_origin_y = 0;
        }

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
        match self.map_sample_dimensions() {
            Some(_) => chunks
                .into_iter()
                .filter(|chunk| self.chunk_intersects_draw_area(chunk))
                .map(|chunk| chunk.id)
                .collect(),
            None => chunks.into_iter().map(|chunk| chunk.id).collect(),
        }
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
            "windows_game/extracted_big_files/INIZH/Data/INI/Default/Terrain.ini",
            "windows_game/extracted_big_files/INIZH/Data/INI/Terrain.ini",
            "windows_game/extracted_big_files_v2/INIZH/Data/INI/Default/Terrain.ini",
            "windows_game/extracted_big_files_v2/INIZH/Data/INI/Terrain.ini",
            "../windows_game/extracted_big_files/INIZH/Data/INI/Default/Terrain.ini",
            "../windows_game/extracted_big_files/INIZH/Data/INI/Terrain.ini",
            "../windows_game/extracted_big_files_v2/INIZH/Data/INI/Default/Terrain.ini",
            "../windows_game/extracted_big_files_v2/INIZH/Data/INI/Terrain.ini",
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
                let mut fallback_candidate = None;
                let mut resolved_candidate = None;

                for terrain in terrains {
                    let texture = terrain.texture_name.as_str().trim();
                    if texture.is_empty() {
                        continue;
                    }

                    let texture_key = texture.to_ascii_lowercase();
                    if !used_textures.insert(texture_key) {
                        continue;
                    }

                    if resolved_candidate.is_none()
                        && TerrainTextures::is_available_terrain_texture_path(texture)
                    {
                        resolved_candidate = Some(terrain);
                        break;
                    }

                    if fallback_candidate.is_none() {
                        fallback_candidate = Some(terrain);
                    }
                }

                let candidate = resolved_candidate.or(fallback_candidate);

                if let Some(terrain) = candidate {
                    let texture_id = self.texture_system.register_texture(TerrainTexture::new(
                        0,
                        terrain.name.as_str().to_string(),
                        terrain.texture_name.as_str().to_string(),
                    ));
                    defaults.push(texture_id);
                }
            }
        }

        if defaults.is_empty() {
            let grass = self.texture_system.register_texture(TerrainTexture::new(
                0,
                "Grass".to_string(),
                "Data/Terrain/Grass.dds".to_string(),
            ));
            let cliff = self.texture_system.register_texture(TerrainTexture::new(
                0,
                "Cliff".to_string(),
                "Data/Terrain/Cliff.dds".to_string(),
            ));
            let snow = self.texture_system.register_texture(TerrainTexture::new(
                0,
                "Snow".to_string(),
                "Data/Terrain/Snow.dds".to_string(),
            ));
            let sand = self.texture_system.register_texture(TerrainTexture::new(
                0,
                "Sand".to_string(),
                "Data/Terrain/Sand.dds".to_string(),
            ));

            defaults.extend([grass, cliff, snow, sand]);
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
        self.active_chunk_texture_ids = None;
        self.last_visible_chunk_signature = 0;
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

    fn prepare_chunk_texture_binding(
        &mut self,
        chunk_id: ChunkId,
        texture_ids: &[TextureId],
        slot_map: HashMap<TextureId, usize>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> TerrainResult<()> {
        let layout = match &self.terrain_texture_bind_group_layout {
            Some(layout) => Arc::clone(layout),
            None => return Ok(()),
        };

        if self.terrain_sampler.is_none() {
            self.terrain_sampler = Some(device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Terrain Texture Sampler"),
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }));
        }

        let sampler = self
            .terrain_sampler
            .as_ref()
            .expect("terrain sampler should be initialised");

        let mut final_ids = [0; MAX_TEXTURES_PER_CHUNK];
        if texture_ids.is_empty() {
            final_ids = [0; MAX_TEXTURES_PER_CHUNK];
        } else {
            for (idx, id) in texture_ids.iter().enumerate().take(MAX_TEXTURES_PER_CHUNK) {
                final_ids[idx] = *id;
            }
            for idx in texture_ids.len()..MAX_TEXTURES_PER_CHUNK {
                final_ids[idx] = texture_ids[0];
            }
        }

        for texture_id in final_ids.iter() {
            if let Err(err) = self.texture_system.load_texture(*texture_id) {
                warn!("Failed to load terrain texture {}: {}", texture_id, err);
            }
        }

        let mut diffuse_views = Vec::with_capacity(MAX_TEXTURES_PER_CHUNK);

        for (binding, texture_id) in final_ids.iter().enumerate() {
            let diffuse_fallback = DEFAULT_TERRAIN_COLORS[binding % DEFAULT_TERRAIN_COLORS.len()];
            let diffuse_view = self.texture_system.acquire_texture_view(
                *texture_id,
                TextureKind::Diffuse,
                device,
                queue,
                diffuse_fallback,
            )?;
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
                slot_map,
                texture_ids: final_ids,
                diffuse_views,
            },
        );

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
        let device = match &self.device {
            Some(device) => Arc::clone(device),
            None => return Ok(()),
        };

        let queue = match &self.queue {
            Some(queue) => Arc::clone(queue),
            None => return Ok(()),
        };

        let visible_chunk_ids = self.visible_chunk_ids_for_draw_area();
        let visible_signature = self.visible_chunk_signature(&visible_chunk_ids);
        let refresh_texture_slots = self.active_chunk_texture_ids.is_none()
            || self.last_visible_chunk_signature != visible_signature;

        let stable_texture_ids = if refresh_texture_slots {
            let ids = self.select_stable_chunk_texture_ids(&visible_chunk_ids);
            self.active_chunk_texture_ids = Some(ids);
            self.last_visible_chunk_signature = visible_signature;
            ids
        } else {
            self.active_chunk_texture_ids.unwrap_or([0; MAX_TEXTURES_PER_CHUNK])
        };

        let texture_slots_changed = refresh_texture_slots;

        let mut shared_slot_map: HashMap<TextureId, usize> = HashMap::new();
        for (slot, texture_id) in stable_texture_ids.iter().enumerate() {
            shared_slot_map.entry(*texture_id).or_insert(slot);
        }

        let mut upload_budget = self.max_chunk_mesh_uploads_per_frame.max(1);
        for chunk_id in visible_chunk_ids {
            let chunk = match self.chunk_manager.get_chunk(chunk_id) {
                Some(chunk) => chunk.clone(),
                None => continue,
            };

            let needs_upload = match self.chunk_meshes.get(&chunk.id) {
                Some(mesh)
                    if mesh.revision == chunk.geometry_revision
                        && !texture_slots_changed
                        && self
                            .chunk_texture_bindings
                            .get(&chunk.id)
                            .map(|binding| binding.texture_ids == stable_texture_ids)
                            .unwrap_or(false) =>
                {
                    false
                }
                _ => true,
            };

            if needs_upload {
                if upload_budget == 0 {
                    continue;
                }
                upload_budget -= 1;

                if chunk.vertices.is_empty() || chunk.indices.is_empty() {
                    continue;
                }

                let mut gpu_vertices = chunk.vertices.clone();
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

                self.prepare_chunk_texture_binding(
                    chunk.id,
                    &stable_texture_ids,
                    shared_slot_map.clone(),
                    device.as_ref(),
                    queue.as_ref(),
                )?;

                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Terrain Chunk {} Vertex Buffer", chunk.id)),
                    contents: cast_slice(&gpu_vertices),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Terrain Chunk {} Index Buffer", chunk.id)),
                    contents: cast_slice(&chunk.indices),
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                });

                self.chunk_meshes.insert(
                    chunk.id,
                    GpuChunkMesh {
                        vertex_buffer,
                        index_buffer,
                        index_count: chunk.indices.len() as u32,
                        revision: chunk.geometry_revision,
                    },
                );
            }
        }

        self.chunk_meshes
            .retain(|id, _| self.chunk_manager.has_chunk(*id));

        self.chunk_texture_bindings
            .retain(|id, _| self.chunk_manager.has_chunk(*id));

        let visible_chunks = self.visible_chunk_ids_for_draw_area();
        self.stats.rendered_chunks = visible_chunks.len();
        self.stats.triangles_rendered = visible_chunks
            .iter()
            .filter_map(|chunk_id| self.chunk_manager.get_chunk(*chunk_id))
            .map(|chunk| chunk.stats.triangle_count as usize)
            .sum();

        Ok(())
    }

    fn visible_chunk_signature(&self, visible_chunk_ids: &[ChunkId]) -> u64 {
        let mut hasher = DefaultHasher::new();
        visible_chunk_ids.len().hash(&mut hasher);
        for &chunk_id in visible_chunk_ids {
            chunk_id.hash(&mut hasher);
            if let Some(chunk) = self.chunk_manager.get_chunk(chunk_id) {
                chunk.geometry_revision.hash(&mut hasher);
                chunk.lod_level.hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    fn select_stable_chunk_texture_ids(
        &mut self,
        visible_chunk_ids: &[ChunkId],
    ) -> [TextureId; MAX_TEXTURES_PER_CHUNK] {
        let mut visible_texture_contributions: HashMap<TextureId, f32> = HashMap::new();

        for &chunk_id in visible_chunk_ids.iter().take(MAX_TEXTURE_SAMPLE_CHUNKS) {
            let Some(chunk) = self.chunk_manager.get_chunk(chunk_id) else {
                continue;
            };
            if chunk.vertices.is_empty() {
                continue;
            }

            let sample_step =
                (chunk.vertices.len() / MAX_TEXTURE_SAMPLES_PER_CHUNK.max(1)).max(1);
            for vertex in chunk.vertices.iter().step_by(sample_step) {
                let position =
                    Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
                let normal_vec = Vec3::new(vertex.normal[0], vertex.normal[1], vertex.normal[2]);
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

                for (texture_id, weight) in blended.iter_pairs() {
                    visible_texture_contributions
                        .entry(texture_id)
                        .and_modify(|total| *total += weight)
                        .or_insert(weight);
                }
            }
        }

        let mut sorted_visible_textures: Vec<(TextureId, f32)> =
            visible_texture_contributions.into_iter().collect();
        sorted_visible_textures
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut selected_textures = Vec::new();
        for (texture_id, _) in &sorted_visible_textures {
            if selected_textures
                .iter()
                .all(|candidate| candidate != texture_id)
            {
                selected_textures.push(*texture_id);
                if selected_textures.len() == MAX_TEXTURES_PER_CHUNK {
                    break;
                }
            }
        }

        if selected_textures.is_empty() {
            if let Some(rule) = self.texture_rules.first() {
                selected_textures.push(rule.texture_id);
            } else {
                selected_textures.push(0);
            }
        }

        while selected_textures.len() < MAX_TEXTURES_PER_CHUNK {
            let fallback = *selected_textures.first().unwrap_or(&0);
            selected_textures.push(fallback);
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

    pub fn record_chunk_draws<'pass>(&self, pass: &mut RenderPass<'pass>) {
        if !self.enabled {
            return;
        }

        self.record_skybox_background_draw(pass);

        if let Some(pipeline) = &self.terrain_pipeline {
            pass.set_pipeline(pipeline);
            if let Some(camera_bg) = &self.terrain_camera_bind_group {
                pass.set_bind_group(0, camera_bg, &[]);
            }
            for chunk_id in self.visible_chunk_ids_for_draw_area() {
                let Some(chunk) = self.chunk_manager.get_chunk(chunk_id) else {
                    continue;
                };
                if let Some(binding) = self.chunk_texture_bindings.get(&chunk.id) {
                    pass.set_bind_group(1, &binding.bind_group, &[]);
                } else {
                    warn!(
                        "Missing texture bind group for chunk {}; using previous binding",
                        chunk.id
                    );
                }

                if let Some(mesh) = self.chunk_meshes.get(&chunk.id) {
                    pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                }
            }
        }

        self.record_water_draws(pass);
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

    fn record_water_draws<'pass>(&self, pass: &mut RenderPass<'pass>) {
        let (Some(water_plane), Some(water_pipeline), Some(camera_bg)) = (
            self.water_plane.as_ref(),
            self.water_pipeline.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
        ) else {
            return;
        };

        pass.set_pipeline(water_pipeline);
        pass.set_bind_group(0, camera_bg, &[]);
        pass.set_vertex_buffer(0, water_plane.vertex_buffer.slice(..));
        pass.set_index_buffer(
            water_plane.index_buffer.slice(..),
            wgpu::IndexFormat::Uint32,
        );
        pass.draw_indexed(0..water_plane.index_count, 0, 0..1);
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
        let ids = self.texture_system.load_textures(texture_paths)?;

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
            if simulation.life < 50 {
                // Arbitrary lifetime
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
        let center_y = simulation.center.y;
        let radius = simulation.radius;
        let magnitude = simulation.magnitude;

        let life_factor = 1.0 - (simulation.life as f32 / 50.0);
        let effect_magnitude = magnitude * life_factor;

        // Apply dome-style seismic effect
        for y in 0..heightmap.height as i32 {
            for x in 0..heightmap.width as i32 {
                let world_x = x as f32 * heightmap.scale;
                let world_y = y as f32 * heightmap.scale;

                let dx = world_x - center_x;
                let dy = world_y - center_y;
                let distance = (dx * dx + dy * dy).sqrt();

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

            if candidate.exists() {
                if let Ok(image) = image::open(&candidate) {
                    return Ok(image);
                }
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
        let mut candidates = Vec::<PathBuf>::new();

        if !bare.is_empty() {
            Self::push_unique_path(&mut candidates, PathBuf::from(&bare));
        }

        if !bare.contains('/') {
            Self::push_unique_path(
                &mut candidates,
                PathBuf::from(format!("Art/Textures/{bare}")),
            );
            Self::push_unique_path(
                &mut candidates,
                PathBuf::from(format!("Art/Terrain/{bare}")),
            );
            Self::push_unique_path(
                &mut candidates,
                PathBuf::from(format!("Data/Art/Textures/{bare}")),
            );
            Self::push_unique_path(
                &mut candidates,
                PathBuf::from(format!("Data/Art/Terrain/{bare}")),
            );
            Self::push_unique_path(
                &mut candidates,
                PathBuf::from(format!("English/Art/Textures/{bare}")),
            );
            Self::push_unique_path(
                &mut candidates,
                PathBuf::from(format!("Data/English/Art/Textures/{bare}")),
            );
        }

        if Path::new(&bare).extension().is_none() {
            let current = candidates.clone();
            for base in current {
                for ext in ["tga", "dds", "png", "jpg", "jpeg", "bmp"] {
                    Self::push_unique_path(
                        &mut candidates,
                        PathBuf::from(format!("{}.{}", base.display(), ext)),
                    );
                }
            }
        }

        for root in self.runtime_texture_search_roots() {
            let current = candidates.clone();
            for candidate in current {
                Self::push_unique_path(&mut candidates, root.join(candidate));
            }
        }

        candidates
    }

    fn push_unique_path(list: &mut Vec<PathBuf>, candidate: PathBuf) {
        if !list.iter().any(|existing| existing == &candidate) {
            list.push(candidate);
        }
    }

    fn runtime_texture_search_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        let mut push_unique = |candidate: PathBuf| {
            if !roots.iter().any(|existing| existing == &candidate) {
                roots.push(candidate);
            }
        };

        let mut bases = Vec::new();
        if let Ok(cwd) = std::env::current_dir() {
            bases.push(cwd);
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                bases.push(parent.to_path_buf());
            }
        }
        bases.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));

        for base in bases {
            for ancestor in base.ancestors().take(8) {
                let ancestor = ancestor.to_path_buf();
                push_unique(ancestor.clone());
                push_unique(ancestor.join("windows_game/extracted_big_files/TexturesZH"));
                push_unique(ancestor.join("windows_game/extracted_big_files/EnglishZH"));
                push_unique(ancestor.join("windows_game/extracted_big_files_v2/TexturesZH"));
                push_unique(ancestor.join("windows_game/extracted_big_files_v2/EnglishZH"));
                push_unique(ancestor.join("windows_game/Command & Conquer Generals Zero Hour"));
                push_unique(
                    ancestor.join("windows_game/Command & Conquer Generals Zero Hour/Data"),
                );
            }
        }

        roots
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
        self.last_visible_chunk_signature = 0;
        self.water_plane = None;
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

        // Update seismic simulations
        self.update_seismic_simulations();

        // Update subsystems
        self.texture_system.update()?;
        self.water_system.update()?;
        self.road_system.update()?;
        if let Some(height_map) = self.height_map.as_ref() {
            self.road_system
                .apply_terrain_normals(|pos| height_map.get_normal_at(pos.x, pos.z));
        }
        self.chunk_manager.update()?;
        self.update_chunk_meshes()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        self.stats.update_time_ms =
            self.chunk_manager.get_stats().update_time.as_secs_f64() * 1000.0;

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

        // Update uniforms
        if let (Some(queue), Some(uniform_buffer)) = (self.queue.as_ref(), &self.uniform_buffer) {
            let view_proj = *projection_matrix * *view_matrix;
            let camera_inverse = view_matrix.inverse();
            let camera_position = camera_inverse.transform_point3(Vec3::ZERO);
            self.chunk_manager.set_camera(camera_position);
            self.chunk_manager.set_view_frustum(ViewFrustum {
                planes: [Vec3::ZERO; 6],
                view_matrix: *view_matrix,
                projection_matrix: *projection_matrix,
            });
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
    let mut global_instance = THE_TERRAIN_VISUAL.lock().unwrap();
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
