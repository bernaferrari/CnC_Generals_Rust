//! Compiled RenderPipeline module.
#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    non_snake_case,
    unused_mut,
    unused_assignments,
    clippy::all
)]

use crate::assets::get_asset_manager;
use crate::fow_rendering::ObjectVisibility;
use crate::game_logic::ObjectId as ObjectID;
use crate::ui::UiTextureId;
use anyhow::Result;
use glam::{Mat4, Vec2, Vec3, Vec4};
use log::{debug, error, info, trace, warn};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::graphics_system::GraphicsSystem;
use super::minimap_renderer::{
    MinimapCoordinates, MinimapDimensions, MinimapTextureRenderer, UiTextureRegistrar,
};
use super::render_item::RenderItem;
use crate::assets::textures::RawTexture;
use crate::assets::{ModelPrewarmStats, W3DMaterial, W3DModel};
use ww3d_renderer_3d::material_system::{MaterialPassClass, VertexMaterialClass};
use ww3d_renderer_3d::rendering::{
    camera_system::CameraClass,
    lighting_system::{LightClass, LightEnvironmentClass},
    mesh_system::{MeshClass, MeshModelClass},
    shader_system::shader::{ShaderClass, TexturingType},
    wgpu_main_renderer::{WgpuMainRenderer, WgpuMainRendererConfig},
};
use ww3d_renderer_3d::texture_system::{TextureClass, TextureFormat};
use ww3d_renderer_3d::w3d_format::{
    W3dMaterialInfoStruct, W3dRGBAStruct, W3dTexCoordStruct, W3dTriangleStruct, W3dVectorStruct,
    W3dVertexMaterialStruct,
};
use ww3d_renderer_3d::RendererResult;

#[cfg(feature = "game_client")]
use game_client::system::SubsystemInterface;
#[cfg(feature = "game_client")]
use game_client::terrain::TerrainVisual;

#[cfg(feature = "game_client")]
pub(super) fn terrain_to_main_axis_matrix() -> Mat4 {
    Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(0.0, 1.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

#[cfg(feature = "game_client")]
pub(crate) fn gameplay_to_render_axis_matrix() -> Mat4 {
    terrain_to_main_axis_matrix()
}

#[cfg(feature = "game_client")]
pub(crate) fn gameplay_to_render_transform(matrix: Mat4) -> Mat4 {
    // Main gameplay objects are already stored in the active world basis
    // (X/Z ground, Y-up). Only imported mesh vertex payloads still need axis
    // conversion at build time.
    matrix
}

pub(super) fn transform_has_finite_components(transform: Mat4) -> bool {
    transform
        .to_cols_array()
        .into_iter()
        .all(|value| value.is_finite())
}

pub(super) fn transform_is_reasonable_for_mesh(transform: Mat4) -> bool {
    if !transform_has_finite_components(transform) {
        return false;
    }
    let x = transform.x_axis.truncate().length();
    let y = transform.y_axis.truncate().length();
    let z = transform.z_axis.truncate().length();
    let translation = transform.w_axis.truncate();

    let scales_ok = [x, y, z]
        .into_iter()
        .all(|len| len.is_finite() && len > 1.0e-4 && len < 1.0e4);
    let translation_ok = translation.is_finite() && translation.length() < 2.0e5;
    scales_ok && translation_ok
}

#[derive(Clone, Copy)]
pub(super) struct CullingPlane {
    normal: Vec3,
    distance: f32,
}

pub(super) fn normalized_plane(plane: Vec4) -> CullingPlane {
    let normal = plane.truncate();
    let len = normal.length();
    if !len.is_finite() || len <= f32::EPSILON {
        return CullingPlane {
            normal: Vec3::Y,
            distance: f32::MAX,
        };
    }
    CullingPlane {
        normal: normal / len,
        distance: plane.w / len,
    }
}

pub(super) fn extract_frustum_planes(view_proj: &Mat4) -> [CullingPlane; 6] {
    // Plane extraction uses row-major equations over glam's column-major storage.
    let row0 = Vec4::new(
        view_proj.x_axis.x,
        view_proj.y_axis.x,
        view_proj.z_axis.x,
        view_proj.w_axis.x,
    );
    let row1 = Vec4::new(
        view_proj.x_axis.y,
        view_proj.y_axis.y,
        view_proj.z_axis.y,
        view_proj.w_axis.y,
    );
    let row2 = Vec4::new(
        view_proj.x_axis.z,
        view_proj.y_axis.z,
        view_proj.z_axis.z,
        view_proj.w_axis.z,
    );
    let row3 = Vec4::new(
        view_proj.x_axis.w,
        view_proj.y_axis.w,
        view_proj.z_axis.w,
        view_proj.w_axis.w,
    );

    [
        normalized_plane(row3 + row0), // left
        normalized_plane(row3 - row0), // right
        normalized_plane(row3 + row1), // bottom
        normalized_plane(row3 - row1), // top
        normalized_plane(row3 + row2), // near
        normalized_plane(row3 - row2), // far
    ]
}

pub(super) fn world_sphere_in_expanded_frustum(
    planes: &[CullingPlane; 6],
    world_position: Vec3,
    world_radius: f32,
    camera_position: Vec3,
) -> bool {
    // Conservative sphere culling to mirror C++ `Cull_Sphere` behavior.
    // Slightly larger pad than C++ so first-frame building spheres (fallback
    // radius) survive a zoomed RTS camera without disabling the test.
    const PLANE_MARGIN: f32 = 28.0;
    const NEAR_BYPASS_DISTANCE: f32 = 250.0;

    let radius = world_radius.max(1.0);
    let near_bypass_sq = (NEAR_BYPASS_DISTANCE + radius).powi(2);
    for plane in planes {
        let signed_distance = plane.normal.dot(world_position) + plane.distance;
        if signed_distance < -(radius + PLANE_MARGIN) {
            return world_position.distance_squared(camera_position) <= near_bypass_sq;
        }
    }
    true
}

#[derive(Debug, Clone, Default)]
pub struct CachedLighting {
    pub sun_direction: Option<[f32; 3]>,
    pub sun_color: Option<[f32; 3]>,
    pub ambient_color: Option<[f32; 3]>,
    pub fog_color: Option<[f32; 3]>,
    pub fog_range: Option<(f32, f32)>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TexturePrewarmStats {
    pub requested: usize,
    pub cache_hits: usize,
    pub resolved: usize,
    pub missing: usize,
    pub queued_remaining: usize,
}

pub(super) fn material_stage_texture(material: &W3DMaterial, stage: usize) -> Option<&str> {
    match stage {
        0 => material.stage0_mapping.texture_name.as_deref(),
        1 => material
            .stage1_mapping
            .as_ref()
            .and_then(|mapping| mapping.texture_name.as_deref()),
        2 => material
            .stage2_mapping
            .as_ref()
            .and_then(|mapping| mapping.texture_name.as_deref()),
        3 => material
            .stage3_mapping
            .as_ref()
            .and_then(|mapping| mapping.texture_name.as_deref()),
        _ => None,
    }
}

pub(super) const PROFILE_STEP_LOG_THRESHOLD: Duration = Duration::from_millis(20);

/// Render pipeline stages - equivalent to C++ SAGE RenderPass enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderPass {
    ShadowPass,         // Shadow map generation
    ForwardOpaque,      // Opaque geometry forward rendering
    ForwardTransparent, // Transparent geometry forward rendering
    WaterPass,          // Water surface rendering
    UIPass,             // 2D UI overlay rendering
}

/// Main render pipeline - equivalent to C++ SAGE RenderPipeline
pub struct RenderPipeline {
    // WW3D renderer bridge
    forward_pass: ForwardPass,

    // Minimap FOW renderer
    minimap_renderer: Option<MinimapTextureRenderer>,
    minimap_base_needs_refresh: bool,
    heightmap_path_hint: Option<String>,
    pending_heightmap_hint_load: bool,
    skybox_textures_hint: Option<[String; 5]>,
    skybox_enabled: bool,
    heightmap_world_size: Option<(f32, f32)>,
    /// Object/forward lighting selected at map activation.
    cached_lighting: Option<CachedLighting>,
    /// Terrain uses its own GameData `TerrainLighting*` record. Keeping this
    /// separate prevents object-scene lighting from being reused for terrain
    /// when the authored arrays differ.
    cached_terrain_lighting: Option<CachedLighting>,
    last_startup_model_prewarm_signature: Option<String>,

    // Render items for current frame
    render_items: Vec<RenderItem>,

    // Rendering state
    frame_number: u64,
    current_pass: Option<RenderPass>,

    // FOW state
    current_player_id: u32, // Which player is viewing (for FOW queries)
    missing_ini_objects: HashSet<String>,
    debug_last_alive_objects: usize,
    /// Live GameLogic object identity reads in unit mesh pass (0 when presentation owns pass).
    debug_last_live_unit_identity_reads: usize,
    /// Live GameLogic dual-reads while a presentation frame is installed (must stay 0).
    debug_last_presentation_live_fallback_reads: usize,
    debug_last_fow_filtered: usize,
    debug_last_frustum_culled: usize,
    debug_last_model_missing: usize,
    debug_last_deferred_model_loads: usize,
    debug_last_deferred_model_load_budget: usize,
    debug_last_model_budget_skips: usize,
    debug_last_zero_mesh_models: usize,
    debug_last_missing_model_samples: Vec<String>,
    debug_warned_bad_mesh_transforms: HashSet<String>,
    model_cull_bounds_cache: HashMap<String, (Vec3, f32)>,
    /// Per-object, per-source-Draw-module animation state. Equal W3D names
    /// must not collapse separate retail Draw modules into one timeline.
    animation_states: HashMap<(u32, u32), ObjectAnimationState>,
    last_frame_time: f32,
    /// When set, collect_render_items prefers presentation-owned transforms/model keys.
    presentation_frame: Option<crate::presentation_frame::PresentationFrame>,
    /// Last presentation laser SegLine CPU pack residual (execute path).
    debug_last_laser_segments_packed: u32,
    debug_last_laser_pack_ok: bool,
    /// True when execute submitted laser vertices via `Queue::write_buffer`.
    debug_last_laser_gpu_write_ok: bool,
    /// Last presentation projectile trail CPU pack residual (execute path).
    debug_last_projectile_segments_packed: u32,
    debug_last_projectile_pack_ok: bool,
    /// Last presentation move/attack order line packs (execute path).
    debug_last_move_lines_packed: u32,
    debug_last_attack_lines_packed: u32,
    /// Last presentation floating-text CPU layout residual (execute path).
    debug_last_floating_texts_packed: u32,
    debug_last_floating_text_pack_ok: bool,
    /// Last presentation world-anim CPU layout residual (execute path).
    debug_last_world_anims_packed: u32,
    debug_last_world_anim_pack_ok: bool,
    /// Last presentation particle-system CPU layout residual (execute path).
    debug_last_particle_systems_packed: u32,
    debug_last_particle_pack_ok: bool,
}

pub(super) const DEFAULT_SKYBOX_TEXTURES: [&str; 5] = [
    "TSMorningN.tga",
    "TSMorningE.tga",
    "TSMorningS.tga",
    "TSMorningW.tga",
    "TSMorningT.tga",
];

pub(super) struct ObjectAnimationState {
    /// Exact selected W3D animation from the frozen source Draw state. `None`
    /// deliberately means bind pose; it must not silently become W3D clip 0.
    animation_index: Option<usize>,
    current_frame: f32,
    frame_rate: f32,
    num_frames: u32,
    mode: crate::assets::AuthoredDrawAnimationMode,
}

/// Forward rendering pass powered by the WW3D renderer backend.
pub struct ForwardPass {
    renderer: WgpuMainRenderer,
    mesh_cache: HashMap<String, Arc<MeshModelClass>>,
    texture_cache: HashMap<String, Arc<TextureClass>>,
    pending_texture_stream: VecDeque<String>,
    queued_texture_stream: HashSet<String>,
    fallback_texture: Option<Arc<TextureClass>>,
    camera: CameraClass,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    /// Live SegLine vertex buffer (created on first laser upload).
    laser_vertex_buffer: Option<Arc<wgpu::Buffer>>,
    laser_vertex_capacity: usize,
    laser_vertices_uploaded: u32,
    laser_draw_gpu: Option<crate::graphics::laser_draw::LaserDrawGpu>,
    /// The active Main WGPU frame owns particle presentation.  Keeping this
    /// renderer with the ForwardPass avoids GameClient::Display creating or
    /// presenting a second surface.
    #[cfg(feature = "game_client")]
    particle_renderer: Option<Arc<Mutex<game_client::effects::ParticleRenderer>>>,
}

pub(super) enum RenderModelLoadResult {
    Ready(Arc<W3DModel>),
    SkippedByBudget,
    Failed,
}

mod forward_materials;
mod forward_render;
mod pipeline_collect;
mod pipeline_debug;
mod pipeline_execute;
mod pipeline_lifecycle;
mod pipeline_minimap;
mod pipeline_prewarm;
mod residuals;
pub use residuals::*;

#[cfg(test)]
mod tests;

/// Concatenated live render_pipeline sources for residual `include_str` scans.
pub const RENDER_PIPELINE_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("forward_materials.rs"),
    include_str!("forward_render.rs"),
    include_str!("pipeline_collect.rs"),
    include_str!("pipeline_debug.rs"),
    include_str!("pipeline_execute.rs"),
    include_str!("pipeline_lifecycle.rs"),
    include_str!("pipeline_minimap.rs"),
    include_str!("pipeline_prewarm.rs"),
    include_str!("residuals.rs"),
);
