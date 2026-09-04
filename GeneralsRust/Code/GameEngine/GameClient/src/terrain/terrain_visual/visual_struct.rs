// Split from `terrain/terrain_visual.rs` dump. Included by `terrain_visual/mod.rs`.

/// Depth attachment format for every terrain_visual render pipeline.
///
/// All pass call sites that record terrain draws bind this same depth texture:
/// Main's "main terrain pass" (`pipeline_prewarm.rs`, ww3d frame
/// `depth_view_arc()`), GameClient `Display` (`display.rs` `depth_view`,
/// `Depth32Float`), and the W3D renderer forward/depth-prepass views
/// (`w3d/renderer.rs`, `Depth32Float`). wgpu validates each `set_pipeline`
/// against the live attachment, so a pipeline authored with
/// `Depth24PlusStencil8` fatals on first use ("Render pipeline targets are
/// incompatible with render pass"). Keep the formats identical.
pub(super) const TERRAIN_PIPELINES_DEPTH_FORMAT: wgpu::TextureFormat =
    wgpu::TextureFormat::Depth32Float;

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

    /// C++ WorldHeightMap source tile data used for terrain color/radar sampling.
    source_tiles: Vec<Option<TileData>>,
    /// C++ `m_textureClasses` — firstTile/numTiles/name for getTextureClassFromNdx.
    source_tile_classes: Vec<TerrainSourceTileClass>,
    /// Parallel to `source_tiles`: `true` where the tile was synthesized by
    /// `stand_in_tile_bgra` because the real `Art/Terrain` TGA did not
    /// resolve. C++ `WorldHeightMap::getTerrainColorAt` samples real tile
    /// art only (WorldHeightMap.cpp:2347-2356); the radar paint source uses
    /// this to report missing art instead of sampling hash placeholders.
    stand_in_source_tiles: Vec<bool>,


    /// Water rendering system
    water_system: WaterSystem,

    /// Road rendering system
    road_system: RoadSystem,

    /// Terrain track rendering system.
    terrain_tracks: TerrainTracksRenderObjClassSystem,
    /// C++ `WaterTracksRenderSystem` ship-wake / shore-wave tracks.
    water_tracks: crate::terrain::WaterTracksRenderSystem,
    /// Last CPU flush from `WaterTracksRenderSystem::flush` (live water record).
    last_water_tracks_flush: crate::terrain::WaterTracksFlush,


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
    water_additive_pipeline: Option<wgpu::RenderPipeline>,
    /// Bind group 1: standing-water albedo + wrap sampler (C++ TWWater01.tga).
    water_texture_bind_group_layout: Option<Arc<wgpu::BindGroupLayout>>,
    water_texture: Option<Texture>,
    water_sampler: Option<Sampler>,
    water_texture_bind_group: Option<BindGroup>,
    /// True when the bound albedo is the 1x1 teal fallback, not TWWater01.
    water_texture_is_fallback: bool,
    bound_standing_water_texture: String,
    /// Last Water.ini standing-water name this visual attempted to bind.
    /// C++ binds INI overrides once (WaterRenderObjClass::updateMapOverrides
    /// on map load); re-binding every frame reloaded the DDS 60x/s.
    requested_standing_water_texture: String,
    /// True once the water texture search found nothing; the teal fallback
    /// bind group stays in place and the search is not re-run per frame.
    water_texture_search_exhausted: bool,
    water_additive_blend: bool,
    river_gpu: RiverGpuState,
    shroud_gpu: ShroudGpuState,
    water_named_bind_groups: HashMap<String, NamedWaterBind>,
    road_pipeline: Option<wgpu::RenderPipeline>,
    /// C++ W3DSnow _PresetAlphaShader DEPTH_WRITE_DISABLE.
    snow_pipeline: Option<wgpu::RenderPipeline>,
    /// Bind group 1: road albedo + repeat sampler (C++ RoadType::applyTexture).
    road_texture_bind_group_layout: Option<Arc<wgpu::BindGroupLayout>>,
    road_texture: Option<Texture>,
    road_sampler: Option<Sampler>,
    road_texture_bind_group: Option<BindGroup>,
    /// True when the bound albedo is the 2x2 gravel fallback, not a Roads.ini texture.
    road_texture_is_fallback: bool,
    /// True once the road texture search found nothing; the gravel fallback
    /// bind group stays in place and the search is not re-run per frame.
    road_texture_search_exhausted: bool,
    /// True once the snow texture search found nothing for the current
    /// wanted name; retried only when the Weather.ini name changes.
    snow_texture_search_exhausted: bool,
    /// True once the scorch texture search found nothing for the current
    /// wanted name.
    scorch_texture_search_exhausted: bool,
    tree_pipeline: Option<wgpu::RenderPipeline>,

    /// Terrain textures
    heightmap_texture: Option<Texture>,
    blend_texture: Option<Texture>,
    detail_textures: Vec<Texture>,
    skybox_textures: [Option<Texture>; 5],
    initial_skybox_texture_names: [Option<String>; 5],
    current_skybox_texture_names: [Option<String>; 5],
    skybox_background_view: Option<TextureView>,
    skybox_background_bind_group: Option<BindGroup>,
    skybox_background_pipeline: Option<wgpu::RenderPipeline>,
    skybox_background_bind_group_layout: Option<Arc<wgpu::BindGroupLayout>>,
    skybox_sampler: Option<Sampler>,
    /// Last skybox face that produced a GPU bind (or fog fallback).
    last_skybox_face_bind: Option<String>,

    /// Seismic simulation
    seismic_simulations: Vec<SeismicSimulationNode>,

    /// Water grid enabled
    water_grid_enabled: bool,

    /// Static water handle
    grid_water_handle: WaterHandle,

    /// CPU water-grid state.
    water_grid: WaterGridCpuState,

    /// CPU terrain bib records passed through W3DTerrainVisual.
    terrain_bibs: Vec<TerrainBibRecord>,

    /// CPU terrain prop records passed through W3DTerrainVisual.
    terrain_props: Vec<TerrainPropRecord>,

    /// CPU construction-clearing requests passed through W3DTerrainVisual.
    construction_removals: Vec<TerrainConstructionRemoval>,

    /// Cached GPU meshes for terrain chunks
    chunk_meshes: HashMap<ChunkId, GpuChunkMesh>,

    /// Rule set for procedural texture selection
    texture_rules: Vec<TextureRule>,

    /// Global C++-style water plane rendered for the active map.
    water_plane: Option<GpuWaterPlane>,
    /// Uploaded C++ `WaterTracksRenderSystem::flush` geometry.
    water_track_meshes: Vec<GpuWaterPlane>,


    /// Cached GPU meshes for visible road surfaces.
    road_meshes: Vec<GpuRoadMesh>,
    /// Cached GPU meshes for W3D sectional/fixed bridges.
    bridge_meshes: Vec<GpuRoadMesh>,
    /// Cached GPU mesh for C++ terrain scorch marks.
    scorch_meshes: Vec<GpuRoadMesh>,
    /// Bind group 1: `EXScorch01.tga` (C++ `ScorchTextureClass`).
    scorch_texture: Option<Texture>,
    scorch_sampler: Option<Sampler>,
    scorch_texture_bind_group: Option<BindGroup>,
    scorch_texture_name: String,
    scorch_texture_is_fallback: bool,
    /// Last frame had `createLightPulse` scene lights (force one static restore).
    had_dynamic_lights: bool,
    /// C++ `W3DRoadBuffer::loadRoads` uploads once; only rebuild GPU overlays
    /// after map/load/device/scorch changes. Re-uploading every frame is 250ms+.
    overlay_gpu_meshes_dirty: bool,

    /// C++ `W3DTreeBuffer` owned by the shipped wgpu terrain visual.
    tree_buffer: W3DTreeBuffer,
    /// Last CPU→GPU tree VB filled with `doLighting` (every draw/update).
    last_tree_gpu_vertices: Vec<TreeGpuVertex>,
    /// CPU mips actually uploaded (after SetLOD skip), BGRA A8R8G8B8.
    last_tree_atlas_mips: Vec<Vec<u8>>,
    /// Cached GPU meshes for W3D trees.
    tree_meshes: Vec<GpuTreeMesh>,
    tree_atlas_texture: Option<wgpu::Texture>,
    /// Bind group 1: tree atlas + clamp sampler (C++ `Set_Texture(0, m_treeTexture)`).
    /// Owned separately from `road_texture_bind_group` / terrain chunk groups.
    tree_atlas_bind_group_layout: Option<Arc<wgpu::BindGroupLayout>>,
    tree_atlas_sampler: Option<Sampler>,
    tree_atlas_bind_group: Option<BindGroup>,

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

    /// C++ `m_extraBlendTilePositions` packed as `i | (j << 16)`.
    extra_blend_tile_positions: Vec<u32>,
    /// Last extra-blend overlay staged/uploaded for the GPU terrain pass.
    extra_blend_gpu_upload: ExtraBlendGpuUpload,
    /// Last CPU extra-blend draw mesh (two triangles per tile).
    extra_blend_draw_mesh: ExtraBlendDrawMesh,
    /// GPU buffer of packed extra-blend tile positions (when a device exists).
    extra_blend_position_buffer: Option<Buffer>,
    /// GPU extra-blend overlay vertex buffer (second 3-way pass).
    extra_blend_vertex_buffer: Option<Buffer>,
    /// GPU extra-blend overlay index buffer.
    extra_blend_index_buffer: Option<Buffer>,
    extra_blend_index_count: u32,
    extra_blend_vertex_count: u32,
    /// Alpha-blend overlay pipeline (depth write off). Falls back to terrain.
    extra_blend_pipeline: Option<wgpu::RenderPipeline>,
    /// Shipped draw-counter incremented by `extra_blend_draw` / `record_extra_blend_pass`.
    extra_blend_draw_count: AtomicU32,

    /// Shore/bib/track/snow/smudge/water-area CPU overlay state.
    overlay: OverlayGpuState,
    shoreline_meshes: Vec<GpuWaterPlane>,
    water_grid_mesh: Option<GpuWaterPlane>,
    polygon_water_meshes: Vec<GpuWaterPlane>,
    bib_meshes: Vec<GpuRoadMesh>,
    tank_track_meshes: Vec<GpuRoadMesh>,
    custom_edge_meshes: Vec<GpuRoadMesh>,
    snow_mesh: Option<GpuRoadMesh>,
    snow_texture: Option<Texture>,
    snow_sampler: Option<Sampler>,
    snow_texture_bind_group: Option<BindGroup>,
    snow_texture_name: String,
    snow_texture_is_fallback: bool,
    smudge_mesh: Option<GpuRoadMesh>,
    flat_lod_meshes: Vec<GpuRoadMesh>,
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
    texture_name: String,
    jba: bool,
}

struct GpuRoadMesh {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    index_count: u32,
}

struct GpuTreeMesh {
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
    [80, 80, 80, 255],    // Missing/neutral (was snow-white scraps)
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

fn positive_usize(value: i32) -> Option<usize> {
    (value > 0).then_some(value as usize)
}
