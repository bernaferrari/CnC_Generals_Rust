//! # Terrain Visual System
//!
//! Core terrain rendering system that matches the C++ TerrainVisual implementation exactly.
//! Handles heightmaps, texturing, water, roads, and all visual terrain features.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::f32::consts::PI;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use glam::{Mat4, Vec2, Vec3, Vec4Swizzles};
use log::{debug, info, warn};
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
use super::scorch_mesh::{
    bake_terrain_scorch_gpu_mesh, terrain_scorch_count, terrain_scorches_in_buffer,
};
use super::terrain_tracks::{TerrainTrackHeightProvider, TerrainTracksConfig};
use super::textures::{
    MAX_BLEND_WEIGHTS, NUM_SOURCE_TILES, TerrainTexture, TerrainTextures, TextureId, TextureKind,
    TextureRule, TextureWeights, TileData,
};
use super::tree_buffer::{
    TREE_MAX_GLOBAL_LIGHTS, TreeGpuVertex, TreeObjectLight, W3DTreeBuffer,
    fill_tree_gpu_upload_vertices,
};
use super::w3d_overlay_mesh::{
    BRIDGE_FLOAT_AMT, DEFAULT_ROAD_SCALE, OverlayGpuVertex, ROAD_FLOAT_AMOUNT, WaterGpuVertex,
    WaterTerrainLight, bake_bridge_span, bake_straight_road_segment, bake_water_tiles_world,
    compute_standing_water_diffuse, default_sectional_bridge_model,
    fill_bridge_gpu_upload_vertices, fill_road_gpu_upload_vertices, fill_water_gpu_upload_vertices,
};
use crate::fx_list::{DisplayDynamicLight, do_the_dynamic_light, scene_dynamic_lights};

use super::{
    ExtraBlendDrawMesh, HeightMap, RoadSystem, TerrainConfig, TerrainError, TerrainLOD,
    TerrainModification, TerrainResult, TerrainStats, TerrainTracksRenderObjClassSystem,
    TerrainVertex, TerrainVisual, WaterSystem, calculate_terrain_lod,
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
    MAP_PREVIEW_DIR_PATH, TERRAIN_TGA_DIR_PATH, TGA_DIR_PATH, USER_TGA_DIR_PATH,
};
use image::GenericImageView;
use image::ImageFormat;

// Live `terrain_visual` module. `include!` keeps one logical module so field
// privacy and the public API stay identical to the former dump.

include!("types.rs");
include!("visual_struct.rs");
include!("impl_core.rs");
include!("impl_roads.rs");
include!("impl_lighting.rs");
include!("impl_gpu.rs");
include!("impl_pipelines.rs");
include!("impl_world.rs");
include!("overlay_gpu.rs");
include!("river_gpu.rs");
include!("shroud_gpu.rs");
include!("traits.rs");
include!("tests.rs");
include!("api.rs");

/// Concatenated live sources for residual `include_str!` scans.
pub const TERRAIN_VISUAL_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("types.rs"),
    include_str!("visual_struct.rs"),
    include_str!("impl_core.rs"),
    include_str!("impl_roads.rs"),
    include_str!("impl_lighting.rs"),
    include_str!("impl_gpu.rs"),
    include_str!("impl_pipelines.rs"),
    include_str!("impl_world.rs"),
    include_str!("overlay_gpu.rs"),
    include_str!("river_gpu.rs"),
    include_str!("shroud_gpu.rs"),
    include_str!("traits.rs"),
    include_str!("api.rs"),
);
