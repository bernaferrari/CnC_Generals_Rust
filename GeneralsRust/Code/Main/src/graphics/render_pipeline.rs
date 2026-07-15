use crate::assets::get_asset_manager;
use crate::fow_rendering::{FOWRenderingBridge, ObjectVisibility};
use crate::game_logic::{GameLogic, ObjectId as ObjectID};
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
fn terrain_to_main_axis_matrix() -> Mat4 {
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

fn transform_has_finite_components(transform: Mat4) -> bool {
    transform
        .to_cols_array()
        .into_iter()
        .all(|value| value.is_finite())
}

fn transform_is_reasonable_for_mesh(transform: Mat4) -> bool {
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
struct CullingPlane {
    normal: Vec3,
    distance: f32,
}

fn normalized_plane(plane: Vec4) -> CullingPlane {
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

fn extract_frustum_planes(view_proj: &Mat4) -> [CullingPlane; 6] {
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

fn world_sphere_in_expanded_frustum(
    planes: &[CullingPlane; 6],
    world_position: Vec3,
    world_radius: f32,
    camera_position: Vec3,
) -> bool {
    // Conservative sphere culling to mirror C++ `Cull_Sphere` behavior.
    const PLANE_MARGIN: f32 = 18.0;
    const NEAR_BYPASS_DISTANCE_SQ: f32 = 150.0 * 150.0;

    let radius = world_radius.max(1.0);
    for plane in planes {
        let signed_distance = plane.normal.dot(world_position) + plane.distance;
        if signed_distance < -(radius + PLANE_MARGIN) {
            return world_position.distance_squared(camera_position) <= NEAR_BYPASS_DISTANCE_SQ;
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

fn material_stage_texture(material: &W3DMaterial, stage: usize) -> Option<&str> {
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

const PROFILE_STEP_LOG_THRESHOLD: Duration = Duration::from_millis(20);

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
    cached_lighting: Option<CachedLighting>,
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
    debug_last_fow_filtered: usize,
    debug_last_model_missing: usize,
    debug_last_deferred_model_loads: usize,
    debug_last_deferred_model_load_budget: usize,
    debug_last_model_budget_skips: usize,
    debug_last_zero_mesh_models: usize,
    debug_last_missing_model_samples: Vec<String>,
    debug_warned_bad_mesh_transforms: HashSet<String>,
    model_cull_bounds_cache: HashMap<String, (Vec3, f32)>,
    animation_states: HashMap<u32, ObjectAnimationState>,
    last_frame_time: f32,
    /// When set, collect_render_items prefers presentation-owned transforms/model keys.
    presentation_frame: Option<crate::presentation_frame::PresentationFrame>,
}

const DEFAULT_SKYBOX_TEXTURES: [&str; 5] = [
    "TSMorningN.tga",
    "TSMorningE.tga",
    "TSMorningS.tga",
    "TSMorningW.tga",
    "TSMorningT.tga",
];

struct ObjectAnimationState {
    animation_index: usize,
    current_frame: f32,
    frame_rate: f32,
    num_frames: u32,
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
}

enum RenderModelLoadResult {
    Ready(Arc<W3DModel>),
    SkippedByBudget,
    Failed,
}

impl RenderPipeline {
    fn missing_model_debug_cubes_enabled_from(value: Option<&std::ffi::OsStr>) -> bool {
        value
            .and_then(|value| value.to_str())
            .map(|value| {
                matches!(
                    value.to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false)
    }

    fn missing_model_debug_cubes_enabled() -> bool {
        Self::missing_model_debug_cubes_enabled_from(
            std::env::var_os("GENERALS_RENDER_MISSING_MODEL_CUBES").as_deref(),
        )
    }

    fn should_prewarm_startup_map_template(
        asset_manager: &crate::assets::AssetManager,
        template: &str,
    ) -> bool {
        let template = template.trim();
        if template.is_empty() {
            return false;
        }

        if let Some(definition) = asset_manager.get_object_definition(template) {
            return definition.model_name.is_some();
        }

        if asset_manager.get_model_for_object(template).is_some() {
            return true;
        }

        let lower = template.to_ascii_lowercase();
        if lower.starts_with("amb_")
            || lower.starts_with("ambient")
            || lower.starts_with("cin_")
            || lower.starts_with("gc_")
            || lower.starts_with("scorch")
        {
            return false;
        }

        false
    }

    pub fn debug_render_item_count(&self) -> usize {
        self.render_items.len()
    }

    pub fn debug_last_alive_objects(&self) -> usize {
        self.debug_last_alive_objects
    }

    pub fn debug_last_fow_filtered(&self) -> usize {
        self.debug_last_fow_filtered
    }

    pub fn debug_last_model_missing(&self) -> usize {
        self.debug_last_model_missing
    }

    pub fn debug_last_deferred_model_loads(&self) -> usize {
        self.debug_last_deferred_model_loads
    }

    pub fn debug_last_deferred_model_load_budget(&self) -> usize {
        self.debug_last_deferred_model_load_budget
    }

    pub fn debug_last_model_budget_skips(&self) -> usize {
        self.debug_last_model_budget_skips
    }

    pub fn debug_last_zero_mesh_models(&self) -> usize {
        self.debug_last_zero_mesh_models
    }

    pub fn debug_last_missing_model_samples(&self) -> &[String] {
        &self.debug_last_missing_model_samples
    }

    pub fn debug_render_pass_counts(&self) -> (usize, usize, usize, usize, usize) {
        let mut shadow = 0usize;
        let mut forward_opaque = 0usize;
        let mut forward_transparent = 0usize;
        let mut water = 0usize;
        let mut ui = 0usize;

        for item in &self.render_items {
            match item.render_pass {
                RenderPass::ShadowPass => shadow += 1,
                RenderPass::ForwardOpaque => forward_opaque += 1,
                RenderPass::ForwardTransparent => forward_transparent += 1,
                RenderPass::WaterPass => water += 1,
                RenderPass::UIPass => ui += 1,
            }
        }

        (shadow, forward_opaque, forward_transparent, water, ui)
    }

    pub fn debug_render_item_breakdown_for_objects(&self, object_ids: &[ObjectID]) -> String {
        let focus_ids: HashSet<ObjectID> = object_ids.iter().copied().collect();
        if focus_ids.is_empty() {
            return "none".to_string();
        }

        let mut counts: HashMap<ObjectID, (usize, usize, usize, String)> = HashMap::new();
        for item in &self.render_items {
            if !focus_ids.contains(&item.object_id) {
                continue;
            }

            let entry = counts.entry(item.object_id).or_insert_with(|| {
                (
                    0,
                    0,
                    0,
                    format!("{}::{}", item.model_name, item.material.name),
                )
            });
            match item.render_pass {
                RenderPass::ForwardOpaque => entry.0 += 1,
                RenderPass::ForwardTransparent => entry.1 += 1,
                _ => entry.2 += 1,
            }
        }

        let mut ordered = object_ids.to_vec();
        ordered.sort_unstable();
        ordered
            .into_iter()
            .map(|id| {
                if let Some((opaque, transparent, other, sample)) = counts.get(&id) {
                    format!(
                        "{}:opaque={} transparent={} other={} sample={}",
                        id, opaque, transparent, other, sample
                    )
                } else {
                    format!("{}:opaque=0 transparent=0 other=0 sample=none", id)
                }
            })
            .collect::<Vec<_>>()
            .join(" | ")
    }

    pub fn prewarm_textures_blocking<I, S>(
        &mut self,
        texture_names: I,
    ) -> Result<TexturePrewarmStats>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.forward_pass.prewarm_textures_blocking(texture_names)
    }

    pub fn debug_forward_renderer_stats(&self) -> (u32, u32, u32) {
        let stats = self.forward_pass.renderer.stats();
        (
            stats.draw_calls,
            stats.meshes_rendered,
            stats.triangles_rendered,
        )
    }

    fn render_pass_for_material(material: &W3DMaterial) -> RenderPass {
        match material.blend_mode {
            crate::assets::models::BlendMode::Opaque => {
                if material.opacity < 0.999 {
                    RenderPass::ForwardTransparent
                } else {
                    RenderPass::ForwardOpaque
                }
            }
            crate::assets::models::BlendMode::Alpha
            | crate::assets::models::BlendMode::Additive
            | crate::assets::models::BlendMode::Modulate => RenderPass::ForwardTransparent,
        }
    }

    fn compare_render_items(a: &RenderItem, b: &RenderItem) -> std::cmp::Ordering {
        let pass_cmp = (a.render_pass as u32).cmp(&(b.render_pass as u32));
        if pass_cmp != std::cmp::Ordering::Equal {
            return pass_cmp;
        }

        if a.render_pass == RenderPass::ForwardTransparent {
            let distance_cmp = b
                .distance
                .partial_cmp(&a.distance)
                .unwrap_or(std::cmp::Ordering::Equal);
            if distance_cmp != std::cmp::Ordering::Equal {
                return distance_cmp;
            }
            let material_cmp = a.material_key.cmp(&b.material_key);
            if material_cmp != std::cmp::Ordering::Equal {
                return material_cmp;
            }
            return a
                .object_id
                .cmp(&b.object_id)
                .then_with(|| a.model_name.cmp(&b.model_name))
                .then_with(|| a.mesh_index.cmp(&b.mesh_index));
        }

        let material_cmp = a.material_key.cmp(&b.material_key);
        if material_cmp != std::cmp::Ordering::Equal {
            return material_cmp;
        }

        a.distance
            .partial_cmp(&b.distance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.object_id.cmp(&b.object_id))
            .then_with(|| a.model_name.cmp(&b.model_name))
            .then_with(|| a.mesh_index.cmp(&b.mesh_index))
    }

    fn paint_minimap_circle(
        texture: &mut [u8],
        width: u32,
        height: u32,
        center_x: i32,
        center_y: i32,
        radius: i32,
        tint_rgb: [u8; 3],
        blend: f32,
    ) {
        if radius <= 0 {
            return;
        }

        let blend = blend.clamp(0.0, 1.0);
        let px_width = width as i32;
        let px_height = height as i32;
        let radius_sq = radius * radius;

        for oy in -radius..=radius {
            for ox in -radius..=radius {
                if ox * ox + oy * oy > radius_sq {
                    continue;
                }

                let x = center_x + ox;
                let y = center_y + oy;
                if x < 0 || y < 0 || x >= px_width || y >= px_height {
                    continue;
                }

                let base = ((y as u32 * width + x as u32) * 4) as usize;
                texture[base] = (texture[base] as f32 * (1.0 - blend) + tint_rgb[0] as f32 * blend)
                    .clamp(0.0, 255.0) as u8;
                texture[base + 1] = (texture[base + 1] as f32 * (1.0 - blend)
                    + tint_rgb[1] as f32 * blend)
                    .clamp(0.0, 255.0) as u8;
                texture[base + 2] = (texture[base + 2] as f32 * (1.0 - blend)
                    + tint_rgb[2] as f32 * blend)
                    .clamp(0.0, 255.0) as u8;
                texture[base + 3] = 255;
            }
        }
    }

    /// Initialize render pipeline - equivalent to C++ RenderPipeline::Initialize()
    pub fn initialize(graphics_system: &GraphicsSystem) -> Result<Self> {
        info!("Initializing RenderPipeline (C++ SAGE equivalent)");

        // Initialize forward pass
        let forward_pass = ForwardPass::initialize()?;
        let (ambient_light, sun_color, sun_direction) = graphics_system.current_lighting();

        info!("RenderPipeline initialized successfully");

        Ok(Self {
            forward_pass,
            minimap_renderer: None, // Will be initialized when needed
            minimap_base_needs_refresh: false,
            heightmap_path_hint: None,
            pending_heightmap_hint_load: false,
            skybox_textures_hint: None,
            skybox_enabled: true,
            heightmap_world_size: None,
            cached_lighting: Some(CachedLighting {
                sun_direction: Some(sun_direction),
                sun_color: Some(sun_color),
                ambient_color: Some(ambient_light),
                fog_color: None,
                fog_range: None,
            }),
            last_startup_model_prewarm_signature: None,
            render_items: Vec::new(),
            frame_number: 0,
            current_pass: None,
            current_player_id: 0, // Default to player 0
            missing_ini_objects: HashSet::new(),
            debug_last_alive_objects: 0,
            debug_last_live_unit_identity_reads: 0,
            debug_last_fow_filtered: 0,
            debug_last_model_missing: 0,
            debug_last_deferred_model_loads: 0,
            debug_last_deferred_model_load_budget: 0,
            debug_last_model_budget_skips: 0,
            debug_last_zero_mesh_models: 0,
            debug_last_missing_model_samples: Vec::new(),
            debug_warned_bad_mesh_transforms: HashSet::new(),
            model_cull_bounds_cache: HashMap::new(),
            animation_states: HashMap::new(),
            last_frame_time: 0.0,
            presentation_frame: None,
        })
    }

    /// Provide full presentation snapshot for the next collect_render_items pass.
    pub fn set_presentation_frame(
        &mut self,
        frame: Option<crate::presentation_frame::PresentationFrame>,
    ) {
        self.presentation_frame = frame;
    }

    #[inline]
    pub fn presentation_frame(&self) -> Option<&crate::presentation_frame::PresentationFrame> {
        self.presentation_frame.as_ref()
    }

    /// Live GameLogic identity reads during last unit mesh collect (0 when presentation owns pass).
    pub fn last_live_unit_identity_reads(&self) -> usize {
        self.debug_last_live_unit_identity_reads
    }

    /// Pure unit-identity + FOW collection for the main mesh pass (no GameLogic borrow).
    ///
    /// Production `collect_render_items` uses this when a presentation frame is set.
    /// W3D mesh asset load remains outside this helper.
    pub fn collect_unit_render_inputs_from_presentation(
        frame: &crate::presentation_frame::PresentationFrame,
    ) -> Vec<crate::presentation_frame::UnitRenderInput> {
        frame.unit_render_inputs()
    }

    /// Backward-compatible: store IDs-only by building a minimal frame is not needed;
    /// prefer set_presentation_frame. Kept as thin alias for call sites.
    pub fn set_presentation_object_ids(&mut self, ids: Option<Vec<ObjectID>>) {
        if ids.is_none() {
            self.presentation_frame = None;
        }
        // IDs-only path no longer used; clear frame when None.
    }

    /// Execute complete rendering pipeline - equivalent to C++ RenderPipeline::Execute()
    pub fn execute(
        &mut self,
        graphics_system: &mut GraphicsSystem,
        // Live residual only when presentation is absent. Prefer None after
        // set_presentation_frame(Some(_)) for the immutable snapshot path.
        game_logic: Option<&GameLogic>,
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
        camera_position: Vec3,
        time: f32,
        allow_sync_model_loads: bool,
        deferred_startup_model_load_budget: usize,
        skip_world_scene: bool,
    ) -> Result<()> {
        let execute_started = std::time::Instant::now();
        trace!("RenderPipeline::execute frame {}", self.frame_number + 1);
        if (self.frame_number + 1).is_multiple_of(300) {
            debug!(
                "RenderPipeline frame {} - {} objects queued",
                self.frame_number + 1,
                self.render_items.len()
            );
        }

        self.frame_number += 1;
        if self.frame_number <= 5 {
            info!(
                "RenderPipeline::execute frame {} start (skip_world_scene={})",
                self.frame_number, skip_world_scene
            );
        }
        graphics_system.begin_frame();

        let delta_time = time - self.last_frame_time;
        self.last_frame_time = time;

        // Update global uniforms
        graphics_system.update_global_uniforms(
            view_matrix,
            projection_matrix,
            camera_position,
            time,
        );
        // Removed excessive logging

        // Clear render items from previous frame
        self.render_items.clear();

        let render_world_scene = !skip_world_scene;

        let mut collect_elapsed = std::time::Duration::ZERO;
        let mut sort_elapsed = std::time::Duration::ZERO;
        let mut terrain_elapsed = std::time::Duration::ZERO;
        if render_world_scene {
            self.sync_lighting_from_map_metadata(game_logic);
            if allow_sync_model_loads {
                if self.frame_number <= 5 {
                    info!(
                        "RenderPipeline::execute frame {} prewarm_start",
                        self.frame_number
                    );
                }
                self.prewarm_startup_models(graphics_system, game_logic, allow_sync_model_loads);
                if self.frame_number <= 5 {
                    info!(
                        "RenderPipeline::execute frame {} prewarm_done",
                        self.frame_number
                    );
                }
            }

            // Shell/menu startup needs to make visible progress without stalling first paint.
            let mut deferred_model_load_budget = if allow_sync_model_loads {
                usize::MAX
            } else {
                deferred_startup_model_load_budget
            };
            let initial_deferred_model_load_budget = if allow_sync_model_loads {
                0
            } else {
                deferred_model_load_budget
            };
            self.debug_last_model_budget_skips = 0;
            self.debug_last_zero_mesh_models = 0;
            self.debug_last_missing_model_samples.clear();
            self.debug_warned_bad_mesh_transforms.clear();

            // Collect render items from game objects - equivalent to C++ RenderPipeline::CollectRenderItems()
            let collect_started = std::time::Instant::now();
            if self.frame_number <= 5 {
                info!(
                    "RenderPipeline::execute frame {} collect_start (items={})",
                    self.frame_number,
                    self.render_items.len()
                );
            }
            self.collect_render_items(
                graphics_system,
                game_logic,
                view_matrix,
                projection_matrix,
                camera_position,
                allow_sync_model_loads,
                &mut deferred_model_load_budget,
                delta_time,
            )?;
            collect_elapsed = collect_started.elapsed();
            if self.frame_number <= 5 {
                info!(
                    "RenderPipeline::execute frame {} collect_done ({} items, {:?})",
                    self.frame_number,
                    self.render_items.len(),
                    collect_elapsed
                );
            }
            self.debug_last_deferred_model_load_budget = initial_deferred_model_load_budget;
            self.debug_last_deferred_model_loads = if allow_sync_model_loads {
                0
            } else {
                initial_deferred_model_load_budget.saturating_sub(deferred_model_load_budget)
            };

            #[cfg(feature = "game_client")]
            {
                self.drain_render_bridge_submissions(
                    graphics_system,
                    camera_position,
                    &mut deferred_model_load_budget,
                );
            }

            // Sort render items for optimal rendering - equivalent to C++ RenderPipeline::SortRenderItems()
            let sort_started = std::time::Instant::now();
            self.sort_render_items();
            sort_elapsed = sort_started.elapsed();
            // Removed excessive logging

            static LOGGED_STARTUP_RENDER_ITEM_SUMMARY: AtomicBool = AtomicBool::new(false);
            if !self.render_items.is_empty()
                && !LOGGED_STARTUP_RENDER_ITEM_SUMMARY.swap(true, Ordering::Relaxed)
            {
                let sample_items: Vec<String> = self
                    .render_items
                    .iter()
                    .take(12)
                    .map(|item| format!("{}#{}", item.model_name, item.mesh_index))
                    .collect();
                info!(
                    "Startup render summary: render_items={} sample_models={:?}",
                    self.render_items.len(),
                    sample_items
                );
            }
        } else {
            self.debug_last_deferred_model_load_budget = 0;
            self.debug_last_deferred_model_loads = 0;
            self.debug_last_model_budget_skips = 0;
            self.debug_last_zero_mesh_models = 0;
            self.debug_last_missing_model_samples.clear();
            self.debug_last_alive_objects = 0;
            self.debug_last_fow_filtered = 0;
            self.debug_last_model_missing = 0;
        }

        let shell_scene = self
            .presentation_frame
            .as_ref()
            .map(|p| p.fow_shell_bypass)
            .unwrap_or_else(|| game_logic.map(|g| g.isInShellGame()).unwrap_or(false));
        if render_world_scene && !shell_scene {
            // Presentation-owned bounds/heights when frame is set; live GameLogic
            // is only a boot fallback (execute already passes None with snapshot).
            if let Err(e) = self.refresh_minimap_terrain_base(game_logic) {
                error!("Failed to refresh minimap terrain base: {}", e);
            }

            // Update minimap FOW texture before rendering UI
            if let Err(e) = self.update_minimap_fow_texture() {
                error!("Failed to update minimap FOW texture: {}", e);
            }
        }

        #[cfg(feature = "game_client")]
        if render_world_scene {
            let terrain_started = std::time::Instant::now();
            self.update_and_enqueue_terrain_pass(view_matrix, projection_matrix)?;
            terrain_elapsed = terrain_started.elapsed();
        }

        let forward_started = std::time::Instant::now();
        if self.frame_number <= 5 {
            info!(
                "RenderPipeline::execute frame {} forward_pass_start (items={})",
                self.frame_number,
                self.render_items.len()
            );
        }
        self.forward_pass.render(
            graphics_system,
            &self.render_items,
            view_matrix,
            projection_matrix,
            camera_position,
            self.cached_lighting.as_ref(),
        )?;
        let forward_elapsed = forward_started.elapsed();
        if self.frame_number <= 5 {
            info!(
                "RenderPipeline::execute frame {} forward_pass_done ({:?})",
                self.frame_number, forward_elapsed
            );
        }

        graphics_system.end_frame();
        if render_world_scene && !shell_scene {
            self.maybe_load_heightmap_hint_after_first_present(graphics_system, game_logic);
        }

        // Removed excessive logging
        let execute_elapsed = execute_started.elapsed();
        if execute_elapsed >= std::time::Duration::from_millis(200) {
            warn!(
                "RenderPipeline breakdown: total={:?} collect={:?} sort={:?} terrain={:?} forward={:?} render_world_scene={} render_items={} model_missing={} deferred_loads={}/{}",
                execute_elapsed,
                collect_elapsed,
                sort_elapsed,
                terrain_elapsed,
                forward_elapsed,
                render_world_scene,
                self.render_items.len(),
                self.debug_last_model_missing,
                self.debug_last_deferred_model_loads,
                self.debug_last_deferred_model_load_budget
            );
        }
        Ok(())
    }

    fn sync_lighting_from_map_metadata(&mut self, game_logic: Option<&GameLogic>) {
        // Prefer frozen presentation env when available (no live map-settings re-read).
        let derived = if let Some(pres) = self.presentation_frame.as_ref() {
            let env = &pres.world_env;
            if !env.has_map_metadata
                && env.sun_direction.is_none()
                && env.sun_color.is_none()
                && env.ambient_color.is_none()
            {
                return;
            }
            CachedLighting {
                sun_direction: env.sun_direction,
                sun_color: env.sun_color,
                ambient_color: env.ambient_color,
                fog_color: env.fog_color,
                fog_range: env.fog_range(),
            }
        } else {
            let Some(gl) = game_logic else {
                return;
            };
            let Some(meta) = gl.last_parsed_map_settings() else {
                return;
            };
            CachedLighting {
                sun_direction: meta.sun_direction,
                sun_color: meta.sun_color.or(meta.sky_color),
                ambient_color: meta.ambient_color.or(meta.fog_color).or(meta.sky_color),
                fog_color: meta.fog_color.or(meta.sky_color).or(meta.sun_color),
                fog_range: meta.fog_start.zip(meta.fog_end),
            }
        };

        match &mut self.cached_lighting {
            Some(existing) => {
                if existing.sun_direction.is_none() {
                    existing.sun_direction = derived.sun_direction;
                }
                if existing.sun_color.is_none() {
                    existing.sun_color = derived.sun_color;
                }
                if existing.ambient_color.is_none() {
                    existing.ambient_color = derived.ambient_color;
                }
                if existing.fog_color.is_none() {
                    existing.fog_color = derived.fog_color;
                }
                if existing.fog_range.is_none() {
                    existing.fog_range = derived.fog_range;
                }
            }
            None => {
                self.cached_lighting = Some(derived);
            }
        }
    }

    fn prewarm_startup_models(
        &mut self,
        graphics_system: &mut GraphicsSystem,
        game_logic: Option<&GameLogic>,
        allow_sync_model_loads: bool,
    ) {
        let (map_name, signature) = if let Some(pres) = self.presentation_frame.as_ref() {
            (
                pres.world_env.map_name.clone(),
                pres.world_env.prewarm_signature(pres.fow_shell_bypass),
            )
        } else if let Some(gl) = game_logic {
            let map_name = gl.get_current_map_name().trim().to_string();
            let metadata = gl.last_parsed_map_settings();
            let signature = format!(
                "{}|meta:{}|objects:{}|heightmap:{}|shell:{}",
                map_name,
                metadata.is_some(),
                metadata.as_ref().map(|m| m.objects.len()).unwrap_or(0),
                metadata
                    .as_ref()
                    .and_then(|m| m.heightmap_path.as_ref())
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                gl.isInShellGame()
            );
            (map_name, signature)
        } else {
            return;
        };

        if self
            .last_startup_model_prewarm_signature
            .as_deref()
            .is_some_and(|prev| prev == signature)
        {
            return;
        }

        // Prefer frozen prewarm names from PresentationWorldEnv (capped list).
        // When a presentation frame is installed, never re-query live map metadata
        // (empty prewarm list is fail-closed: skip names rather than dual-read logic).
        let template_names: Vec<String> = if self.presentation_frame.is_some() {
            self.presentation_frame
                .as_ref()
                .map(|p| p.world_env.prewarm_template_names.clone())
                .unwrap_or_default()
        } else {
            game_logic
                .and_then(|g| g.last_parsed_map_settings())
                .map(|m| {
                    m.objects
                        .iter()
                        .map(|o| o.template.clone())
                        .filter(|s| !s.trim().is_empty())
                        .take(256)
                        .collect()
                })
                .unwrap_or_default()
        };

        let mut candidates: Vec<String> = Vec::new();
        let mut seen = HashSet::new();

        if !template_names.is_empty() {
            if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
                if let Ok(asset_manager) = asset_manager_arc.lock() {
                    for template_raw in &template_names {
                        let template = template_raw.trim();
                        if template.is_empty() {
                            continue;
                        }
                        if !Self::should_prewarm_startup_map_template(&asset_manager, template) {
                            continue;
                        }
                        let key = template.to_ascii_lowercase();
                        if seen.insert(key) {
                            candidates.push(template.to_string());
                        }
                    }
                } else {
                    warn!("Startup model prewarm skipped: asset manager mutex poisoned");
                }
            }
        }

        if candidates.is_empty() {
            if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
                if let Ok(asset_manager) = asset_manager_arc.lock() {
                    candidates.extend(
                        asset_manager
                            .get_common_cnc_units()
                            .into_iter()
                            .map(str::to_string),
                    );
                }
            }
        } else if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
            if let Ok(asset_manager) = asset_manager_arc.lock() {
                for unit in asset_manager.get_common_cnc_units() {
                    if candidates.len() >= if allow_sync_model_loads { 48 } else { 12 } {
                        break;
                    }
                    let key = unit.to_ascii_lowercase();
                    if seen.insert(key) {
                        candidates.push(unit.to_string());
                    }
                }
            }
        }

        let prewarm_limit = if allow_sync_model_loads { 48 } else { 12 };
        candidates.truncate(prewarm_limit);
        if candidates.is_empty() {
            self.last_startup_model_prewarm_signature = Some(signature);
            return;
        }

        let mut cached_to_graphics = 0usize;
        let mut stats = ModelPrewarmStats::default();

        if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
            match asset_manager_arc.lock() {
                Ok(mut asset_manager) => {
                    stats = asset_manager.prewarm_object_models_blocking(candidates.iter());
                    for name in &candidates {
                        if let Some(model) = asset_manager.get_cached_model(name) {
                            let resolved_name = asset_manager
                                .get_model_for_object(name)
                                .unwrap_or_else(|| name.clone());
                            graphics_system.cache_model(resolved_name.clone(), model.clone());
                            if resolved_name != *name {
                                graphics_system.cache_model(name.clone(), model);
                            }
                            cached_to_graphics += 1;
                        }
                    }
                }
                Err(_) => {
                    warn!("Startup model prewarm skipped: asset manager mutex poisoned");
                }
            }
        }

        info!(
            "Startup model prewarm: map='{}' candidates={} requested={} cache_hits={} resolved={} missing={} graphics_cached={}",
            if map_name.is_empty() { "<unknown>" } else { &map_name },
            candidates.len(),
            stats.requested,
            stats.cache_hits,
            stats.resolved,
            stats.missing,
            cached_to_graphics
        );

        self.last_startup_model_prewarm_signature = Some(signature);
    }

    fn maybe_load_heightmap_hint_after_first_present(
        &mut self,
        graphics_system: &GraphicsSystem,
        game_logic: Option<&GameLogic>,
    ) {
        if !self.pending_heightmap_hint_load || self.frame_number <= 1 {
            return;
        }

        let world_bounds = self
            .presentation_frame
            .as_ref()
            .map(|p| p.world_env.world_bounds_vec3())
            .or_else(|| game_logic.map(|g| g.world_bounds()));
        let Some(world_bounds) = world_bounds else {
            return;
        };
        match self.load_heightmap_from_hint(
            &graphics_system.device_arc(),
            &graphics_system.queue_arc(),
            Some(world_bounds),
        ) {
            Ok(()) => {
                self.pending_heightmap_hint_load = false;
            }
            Err(err) => {
                warn!("Deferred heightmap hint load failed: {}", err);
                self.pending_heightmap_hint_load = false;
            }
        }
    }

    #[cfg(feature = "game_client")]
    fn update_and_enqueue_terrain_pass(
        &mut self,
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
    ) -> Result<()> {
        static LOGGED_ZERO_TERRAIN_CHUNKS: AtomicBool = AtomicBool::new(false);
        static LOGGED_NONZERO_TERRAIN_CHUNKS: AtomicBool = AtomicBool::new(false);

        let terrain_pass_started = Instant::now();
        if let Ok(mut guard) = game_client::terrain::terrain_visual::get_terrain_visual() {
            if let Some(terrain_visual) = guard.as_mut() {
                let client_view_matrix = Mat4::from_cols_array_2d(&view_matrix.to_cols_array_2d());
                let client_projection_matrix =
                    Mat4::from_cols_array_2d(&projection_matrix.to_cols_array_2d());
                let terrain_render_started = Instant::now();
                terrain_visual
                    .render(&client_view_matrix, &client_projection_matrix)
                    .map_err(|e| {
                        anyhow::anyhow!("terrain visual render state update failed: {}", e)
                    })?;
                let terrain_render_elapsed = terrain_render_started.elapsed();
                let terrain_update_started = Instant::now();
                terrain_visual
                    .update()
                    .map_err(|e| anyhow::anyhow!("terrain visual update failed: {}", e))?;
                let terrain_update_elapsed = terrain_update_started.elapsed();

                let chunk_count = terrain_visual.chunk_draw_count();
                let terrain_total_elapsed = terrain_pass_started.elapsed();
                if terrain_total_elapsed >= PROFILE_STEP_LOG_THRESHOLD
                    || terrain_render_elapsed >= PROFILE_STEP_LOG_THRESHOLD
                    || terrain_update_elapsed >= PROFILE_STEP_LOG_THRESHOLD
                {
                    debug!(
                        "TerrainVisual breakdown: total={:?} render={:?} update={:?} visible_chunks={} total_chunks={} pending_visible_chunks={}",
                        terrain_total_elapsed,
                        terrain_render_elapsed,
                        terrain_update_elapsed,
                        terrain_visual.debug_visible_chunk_count(),
                        terrain_visual.debug_total_chunk_count(),
                        terrain_visual.debug_pending_visible_chunk_count()
                    );
                }
                if chunk_count == 0 {
                    if !LOGGED_ZERO_TERRAIN_CHUNKS.swap(true, Ordering::Relaxed) {
                        warn!("Terrain visual updated but no visible chunks were selected for drawing");
                    }
                } else if !LOGGED_NONZERO_TERRAIN_CHUNKS.swap(true, Ordering::Relaxed) {
                    info!(
                        "Terrain visual selected {} visible chunks for drawing",
                        chunk_count
                    );
                }
            } else {
                return Ok(());
            }
        } else {
            return Ok(());
        }

        let _view = *view_matrix;
        let _projection = *projection_matrix;
        let clear_color = self.terrain_clear_color();
        self.forward_pass.enqueue_pre_scene_callback(move |frame| {
            let terrain_draw_started = Instant::now();
            let depth_view = frame.depth_view_arc();
            let color_view = frame.color_view_arc();
            let encoder = frame.encoder();
            let terrain_visual_guard =
                game_client::terrain::terrain_visual::get_terrain_visual().ok();
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main terrain pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view.as_ref(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: depth_view.as_ref().map(|depth| {
                    wgpu::RenderPassDepthStencilAttachment {
                        view: depth.as_ref(),
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some(terrain_guard) = terrain_visual_guard.as_ref() {
                if let Some(terrain_visual) = terrain_guard.as_ref() {
                    terrain_visual.record_chunk_draws(&mut render_pass);
                }
            }
            drop(render_pass);

            let terrain_draw_elapsed = terrain_draw_started.elapsed();
            if terrain_draw_elapsed >= PROFILE_STEP_LOG_THRESHOLD {
                debug!(
                    "TerrainVisual chunk draw recording took {:?}",
                    terrain_draw_elapsed
                );
            }

            Ok(())
        });

        Ok(())
    }

    /// Collect render items from game objects - equivalent to C++ RenderPipeline::CollectRenderItems()
    /// Integrates FOW visibility filtering.
    ///
    /// # Presentation boundary (host path)
    /// When `presentation_frame` is set, the **main unit mesh pass** iterates
    /// `PresentationFrame::unit_render_inputs()` only:
    /// position / orientation / team / model_key / selected / selection_radius /
    /// aliveness / engine_bridged / **fow_visibility** / shell FOW bypass —
    /// all snapshot-owned.
    ///
    /// Remaining residuals (not unit identity; see mesh_asset_resolve residual notes):
    /// - live fallback when no presentation frame is set (boot/loading)
    /// - mesh asset resolve: GraphicsSystem cache + AssetManager + filesystem residual
    ///   (`assets::mesh_asset_resolve`); deferred load budget still incremental
    /// - terrain / cell-grid FOW overlay (not unit mesh identity)
    ///
    /// Do **not** re-read live position/orientation/health/team/selected/model_key/FOW
    /// when presentation owns those fields.
    fn collect_render_items(
        &mut self,
        graphics_system: &mut GraphicsSystem,
        game_logic: Option<&GameLogic>,
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
        camera_position: Vec3,
        allow_sync_model_loads: bool,
        deferred_model_load_budget: &mut usize,
        delta_time: f32,
    ) -> Result<()> {
        let collect_started = Instant::now();
        let object_ids_started = Instant::now();
        // Snapshot ownership: when presentation is present, drive the main unit
        // mesh pass from unit_render_inputs (no live object identity / FOW re-read).
        // Keep frame installed for post-collect execute residual (minimap/shell/heightmap).
        let presentation = self.presentation_frame.clone();
        let presentation_unit_pass = presentation.is_some();
        // Reset live-identity residual each collect; presentation path must stay at 0.
        self.debug_last_live_unit_identity_reads = 0;
        // Shell FOW bypass from snapshot when available (no live GameLogic re-read).
        let bypass_fow = presentation
            .as_ref()
            .map(|p| p.fow_shell_bypass)
            .unwrap_or_else(|| game_logic.map(|g| g.isInShellGame()).unwrap_or(false));

        // Snapshot-owned unit inputs for the main mesh pass (empty when no frame).
        let mut unit_inputs: Vec<crate::presentation_frame::UnitRenderInput> =
            if let Some(ref pres) = presentation {
                pres.unit_render_inputs()
            } else {
                Vec::new()
            };

        // Live fallback identity list when presentation is absent (boot/loading).
        let mut live_object_ids: Vec<ObjectID> = if presentation_unit_pass {
            Vec::new()
        } else if let Some(gl) = game_logic {
            gl.get_objects().keys().copied().collect()
        } else {
            Vec::new()
        };

        // Live FOW batch needs IDs only when presentation is absent.
        let mut fow_ids: Vec<ObjectID> = if presentation_unit_pass {
            Vec::new()
        } else {
            live_object_ids.clone()
        };

        trace!(
            "collect_render_items processing {} units (presentation_unit_pass={})",
            if presentation_unit_pass {
                unit_inputs.len()
            } else {
                live_object_ids.len()
            },
            presentation_unit_pass
        );

        if allow_sync_model_loads {
            if presentation_unit_pass {
                unit_inputs.sort_by_key(|u| u.id.0);
            } else {
                live_object_ids.sort_unstable();
                fow_ids.sort_unstable();
            }
        } else if presentation_unit_pass {
            // Distance sort from snapshot positions only — no live transform re-read.
            unit_inputs.sort_by(|a, b| {
                let da = a.position.distance_squared(camera_position);
                let db = b.position.distance_squared(camera_position);
                da.partial_cmp(&db)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.id.cmp(&b.id))
            });
        } else {
            let mut object_ids_with_distance: Vec<(ObjectID, f32)> = live_object_ids
                .iter()
                .copied()
                .map(|object_id| {
                    let distance_squared = game_logic
                        .and_then(|g| g.get_objects().get(&object_id))
                        .map(|obj| {
                            gameplay_to_render_transform(obj.get_transform_matrix())
                                .w_axis
                                .truncate()
                                .distance_squared(camera_position)
                        })
                        .unwrap_or(f32::INFINITY);
                    (object_id, distance_squared)
                })
                .collect();
            object_ids_with_distance.sort_by(|(a_id, a_distance), (b_id, b_distance)| {
                a_distance
                    .partial_cmp(b_distance)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a_id.cmp(b_id))
            });
            live_object_ids = object_ids_with_distance
                .into_iter()
                .map(|(object_id, _)| object_id)
                .collect();
            fow_ids = live_object_ids.clone();
        }
        let object_ids_elapsed = object_ids_started.elapsed();

        let mut alive_objects = 0usize;
        let mut fow_filtered = 0usize;
        let mut model_missing = 0usize;
        // Shell maps / presentation path: no live shroud batch.
        // Live fallback still queries FOW bridge once per collect.
        let visibility_started = Instant::now();
        let visibilities = if bypass_fow || presentation_unit_pass {
            std::collections::HashMap::new()
        } else {
            self.get_batch_fow_visibility(&fow_ids)
        };
        let visibility_elapsed = visibility_started.elapsed();

        let view_proj = *projection_matrix * *view_matrix;
        let frustum_planes = extract_frustum_planes(&view_proj);

        let mut render_model_load_elapsed = Duration::ZERO;
        let mut render_item_build_elapsed = Duration::ZERO;

        // --- Main unit mesh pass: presentation-owned inputs when available ---
        // Live object identity is only resolved when presentation_unit_pass is false.
        enum UnitPassSource {
            Presentation(crate::presentation_frame::UnitRenderInput),
            Live(ObjectID),
        }
        let pass_sources: Vec<UnitPassSource> = if presentation_unit_pass {
            unit_inputs
                .into_iter()
                .map(UnitPassSource::Presentation)
                .collect()
        } else {
            live_object_ids
                .into_iter()
                .map(UnitPassSource::Live)
                .collect()
        };

        for source in pass_sources {
            // Resolve unit-identity + FOW without live re-read when presentation owns them.
            let (
                object_id,
                world_matrix,
                model_name_owned,
                template_name_owned,
                selection_radius,
                model_hint_owned,
                snapshot_fow,
            ) = match &source {
                UnitPassSource::Presentation(u) => {
                    // engine_bridged already filtered in unit_render_inputs; keep guard.
                    if u.engine_bridged {
                        continue;
                    }
                    let m = u.world_matrix();
                    (
                        u.id,
                        gameplay_to_render_transform(m),
                        u.model_key.clone(),
                        u.template_name.clone(),
                        u.selection_radius,
                        Some(u.model_key.clone()),
                        Some(u.fow_visibility),
                    )
                }
                UnitPassSource::Live(id) => {
                    self.debug_last_live_unit_identity_reads =
                        self.debug_last_live_unit_identity_reads.saturating_add(1);
                    let Some(gl) = game_logic else {
                        continue;
                    };
                    let Some(object) = gl.get_objects().get(id) else {
                        continue;
                    };
                    if !object.is_alive() {
                        continue;
                    }
                    // Live residual: engine_object_id skip when no presentation frame.
                    #[cfg(feature = "game_client")]
                    if object.engine_object_id.is_some() {
                        continue;
                    }
                    (
                        *id,
                        gameplay_to_render_transform(object.get_transform_matrix()),
                        object.get_template().get_model_name().to_string(),
                        object.template_name.clone(),
                        object.selection_radius.max(5.0),
                        object.get_template().model_name.clone(),
                        None,
                    )
                }
            };

            alive_objects += 1;

            // FOW never-explored skip: presentation path uses snapshot only.
            let fow_visibility = if let Some(snap_vis) = snapshot_fow {
                if !bypass_fow && !snap_vis.should_render() {
                    fow_filtered += 1;
                    trace!(
                        "Skipping object {} - never explored (presentation FOW) by player {}",
                        object_id,
                        self.current_player_id
                    );
                    continue;
                }
                if bypass_fow {
                    ObjectVisibility::FULLY_VISIBLE
                } else {
                    snap_vis
                }
            } else {
                if !bypass_fow && !self.should_render_object(object_id) {
                    fow_filtered += 1;
                    trace!(
                        "Skipping object {} - never explored by player {}",
                        object_id,
                        self.current_player_id
                    );
                    continue;
                }
                visibilities
                    .get(&object_id)
                    .copied()
                    .unwrap_or_else(ObjectVisibility::default)
            };

            let world_position = world_matrix.w_axis.truncate();
            let model_name = model_name_owned.as_str();
            let template_name_for_cull = template_name_owned.as_str();
            let (cull_center, cull_radius) = self.resolve_object_world_cull_sphere(
                graphics_system,
                model_name,
                template_name_for_cull,
                selection_radius,
                world_matrix,
            );
            if !world_sphere_in_expanded_frustum(
                &frustum_planes,
                cull_center,
                cull_radius,
                camera_position,
            ) {
                continue;
            }

            let model_hint = model_hint_owned.as_deref().or(Some(model_name));

            let model_load_started = Instant::now();
            let render_model_load_result = Self::ensure_render_model_loaded(
                graphics_system,
                template_name_for_cull,
                model_name,
                allow_sync_model_loads,
                deferred_model_load_budget,
            );
            render_model_load_elapsed += model_load_started.elapsed();

            let render_item_build_started = Instant::now();
            match render_model_load_result {
                RenderModelLoadResult::Ready(w3d_model) => {
                    if w3d_model.meshes.is_empty() {
                        self.debug_last_zero_mesh_models += 1;
                        // Fall through to fallback cube below (same as Failed path)
                    } else {
                        let visibility = fow_visibility;

                        let anim_frame = if !w3d_model.animations.is_empty()
                            && w3d_model.hierarchy.is_some()
                        {
                            let obj_key = object_id.0;
                            let state = self.animation_states.entry(obj_key).or_insert_with(|| {
                                let (num_frames, frame_rate) =
                                    w3d_model.animation_metadata(0).unwrap_or((1, 30));
                                ObjectAnimationState {
                                    animation_index: 0,
                                    current_frame: 0.0,
                                    frame_rate: frame_rate as f32,
                                    num_frames,
                                }
                            });
                            if delta_time > 0.0 && delta_time < 1.0 {
                                state.current_frame += delta_time * state.frame_rate;
                                if state.num_frames > 1
                                    && state.current_frame >= state.num_frames as f32
                                {
                                    state.current_frame %= (state.num_frames - 1) as f32;
                                }
                            }
                            state.current_frame
                        } else {
                            0.0
                        };

                        for (mesh_idx, mesh) in w3d_model.meshes.iter().enumerate() {
                            let mut material = mesh.material.clone();

                            if material.texture_name.is_none() {
                                if let Some(asset_manager_arc) = crate::assets::get_asset_manager()
                                {
                                    if let Ok(asset_manager) = asset_manager_arc.lock() {
                                        if let Some(obj_def) = asset_manager
                                            .resolve_object_definition(
                                                &template_name_owned,
                                                model_hint,
                                            )
                                        {
                                            if let Some(texture_from_ini) =
                                                obj_def.get_primary_texture()
                                            {
                                                material.texture_name =
                                                    Some(texture_from_ini.to_string());
                                                trace!(
                                                    "WW3D material fallback: object {} ('{}') -> texture {}",
                                                    object_id,
                                                    template_name_owned,
                                                    texture_from_ini
                                                );
                                            } else if self
                                                .missing_ini_objects
                                                .insert(format!("{}::texture", template_name_owned))
                                            {
                                                debug!(
                                                    "WW3D assets: INI definition for '{}' defines no textures",
                                                    template_name_owned
                                                );
                                            }
                                        } else if self
                                            .missing_ini_objects
                                            .insert(template_name_owned.clone())
                                        {
                                            debug!(
                                                "WW3D assets: no INI definition for '{}' (model hint: {:?})",
                                                template_name_owned,
                                                model_hint
                                            );
                                        }
                                    }
                                }
                            }

                            // Mesh local transforms coming from WW3D hierarchy/HLOD data are in
                            // source gameplay basis. If we axis-convert vertex payload at mesh build
                            // time, local transforms must be converted into the same render basis.
                            let mesh_local_transform = if mesh.vertices_in_render_space {
                                mesh.transform
                            } else {
                                let axis = gameplay_to_render_axis_matrix();
                                axis * mesh.transform * axis.inverse()
                            };
                            let mesh_local_transform = if transform_is_reasonable_for_mesh(
                                mesh_local_transform,
                            ) {
                                mesh_local_transform
                            } else {
                                let key = format!(
                                    "{}::{}::{}",
                                    template_name_owned, model_name, mesh.name
                                );
                                if self.debug_warned_bad_mesh_transforms.insert(key.clone()) {
                                    warn!(
                                        "Invalid mesh local transform for '{}': template='{}' model='{}' mesh='{}'; using identity transform",
                                        key, template_name_owned, model_name, mesh.name
                                    );
                                }
                                Mat4::IDENTITY
                            };
                            let mut render_item = RenderItem::new(
                                object_id,
                                model_name.to_string(),
                                mesh_idx,
                                world_position,
                                world_matrix,
                                &material,
                                Self::render_pass_for_material(&material),
                            );
                            render_item.set_mesh_local_transform(mesh_local_transform);
                            render_item.distance = world_position.distance(camera_position);
                            render_item.set_fow_visibility(visibility);
                            render_item.animation_frame = anim_frame;

                            self.render_items.push(render_item);
                        }

                        trace!(
                            "Object {} will render with FOW alpha={}, explored={}",
                            object_id,
                            visibility.visibility_alpha,
                            visibility.is_explored
                        );
                        render_item_build_elapsed += render_item_build_started.elapsed();
                        continue; // Skip the fallback path
                    }

                    if Self::missing_model_debug_cubes_enabled() {
                        if let Some(fallback_model) =
                            graphics_system.get_model_or_fallback("__fallback_cube__")
                        {
                            if !fallback_model.meshes.is_empty() {
                                let fallback_mesh = &fallback_model.meshes[0];
                                let mut render_item = RenderItem::new(
                                    object_id,
                                    "__fallback_cube__".to_string(),
                                    0,
                                    world_position,
                                    world_matrix,
                                    &fallback_mesh.material,
                                    RenderPass::ForwardOpaque,
                                );
                                render_item.distance = world_position.distance(camera_position);
                                render_item.set_fow_visibility(fow_visibility);

                                self.render_items.push(render_item);
                            }
                        }
                    }
                }
                RenderModelLoadResult::SkippedByBudget => {
                    self.debug_last_model_budget_skips += 1;
                    if self.debug_last_missing_model_samples.len() < 16 {
                        self.debug_last_missing_model_samples
                            .push(format!("{}:{} [budget]", template_name_owned, model_name));
                    }
                    model_missing += 1;
                }
                RenderModelLoadResult::Failed => {
                    if self.debug_last_missing_model_samples.len() < 16 {
                        // Prefer presentation/live-resolved model hint (no re-read of Object).
                        let explicit = model_hint_owned.as_deref().unwrap_or("");
                        self.debug_last_missing_model_samples.push(format!(
                            "{}:{} explicit_model={}",
                            template_name_owned,
                            model_name,
                            if explicit.is_empty() {
                                "<none>"
                            } else {
                                explicit
                            }
                        ));
                    }
                    model_missing += 1;

                    if Self::missing_model_debug_cubes_enabled() {
                        if let Some(fallback_model) =
                            graphics_system.get_model_or_fallback("__fallback_cube__")
                        {
                            if !fallback_model.meshes.is_empty() {
                                let fallback_mesh = &fallback_model.meshes[0];
                                let mut render_item = RenderItem::new(
                                    object_id,
                                    "__fallback_cube__".to_string(),
                                    0,
                                    world_position,
                                    world_matrix,
                                    &fallback_mesh.material,
                                    RenderPass::ForwardOpaque,
                                );
                                render_item.distance = world_position.distance(camera_position);
                                render_item.set_fow_visibility(fow_visibility);

                                self.render_items.push(render_item);
                            }
                        }
                    }
                }
            }
            render_item_build_elapsed += render_item_build_started.elapsed();
        }

        self.debug_last_alive_objects = alive_objects;
        self.debug_last_fow_filtered = fow_filtered;
        self.debug_last_model_missing = model_missing;
        debug!(
            "Collected {} render items for player {} (FOW filtering active)",
            self.render_items.len(),
            self.current_player_id
        );
        let collect_elapsed = collect_started.elapsed();
        if collect_elapsed >= PROFILE_STEP_LOG_THRESHOLD
            || object_ids_elapsed >= PROFILE_STEP_LOG_THRESHOLD
            || visibility_elapsed >= PROFILE_STEP_LOG_THRESHOLD
            || render_model_load_elapsed >= PROFILE_STEP_LOG_THRESHOLD
            || render_item_build_elapsed >= PROFILE_STEP_LOG_THRESHOLD
        {
            debug!(
                "Render collection breakdown: total={:?} ids={:?} visibility={:?} model_load={:?} item_build={:?} alive={} filtered={} missing={} items={}",
                collect_elapsed,
                object_ids_elapsed,
                visibility_elapsed,
                render_model_load_elapsed,
                render_item_build_elapsed,
                self.debug_last_alive_objects,
                self.debug_last_fow_filtered,
                self.debug_last_model_missing,
                self.render_items.len()
            );
        }

        Ok(())
    }

    fn resolve_object_world_cull_sphere(
        &mut self,
        graphics_system: &GraphicsSystem,
        model_name: &str,
        template_name: &str,
        selection_radius: f32,
        world_matrix: Mat4,
    ) -> (Vec3, f32) {
        let mut model_bounds = self.model_cull_bounds_cache.get(model_name).copied();
        if model_bounds.is_none() {
            let source = graphics_system
                .get_model(model_name)
                .or_else(|| graphics_system.get_model(template_name));
            if let Some(model) = source {
                model_bounds = Self::model_local_cull_bounds(model.as_ref());
                if let Some(bounds) = model_bounds {
                    self.model_cull_bounds_cache
                        .insert(model_name.to_string(), bounds);
                }
            }
        }

        let world_scale = world_matrix
            .x_axis
            .truncate()
            .length()
            .max(world_matrix.y_axis.truncate().length())
            .max(world_matrix.z_axis.truncate().length())
            .max(1.0);
        let fallback_radius = selection_radius.max(10.0);
        let fallback_center = world_matrix.w_axis.truncate();
        model_bounds
            .map(|(local_center, local_radius)| {
                let world_center = world_matrix.transform_point3(local_center);
                let world_radius = (local_radius * world_scale).max(fallback_radius);
                (world_center, world_radius)
            })
            .unwrap_or((fallback_center, fallback_radius))
    }

    fn model_local_cull_bounds(model: &crate::assets::W3DModel) -> Option<(Vec3, f32)> {
        let min = model.bounding_box_min;
        let max = model.bounding_box_max;
        if !min.is_finite() || !max.is_finite() {
            return None;
        }
        if min.x > max.x || min.y > max.y || min.z > max.z {
            return None;
        }
        let center = (min + max) * 0.5;
        let extents = (max - min) * 0.5;
        let radius = extents.length();
        if radius.is_finite() && radius > 0.0 {
            Some((center, radius))
        } else {
            None
        }
    }

    fn ensure_render_model_loaded(
        graphics_system: &mut GraphicsSystem,
        template_name: &str,
        model_name: &str,
        allow_sync_model_loads: bool,
        deferred_model_load_budget: &mut usize,
    ) -> RenderModelLoadResult {
        use crate::assets::mesh_asset_resolve::{
            remap_model_key_alias, resolve_mesh_for_model_key, MeshResolveResult,
            PLACEHOLDER_MODEL_KEY,
        };

        static STARTUP_MODEL_TRACE_COUNT: AtomicUsize = AtomicUsize::new(0);
        let trace_this_attempt = !allow_sync_model_loads
            && STARTUP_MODEL_TRACE_COUNT
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                    (count < 64).then_some(count + 1)
                })
                .is_ok();

        // Canonical key from presentation model_key / get_model_name (airanger → airanger_s).
        let resolved_key = remap_model_key_alias(model_name);

        if let Some(model) = graphics_system.get_model(&resolved_key).cloned() {
            if trace_this_attempt {
                info!(
                    "Startup model load: cache hit template='{}' model='{}'",
                    template_name, resolved_key
                );
            }
            return RenderModelLoadResult::Ready(model);
        }
        if resolved_key != model_name {
            if let Some(model) = graphics_system.get_model(model_name).cloned() {
                graphics_system.cache_model(resolved_key.clone(), model.as_ref().clone());
                return RenderModelLoadResult::Ready(model);
            }
        }
        if let Some(model) = graphics_system.get_model(template_name).cloned() {
            if resolved_key != template_name {
                graphics_system.cache_model(resolved_key.clone(), model.as_ref().clone());
            }
            if trace_this_attempt {
                info!(
                    "Startup model load: cache hit template='{}' model='{}' (aliased from template cache)",
                    template_name, resolved_key
                );
            }
            return RenderModelLoadResult::Ready(model);
        }

        if !allow_sync_model_loads && *deferred_model_load_budget == 0 {
            if trace_this_attempt {
                info!(
                    "Startup model load: skipped by budget template='{}' model='{}'",
                    template_name, resolved_key
                );
            }
            return RenderModelLoadResult::SkippedByBudget;
        }

        if !allow_sync_model_loads {
            *deferred_model_load_budget -= 1;
        }

        let mut requested_model_name = resolved_key.clone();
        if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
            let loaded_model = match asset_manager_arc.lock() {
                Ok(mut asset_manager) => {
                    if let Some(mapped_name) = asset_manager.get_model_for_object(template_name) {
                        requested_model_name = remap_model_key_alias(&mapped_name);
                    }
                    if trace_this_attempt {
                        info!(
                            "Startup model load: template='{}' model='{}' requested='{}'",
                            template_name, model_name, requested_model_name
                        );
                    }

                    match asset_manager.load_w3d_model(&requested_model_name) {
                        Ok(model) => Some(model),
                        Err(err) => {
                            warn!(
                                "Failed to load W3D model '{}' for object '{}': {}",
                                requested_model_name, template_name, err
                            );
                            None
                        }
                    }
                }
                Err(err) => {
                    if trace_this_attempt {
                        warn!(
                            "Startup model load: asset manager lock poisoned for template='{}' model='{}': {}",
                            template_name, model_name, err
                        );
                    }
                    None
                }
            };

            if let Some(model) = loaded_model {
                graphics_system.cache_model(requested_model_name.clone(), model.clone());
                if requested_model_name != resolved_key {
                    graphics_system.cache_model(resolved_key.clone(), model.clone());
                }
                if requested_model_name != model_name {
                    graphics_system.cache_model(model_name.to_string(), model.clone());
                }
                if template_name != requested_model_name
                    && template_name != model_name
                    && template_name != resolved_key
                {
                    graphics_system.cache_model(template_name.to_string(), model);
                }
                if trace_this_attempt {
                    info!(
                        "Startup model load: success template='{}' requested='{}'",
                        template_name, requested_model_name
                    );
                }
            }
        }

        let resolved = if let Some(model) = graphics_system.get_model(&resolved_key).cloned() {
            Some(model)
        } else if let Some(model) = graphics_system.get_model(model_name).cloned() {
            if model_name != resolved_key {
                graphics_system.cache_model(resolved_key.clone(), model.as_ref().clone());
            }
            Some(model)
        } else if let Some(model) = graphics_system.get_model(template_name).cloned() {
            if resolved_key != template_name {
                graphics_system.cache_model(resolved_key.clone(), model.as_ref().clone());
            }
            Some(model)
        } else if let Some(model) = graphics_system.get_model(&requested_model_name).cloned() {
            if requested_model_name != resolved_key {
                graphics_system.cache_model(resolved_key.clone(), model.as_ref().clone());
            }
            if requested_model_name != template_name {
                graphics_system.cache_model(template_name.to_string(), model.as_ref().clone());
            }
            Some(model)
        } else {
            // Mesh residual path: filesystem W3D (extracted/sample) or honesty placeholder.
            // use_placeholder only when debug cubes are enabled (production remains fail-closed
            // for missing retail meshes unless opt-in).
            let use_placeholder = Self::missing_model_debug_cubes_enabled();
            match resolve_mesh_for_model_key(&resolved_key, use_placeholder) {
                MeshResolveResult::Loaded {
                    model_key,
                    model,
                    source_path,
                } => {
                    if trace_this_attempt {
                        info!(
                            "Startup model load: residual resolve template='{}' key='{}' path={:?}",
                            template_name, model_key, source_path
                        );
                    }
                    graphics_system.cache_model(model_key.clone(), model.clone());
                    if model_key != model_name {
                        graphics_system.cache_model(model_name.to_string(), model.clone());
                    }
                    if model_key != template_name {
                        graphics_system.cache_model(template_name.to_string(), model.clone());
                    }
                    Some(std::sync::Arc::new(model))
                }
                MeshResolveResult::Placeholder { model, .. } => {
                    // Cache under both placeholder sentinel and requested key for draw.
                    graphics_system.cache_model(PLACEHOLDER_MODEL_KEY.to_string(), model.clone());
                    if use_placeholder {
                        // Return Ready so the unit pass can draw the honest placeholder mesh.
                        graphics_system.cache_model(resolved_key.clone(), model.clone());
                        Some(std::sync::Arc::new(model))
                    } else {
                        None
                    }
                }
                MeshResolveResult::Missing { .. } => None,
            }
        };
        if trace_this_attempt && resolved.is_none() {
            warn!(
                "Startup model load: unresolved template='{}' model='{}' requested='{}'",
                template_name, model_name, requested_model_name
            );
        }
        resolved
            .map(RenderModelLoadResult::Ready)
            .unwrap_or(RenderModelLoadResult::Failed)
    }

    /// Drain submissions from the GameClient RenderBridge and convert them
    /// into `RenderItem`s so they flow through the existing ForwardPass.
    ///
    /// C++ parity: drawables submit to the WW3D scene during
    /// `GameClient::update()`; the render pipeline then consumes those
    /// submissions during `RenderPipeline::execute()`.
    #[cfg(feature = "game_client")]
    fn drain_render_bridge_submissions(
        &mut self,
        graphics_system: &mut GraphicsSystem,
        camera_position: Vec3,
        deferred_model_load_budget: &mut usize,
    ) {
        use game_client::render_bridge::get_render_bridge;

        let mut bridge_guard = match get_render_bridge().lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let bridge = match bridge_guard.as_mut() {
            Some(b) => b,
            None => return,
        };

        bridge.flush();

        let submissions = bridge.drain_scene_submissions();
        if submissions.is_empty() {
            return;
        }

        let submissions_count = submissions.len();
        let mut bridge_items_added = 0usize;

        for drained in submissions {
            let submission = drained.submission;
            let is_transparent = drained.is_transparent;
            let model_name = &submission.model_name;
            if model_name.is_empty() {
                continue;
            }

            let render_pass = if is_transparent {
                RenderPass::ForwardTransparent
            } else {
                RenderPass::ForwardOpaque
            };

            let client_transform: Mat4 = submission.world_transform;
            let world_matrix = Mat4::from_cols_array_2d(&client_transform.to_cols_array_2d());
            let world_position = Vec3::new(
                world_matrix.w_axis.x,
                world_matrix.w_axis.y,
                world_matrix.w_axis.z,
            );

            let object_id = crate::game_logic::ObjectId(submission.drawable_id.0);
            let vis_alpha = submission.render_state.opacity;
            let fow_vis = ObjectVisibility {
                visibility_alpha: vis_alpha,
                is_explored: 1.0,
                visibility_falloff: 1.0,
            };

            let load_result = Self::ensure_render_model_loaded(
                graphics_system,
                model_name,
                model_name,
                true,
                deferred_model_load_budget,
            );

            match load_result {
                RenderModelLoadResult::Ready(w3d_model) => {
                    if w3d_model.meshes.is_empty() {
                        if Self::missing_model_debug_cubes_enabled() {
                            if let Some(fallback_model) =
                                graphics_system.get_model_or_fallback("__fallback_cube__")
                            {
                                if !fallback_model.meshes.is_empty() {
                                    let mut item = RenderItem::new(
                                        object_id,
                                        "__fallback_cube__".to_string(),
                                        0,
                                        world_position,
                                        world_matrix,
                                        &fallback_model.meshes[0].material,
                                        render_pass,
                                    );
                                    item.distance = world_position.distance(camera_position);
                                    item.set_fow_visibility(fow_vis);
                                    self.render_items.push(item);
                                    bridge_items_added += 1;
                                }
                            }
                        }
                    } else {
                        let anim_frame = if !w3d_model.animations.is_empty()
                            && w3d_model.hierarchy.is_some()
                        {
                            let obj_key = object_id.0;
                            let state = self.animation_states.entry(obj_key).or_insert_with(|| {
                                let (num_frames, frame_rate) =
                                    w3d_model.animation_metadata(0).unwrap_or((1, 30));
                                ObjectAnimationState {
                                    animation_index: 0,
                                    current_frame: 0.0,
                                    frame_rate: frame_rate as f32,
                                    num_frames,
                                }
                            });
                            state.current_frame
                        } else {
                            0.0
                        };

                        for (mesh_idx, mesh) in w3d_model.meshes.iter().enumerate() {
                            let mut item = RenderItem::new(
                                object_id,
                                model_name.clone(),
                                mesh_idx,
                                world_position,
                                world_matrix,
                                &mesh.material,
                                render_pass,
                            );
                            item.distance = world_position.distance(camera_position);
                            item.set_fow_visibility(fow_vis);
                            item.animation_frame = anim_frame;
                            item.uv_offset_override =
                                Self::mesh_uv_override_for_submission(&submission, &mesh.name);
                            self.render_items.push(item);
                        }
                        bridge_items_added += 1;
                    }
                }
                RenderModelLoadResult::SkippedByBudget | RenderModelLoadResult::Failed => {
                    if Self::missing_model_debug_cubes_enabled() {
                        if let Some(fallback_model) =
                            graphics_system.get_model_or_fallback("__fallback_cube__")
                        {
                            if !fallback_model.meshes.is_empty() {
                                let mut item = RenderItem::new(
                                    object_id,
                                    "__fallback_cube__".to_string(),
                                    0,
                                    world_position,
                                    world_matrix,
                                    &fallback_model.meshes[0].material,
                                    render_pass,
                                );
                                item.distance = world_position.distance(camera_position);
                                item.set_fow_visibility(fow_vis);
                                self.render_items.push(item);
                                bridge_items_added += 1;
                            }
                        }
                    }
                }
            }
        }

        if bridge_items_added > 0 && self.frame_number.is_multiple_of(300) {
            debug!(
                "RenderBridge drain: {} items from {} submissions",
                bridge_items_added, submissions_count
            );
        }
    }

    #[cfg(feature = "game_client")]
    fn mesh_uv_override_for_submission(
        submission: &game_client::render_bridge::DrawSubmission,
        mesh_name: &str,
    ) -> Option<Vec2> {
        let leaf_name = mesh_name.rsplit('.').next().unwrap_or(mesh_name);
        submission
            .mesh_uv_overrides
            .iter()
            .filter(|override_state| {
                leaf_name
                    .get(..override_state.mesh_name_prefix.len())
                    .is_some_and(|prefix| {
                        prefix.eq_ignore_ascii_case(&override_state.mesh_name_prefix)
                    })
            })
            .max_by_key(|override_state| override_state.mesh_name_prefix.len())
            .map(|override_state| Vec2::new(override_state.u_offset, override_state.v_offset))
    }

    /// Sort render items for optimal rendering - equivalent to C++ RenderPipeline::SortRenderItems()
    fn sort_render_items(&mut self) {
        self.render_items.sort_by(Self::compare_render_items);
    }

    /// Execute water rendering pass - equivalent to C++ RenderPipeline::ExecuteWaterPass()
    fn execute_water_pass(
        &mut self,
        _encoder: &mut wgpu::CommandEncoder,
        _graphics_system: &GraphicsSystem,
    ) -> Result<()> {
        self.current_pass = Some(RenderPass::WaterPass);
        // Water pass implementation would go here
        Ok(())
    }

    /// Execute UI rendering pass - equivalent to C++ RenderPipeline::ExecuteUIPass()
    fn execute_ui_pass(
        &mut self,
        _encoder: &mut wgpu::CommandEncoder,
        _graphics_system: &GraphicsSystem,
    ) -> Result<()> {
        self.current_pass = Some(RenderPass::UIPass);
        // UI pass implementation would go here
        Ok(())
    }

    /// Get current render pass
    pub fn current_pass(&self) -> Option<RenderPass> {
        self.current_pass
    }

    /// Get frame number
    pub fn frame_number(&self) -> u64 {
        self.frame_number
    }

    /// Set the current viewing player for FOW calculations
    pub fn set_current_player(&mut self, player_id: u32) {
        self.current_player_id = player_id;
        trace!("RenderPipeline: Set current player to {}", player_id);
    }

    /// Get the current viewing player
    pub fn get_current_player(&self) -> u32 {
        self.current_player_id
    }

    /// Apply FOW visibility to a render object
    ///
    /// This function queries the FOWRenderingBridge to get visibility data
    /// and returns it for use in shader uniforms.
    ///
    /// # Arguments
    ///
    /// * `object_id` - The object to check visibility for
    ///
    /// # Returns
    ///
    /// ObjectVisibility with alpha, explored state, and falloff values
    pub fn apply_fow_visibility_to_render_object(&self, object_id: ObjectID) -> ObjectVisibility {
        // Query FOW system for this object's visibility to current player
        let visibility =
            FOWRenderingBridge::get_object_visibility(self.current_player_id, object_id);

        trace!(
            "FOW visibility for object {} (player {}): alpha={}, explored={}, falloff={}",
            object_id,
            self.current_player_id,
            visibility.visibility_alpha,
            visibility.is_explored,
            visibility.visibility_falloff
        );

        visibility
    }

    /// Check if an object should be rendered based on FOW visibility
    ///
    /// Objects that have never been seen are not rendered at all.
    /// Objects that are explored but not currently visible are rendered with darkening.
    ///
    /// # Arguments
    ///
    /// * `object_id` - The object to check
    ///
    /// # Returns
    ///
    /// true if the object should be rendered (even if darkened)
    pub fn should_render_object(&self, object_id: ObjectID) -> bool {
        FOWRenderingBridge::should_render_object(self.current_player_id, object_id)
    }

    /// Batch query FOW visibility for multiple objects
    ///
    /// More efficient than individual queries when checking many objects.
    ///
    /// # Arguments
    ///
    /// * `object_ids` - List of objects to check
    ///
    /// # Returns
    ///
    /// Map of object_id to visibility state
    pub fn get_batch_fow_visibility(
        &self,
        object_ids: &[ObjectID],
    ) -> std::collections::HashMap<ObjectID, ObjectVisibility> {
        FOWRenderingBridge::get_all_object_visibilities(self.current_player_id, object_ids)
    }

    /// Initialize minimap FOW texture renderer
    ///
    /// Creates the minimap texture renderer for displaying FOW on the minimap UI
    ///
    /// # Arguments
    ///
    /// * `device` - WGPU device
    /// * `queue` - WGPU queue
    /// * `world_bounds` - World coordinate bounds (min, max)
    pub fn initialize_minimap_renderer(
        &mut self,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        world_bounds: (Vec3, Vec3),
    ) -> Result<()> {
        // Use default minimap dimensions (256x256)
        let dimensions = crate::graphics::minimap_renderer::MinimapDimensions::standard();

        let renderer = MinimapTextureRenderer::new(device, queue, dimensions, world_bounds)?;

        self.minimap_renderer = Some(renderer);
        self.minimap_base_needs_refresh = true;
        info!("Initialized minimap FOW texture renderer");
        Ok(())
    }

    /// Record an optional heightmap path hint to be consumed by the terrain subsystem when plumbed.
    pub fn set_heightmap_hint(&mut self, path: Option<String>) {
        self.pending_heightmap_hint_load = path.is_some();
        self.heightmap_path_hint = path;
    }

    /// Retrieve the current heightmap hint (if any).
    pub fn heightmap_hint(&self) -> Option<&str> {
        self.heightmap_path_hint.as_deref()
    }

    /// Record a skybox texture hint array.
    pub fn set_skybox_hint(&mut self, textures: [String; 5]) {
        self.skybox_textures_hint = Some(textures);
    }

    pub fn set_skybox_enabled(&mut self, enabled: bool) {
        self.skybox_enabled = enabled;
    }

    pub fn skybox_hint(&self) -> Option<&[String; 5]> {
        self.skybox_textures_hint.as_ref()
    }

    fn resolved_skybox_hint(&self) -> [String; 5] {
        self.skybox_textures_hint
            .clone()
            .unwrap_or_else(|| DEFAULT_SKYBOX_TEXTURES.map(|name| name.to_string()))
    }

    fn has_explicit_skybox_hint(&self) -> bool {
        self.skybox_textures_hint.is_some()
    }

    fn terrain_clear_color(&self) -> wgpu::Color {
        if std::env::var_os("GENERALS_DEBUG_CLEAR_COLOR").is_some() {
            return wgpu::Color {
                r: 0.0,
                g: 0.55,
                b: 0.0,
                a: 1.0,
            };
        }
        if let Some(color) = self.cached_lighting.as_ref().and_then(|lighting| {
            lighting
                .fog_color
                .or(lighting.ambient_color)
                .or(lighting.sun_color)
        }) {
            return wgpu::Color {
                r: color[0] as f64,
                g: color[1] as f64,
                b: color[2] as f64,
                a: 1.0,
            };
        }
        wgpu::Color::BLACK
    }

    /// Cache map lighting for terrain/sky consumers and push to terrain if ready.
    pub fn set_environment_lighting(
        &mut self,
        sun_direction: Option<[f32; 3]>,
        sun_color: Option<[f32; 3]>,
        ambient_color: Option<[f32; 3]>,
        fog_color: Option<[f32; 3]>,
        fog_range: Option<(f32, f32)>,
    ) {
        let lighting = CachedLighting {
            sun_direction,
            sun_color,
            ambient_color,
            fog_color,
            fog_range,
        };
        self.cached_lighting = Some(lighting.clone());
        self.apply_cached_lighting_to_terrain(&lighting);
    }

    /// Clear any cached lighting state.
    pub fn clear_environment_lighting(&mut self) {
        self.cached_lighting = None;
    }

    #[cfg(feature = "game_client")]
    fn apply_cached_lighting_to_terrain(&self, lighting: &CachedLighting) {
        if let Ok(mut guard) = game_client::terrain::terrain_visual::get_terrain_visual() {
            if let Some(visual) = guard.as_mut() {
                visual.set_lighting(
                    lighting.sun_direction,
                    lighting.sun_color,
                    lighting.ambient_color,
                    lighting.fog_color,
                    lighting.fog_range,
                );
            }
        }
    }

    #[cfg(not(feature = "game_client"))]
    fn apply_cached_lighting_to_terrain(&self, _lighting: &CachedLighting) {}

    /// Attempt to load the heightmap hinted by the map metadata into the TerrainVisual singleton.
    pub fn load_heightmap_from_hint(
        &mut self,
        device: &Arc<wgpu::Device>,
        queue: &Arc<wgpu::Queue>,
        world_bounds: Option<(Vec3, Vec3)>,
    ) -> Result<()> {
        let Some(path) = self.heightmap_hint() else {
            return Ok(());
        };

        info!("Loading heightmap from map hint: {}", path);
        #[cfg(feature = "game_client")]
        {
            game_client::terrain::terrain_visual::init_terrain_visual()
                .map_err(|e| anyhow::anyhow!("Terrain visual init failed: {}", e))?;
            if let Ok(mut guard) = game_client::terrain::terrain_visual::get_terrain_visual() {
                if let Some(visual) = guard.as_mut() {
                    let explicit_world_size = world_bounds.map(|bounds| {
                        (
                            (bounds.1.x - bounds.0.x).abs().max(1.0),
                            (bounds.1.z - bounds.0.z).abs().max(1.0),
                        )
                    });
                    visual
                        .init_gpu_resources(device.clone(), queue.clone())
                        .map_err(|e| anyhow::anyhow!("Terrain GPU init failed: {}", e))?;
                    visual
                        .load_heightmap_with_world_size(path, explicit_world_size)
                        .map_err(|e| anyhow::anyhow!("Terrain heightmap load failed: {}", e))?;
                    self.heightmap_world_size = Some(visual.world_size());

                    // Apply skybox textures if provided.
                    if self.skybox_enabled {
                        let textures = self.resolved_skybox_hint();
                        let borrowed: [&str; 5] = [
                            textures[0].as_str(),
                            textures[1].as_str(),
                            textures[2].as_str(),
                            textures[3].as_str(),
                            textures[4].as_str(),
                        ];
                        if let Err(err) = visual.replace_skybox_textures(&[""; 5], &borrowed) {
                            if self.has_explicit_skybox_hint() {
                                warn!("Failed to apply skybox textures from map/defaults: {}", err);
                            } else {
                                debug!(
                                    "Skipping default skybox texture override because mounted assets do not expose the legacy fallback set: {}",
                                    err
                                );
                            }
                        }
                    }

                    self.pending_heightmap_hint_load = false;

                    // Push lighting into the terrain visual if available.
                    if let Some(ref lighting) = self.cached_lighting {
                        visual.set_lighting(
                            lighting.sun_direction,
                            lighting.sun_color,
                            lighting.ambient_color,
                            lighting.fog_color,
                            lighting.fog_range,
                        );
                    }
                }
            }
        }
        #[cfg(not(feature = "game_client"))]
        {
            debug!("Terrain visual bridge disabled; skipping heightmap hint load.");
        }
        Ok(())
    }

    /// Load terrain visual data from already-parsed runtime terrain (C++ parity fallback when no hint path exists).
    pub fn load_heightmap_from_runtime_terrain(
        &mut self,
        device: &Arc<wgpu::Device>,
        queue: &Arc<wgpu::Queue>,
        game_logic: Option<&GameLogic>,
    ) -> Result<bool> {
        #[cfg(feature = "game_client")]
        {
            let Some(gl) = game_logic else {
                return Ok(false);
            };
            let Some(heightmap) = gl.terrain_heightmap_snapshot() else {
                return Ok(false);
            };
            let heightmap_resolution = (heightmap.width, heightmap.height);

            game_client::terrain::terrain_visual::init_terrain_visual()
                .map_err(|e| anyhow::anyhow!("Terrain visual init failed: {}", e))?;

            let source_hint_owned: Option<std::path::PathBuf> = self
                .presentation_frame
                .as_ref()
                .and_then(|p| {
                    p.world_env
                        .heightmap_hint
                        .as_ref()
                        .map(std::path::PathBuf::from)
                })
                .or_else(|| gl.heightmap_hint().map(|p| p.to_path_buf()));
            let source_hint_ref = source_hint_owned.as_deref();
            let world_bounds = self
                .presentation_frame
                .as_ref()
                .map(|p| p.world_env.world_bounds_vec3())
                .unwrap_or_else(|| gl.world_bounds());
            let world_size = (
                (world_bounds.1.x - world_bounds.0.x).abs().max(1.0),
                (world_bounds.1.z - world_bounds.0.z).abs().max(1.0),
            );

            if let Ok(mut guard) = game_client::terrain::terrain_visual::get_terrain_visual() {
                if let Some(visual) = guard.as_mut() {
                    visual
                        .init_gpu_resources(device.clone(), queue.clone())
                        .map_err(|e| anyhow::anyhow!("Terrain GPU init failed: {}", e))?;
                    visual
                        .load_heightmap_from_data(heightmap, source_hint_ref, Some(world_size))
                        .map_err(|e| {
                            anyhow::anyhow!("Terrain runtime heightmap load failed: {}", e)
                        })?;
                    let source_tile_classes: Vec<
                        game_client::terrain::terrain_visual::TerrainSourceTileClass,
                    > = gl
                        .terrain_texture_classes_snapshot()
                        .into_iter()
                        .map(
                            |class| game_client::terrain::terrain_visual::TerrainSourceTileClass {
                                first_tile: class.first_tile,
                                num_tiles: class.num_tiles,
                                width: class.width,
                                name: class.name,
                            },
                        )
                        .collect();
                    if !source_tile_classes.is_empty() {
                        match visual.load_source_tiles_from_texture_classes(&source_tile_classes) {
                            Ok(loaded) => debug!(
                                "Loaded {} terrain source tiles from {} texture classes",
                                loaded,
                                source_tile_classes.len()
                            ),
                            Err(err) => warn!("Terrain source tile load failed: {}", err),
                        }
                    }
                    self.heightmap_world_size = Some(visual.world_size());
                    self.pending_heightmap_hint_load = false;

                    if let Some(ref lighting) = self.cached_lighting {
                        visual.set_lighting(
                            lighting.sun_direction,
                            lighting.sun_color,
                            lighting.ambient_color,
                            lighting.fog_color,
                            lighting.fog_range,
                        );
                    }

                    info!(
                        "Loaded terrain visual from runtime terrain data ({}x{}, world_size=({:.1}, {:.1}))",
                        heightmap_resolution.0,
                        heightmap_resolution.1,
                        world_size.0,
                        world_size.1
                    );
                    return Ok(true);
                }
            }

            Ok(false)
        }

        #[cfg(not(feature = "game_client"))]
        {
            let _ = (device, queue, game_logic);
            Ok(false)
        }
    }

    /// Sync map roads/bridges into the terrain-road render path.
    /// Prefers frozen `PresentationWorldEnv` road/bridge segments when present.
    pub fn sync_runtime_map_roads(&mut self, game_logic: Option<&GameLogic>) -> Result<()> {
        #[cfg(feature = "game_client")]
        {
            // When presentation is installed, roads/bridges are snapshot-owned even if
            // empty (fail-closed: no live dual-read mid-frame). Live GameLogic residual
            // only for boot/loading without a presentation frame.
            let (road_segments, bridge_segments) =
                if let Some(env) = self.presentation_frame.as_ref().map(|p| &p.world_env) {
                    let roads: Vec<game_client::terrain::terrain_visual::RuntimeRoadVisualSegment> =
                        env.road_segments
                            .iter()
                            .map(|segment| {
                                game_client::terrain::terrain_visual::RuntimeRoadVisualSegment {
                                    // Presentation stores [x,y,z]; visual wants [x,z,y] like live path.
                                    start: [segment.from[0], segment.from[2], segment.from[1]],
                                    end: [segment.to[0], segment.to[2], segment.to[1]],
                                    width: segment.width,
                                    template_name: segment.template_name.clone(),
                                    width_in_texture: segment.width_in_texture,
                                    road_type_id: segment.road_type_id,
                                    start_is_angled: segment.start_is_angled,
                                    start_is_join: segment.start_is_join,
                                    end_is_angled: segment.end_is_angled,
                                    end_is_join: segment.end_is_join,
                                    curve_radius: segment.curve_radius,
                                }
                            })
                            .collect();
                    let bridges: Vec<([f32; 3], [f32; 3], f32, String)> = env
                        .bridge_segments
                        .iter()
                        .map(|b| (b.start, b.end, b.width, b.template_name.clone()))
                        .collect();
                    (roads, bridges)
                } else if let Some(gl) = game_logic {
                    let roads: Vec<game_client::terrain::terrain_visual::RuntimeRoadVisualSegment> =
                        gl.terrain_road_segments_snapshot()
                            .into_iter()
                            .map(|segment| {
                                game_client::terrain::terrain_visual::RuntimeRoadVisualSegment {
                                    start: [segment.from.x, segment.from.z, segment.from.y],
                                    end: [segment.to.x, segment.to.z, segment.to.y],
                                    width: segment.width,
                                    template_name: segment.template_name,
                                    width_in_texture: segment.width_in_texture,
                                    road_type_id: segment.road_type_id,
                                    start_is_angled: segment.start_is_angled,
                                    start_is_join: segment.start_is_join,
                                    end_is_angled: segment.end_is_angled,
                                    end_is_join: segment.end_is_join,
                                    curve_radius: segment.curve_radius,
                                }
                            })
                            .collect();
                    let bridges: Vec<([f32; 3], [f32; 3], f32, String)> = gl
                        .terrain_bridge_segments_snapshot()
                        .into_iter()
                        .map(|(start, end, width, template_name)| {
                            (start.to_array(), end.to_array(), width, template_name)
                        })
                        .collect();
                    (roads, bridges)
                } else {
                    return Ok(());
                };
            if road_segments.is_empty() && bridge_segments.is_empty() {
                return Ok(());
            }

            if let Ok(mut guard) = game_client::terrain::terrain_visual::get_terrain_visual() {
                if let Some(visual) = guard.as_mut() {
                    visual
                        .set_runtime_map_road_segments(&road_segments, &bridge_segments)
                        .map_err(|e| anyhow::anyhow!("Terrain map-road sync failed: {}", e))?;
                }
            }
        }

        #[cfg(not(feature = "game_client"))]
        {
            let _ = game_logic;
        }

        Ok(())
    }

    /// World size from the loaded heightmap, if available.
    pub fn heightmap_world_size(&self) -> Option<(f32, f32)> {
        self.heightmap_world_size
    }

    pub fn sync_heightmap_world_bounds(&mut self, world_bounds: (Vec3, Vec3)) {
        let width = (world_bounds.1.x - world_bounds.0.x).abs().max(1.0);
        let height = (world_bounds.1.z - world_bounds.0.z).abs().max(1.0);
        self.heightmap_world_size = Some((width, height));

        #[cfg(feature = "game_client")]
        if let Ok(mut guard) = game_client::terrain::terrain_visual::get_terrain_visual() {
            if let Some(visual) = guard.as_mut() {
                visual.set_world_size(width, height);
            }
        }
    }

    /// Update minimap FOW texture
    ///
    /// Updates the minimap texture with FOW state. Prefer the presentation
    /// frame's frozen `fow_grid` when available so terrain/minimap overlay does
    /// not re-query the live shroud manager mid-render.
    pub fn update_minimap_fow_texture(&mut self) -> Result<()> {
        // Clone grid before mutably borrowing minimap_renderer (split-borrow).
        let grid = self
            .presentation_frame
            .as_ref()
            .map(|f| f.fow_grid().clone());
        let player_id = self.current_player_id as usize;
        let frame_number = self.frame_number;
        if let Some(ref mut minimap_renderer) = self.minimap_renderer {
            minimap_renderer.update_texture_from_fow_with_grid(
                player_id,
                frame_number,
                grid.as_ref(),
            )?;

            trace!(
                "Updated minimap FOW texture for player {} at frame {} (grid_active={})",
                player_id,
                frame_number,
                grid.as_ref().map(|g| g.active).unwrap_or(false)
            );
        }
        Ok(())
    }

    /// R8 terrain FOW overlay payload from the presentation snapshot (no live shroud).
    ///
    /// Feed into `FowTerrainOverlay::update_texture` when the GPU overlay is bound.
    /// Returns `None` when inactive / fail-open (skip overlay upload).
    pub fn presentation_terrain_fow_r8(&self) -> Option<Vec<u8>> {
        self.presentation_frame
            .as_ref()
            .and_then(|f| f.terrain_fow_r8())
    }

    /// Pack presentation laser Line3D segments into a CPU vertex buffer for WGPU.
    ///
    /// Residual: does **not** write a live `wgpu::Queue` — returns host-testable
    /// interleaved bytes + honesty flags. Prefer this after `set_presentation_frame`
    /// so SegLine upload does not re-read live GameLogic mid-render.
    pub fn pack_presentation_laser_segments(
        &self,
    ) -> crate::graphics::laser_segment_upload::LaserSegmentUpload {
        match self.presentation_frame.as_ref() {
            Some(frame) => crate::graphics::laser_segment_upload::pack_and_mark_upload_ready(frame),
            None => crate::graphics::laser_segment_upload::LaserSegmentUpload::empty(),
        }
    }

    /// Get minimap texture ID for UI rendering.
    /// Pack presentation projectiles into CPU trail buffer (no live GameLogic).
    pub fn pack_presentation_projectiles(
        &self,
    ) -> crate::graphics::projectile_segment_upload::ProjectileSegmentUpload {
        match self.presentation_frame.as_ref() {
            Some(frame) => {
                crate::graphics::projectile_segment_upload::ProjectileSegmentUpload::pack_from_presentation(
                    frame,
                )
            }
            None => crate::graphics::projectile_segment_upload::ProjectileSegmentUpload::empty(),
        }
    }

    /// Pack presentation move-order lines into CPU buffer (no live GameLogic).
    pub fn pack_presentation_move_lines(
        &self,
    ) -> crate::graphics::move_line_upload::MoveLineUpload {
        match self.presentation_frame.as_ref() {
            Some(frame) => {
                crate::graphics::move_line_upload::MoveLineUpload::pack_from_presentation(frame)
            }
            None => crate::graphics::move_line_upload::MoveLineUpload::empty(),
        }
    }

    /// Pack presentation attack-order lines into CPU buffer (no live GameLogic).
    pub fn pack_presentation_attack_lines(
        &self,
    ) -> crate::graphics::attack_line_upload::AttackLineUpload {
        match self.presentation_frame.as_ref() {
            Some(frame) => {
                crate::graphics::attack_line_upload::AttackLineUpload::pack_from_presentation(frame)
            }
            None => crate::graphics::attack_line_upload::AttackLineUpload::empty(),
        }
    }

    pub fn get_minimap_texture_id(&self) -> Option<UiTextureId> {
        self.minimap_renderer.as_ref()?.get_texture_id()
    }

    /// Get minimap coordinates for click handling
    pub fn get_minimap_coordinates(&self) -> Option<&MinimapCoordinates> {
        self.minimap_renderer.as_ref().map(|r| r.get_coordinates())
    }

    /// Update minimap coordinate mapping after world bounds change.
    pub fn update_minimap_world_bounds(&mut self, world_bounds: (Vec3, Vec3)) {
        if let Some(renderer) = self.minimap_renderer.as_mut() {
            renderer.set_world_bounds(world_bounds);
            self.minimap_base_needs_refresh = true;
        }
    }

    /// Inform the minimap renderer about the latest on-screen rectangle.
    pub fn update_minimap_screen_rect(&mut self, top_left: Vec2, size: Vec2) {
        if let Some(renderer) = self.minimap_renderer.as_mut() {
            renderer.set_screen_rect(top_left, size);
        }
    }

    fn refresh_minimap_terrain_base(&mut self, game_logic: Option<&GameLogic>) -> Result<()> {
        let Some(renderer) = self.minimap_renderer.as_mut() else {
            return Ok(());
        };
        if !self.minimap_base_needs_refresh {
            return Ok(());
        }

        let dimensions = renderer.dimensions();
        // Prefer presentation-owned bounds + coarse height grid (no live height re-sample).
        let (bounds, height_env) = if let Some(pres) = self.presentation_frame.as_ref() {
            (
                Some(pres.world_env.world_bounds_vec3()),
                Some(&pres.world_env),
            )
        } else {
            (None, None)
        };
        let base_texture =
            Self::build_minimap_terrain_base_texture(game_logic, dimensions, bounds, height_env);
        renderer.set_base_terrain_texture(base_texture)?;
        self.minimap_base_needs_refresh = false;
        Ok(())
    }

    fn build_minimap_terrain_base_texture(
        game_logic: Option<&GameLogic>,
        dimensions: MinimapDimensions,
        bounds_override: Option<(Vec3, Vec3)>,
        height_env: Option<&crate::presentation_frame::PresentationWorldEnv>,
    ) -> Vec<u8> {
        let width = dimensions.width.max(1);
        let height = dimensions.height.max(1);
        let pixel_count = (width * height) as usize;
        let mut heights = vec![0.0f32; pixel_count];
        let mut has_sample = false;

        let (world_min, world_max) = bounds_override.unwrap_or_else(|| {
            game_logic
                .map(|g| g.world_bounds())
                .unwrap_or((Vec3::new(-500.0, 0.0, -500.0), Vec3::new(500.0, 0.0, 500.0)))
        });
        let world_span_x = (world_max.x - world_min.x).max(1.0);
        let world_span_z = (world_max.z - world_min.z).max(1.0);

        let idx = |x: u32, y: u32| -> usize { (y * width + x) as usize };

        let use_pres_heights = height_env
            .map(|e| e.height_samples_from_terrain && !e.height_samples.is_empty())
            .unwrap_or(false);

        for y in 0..height {
            for x in 0..width {
                let u = (x as f32 + 0.5) / width as f32;
                let v = (y as f32 + 0.5) / height as f32;
                let world = Vec3::new(
                    world_min.x + u * world_span_x,
                    0.0,
                    world_min.z + v * world_span_z,
                );
                let sample = if height_env.is_some() {
                    // Presentation installed: use coarse grid only (None sample = empty
                    // cell). Do not dual-read live terrain_height_at.
                    if use_pres_heights {
                        height_env.and_then(|e| e.sample_height(world.x, world.z))
                    } else {
                        None
                    }
                } else {
                    // Boot/loading without presentation: live residual.
                    game_logic.and_then(|g| g.terrain_height_at(world))
                };
                if let Some(h) = sample {
                    heights[idx(x, y)] = h;
                    has_sample = true;
                }
            }
        }

        if !has_sample {
            return vec![255u8; pixel_count * 4];
        }

        let (mut min_h, mut max_h) = (f32::MAX, f32::MIN);
        for h in &heights {
            min_h = min_h.min(*h);
            max_h = max_h.max(*h);
        }
        let range_h = (max_h - min_h).max(1.0);
        let waterline = min_h + range_h * 0.14;
        let light_dir = Vec3::new(0.45, 0.70, 0.55).normalize();

        let mut texture = vec![0u8; pixel_count * 4];
        for y in 0..height {
            for x in 0..width {
                let x0 = x.saturating_sub(1);
                let x1 = (x + 1).min(width - 1);
                let y0 = y.saturating_sub(1);
                let y1 = (y + 1).min(height - 1);
                let h = heights[idx(x, y)];
                let left = heights[idx(x0, y)];
                let right = heights[idx(x1, y)];
                let up = heights[idx(x, y0)];
                let down = heights[idx(x, y1)];

                let dx = (right - left) / range_h;
                let dz = (down - up) / range_h;
                let normal = Vec3::new(-dx, 1.0, -dz).normalize_or_zero();
                let shade = normal.dot(light_dir).clamp(0.2, 1.0);

                let elevation = ((h - min_h) / range_h).clamp(0.0, 1.0);
                let mut r = 48.0 + (201.0 - 48.0) * elevation;
                let mut g = 62.0 + (177.0 - 62.0) * elevation;
                let mut b = 44.0 + (128.0 - 44.0) * elevation;

                if h <= waterline {
                    let t = ((waterline - h) / range_h / 0.14).clamp(0.0, 1.0);
                    r = r * (1.0 - 0.55 * t) + 55.0 * 0.55 * t;
                    g = g * (1.0 - 0.55 * t) + 92.0 * 0.55 * t;
                    b = b * (1.0 - 0.55 * t) + 140.0 * 0.55 * t;
                }

                let base = idx(x, y) * 4;
                texture[base] = (r * shade).clamp(0.0, 255.0) as u8;
                texture[base + 1] = (g * shade).clamp(0.0, 255.0) as u8;
                texture[base + 2] = (b * shade).clamp(0.0, 255.0) as u8;
                texture[base + 3] = 255;
            }
        }

        #[cfg(feature = "game_client")]
        {
            if let Ok(guard) = game_client::terrain::terrain_visual::get_terrain_visual() {
                if let Some(visual) = guard.as_ref() {
                    let samples = visual.minimap_road_samples(10);
                    let span_norm = world_span_x.max(world_span_z).max(1.0);

                    for sample in samples {
                        let nx = ((sample.position.x - world_min.x) / world_span_x).clamp(0.0, 1.0);
                        let nz = ((sample.position.z - world_min.z) / world_span_z).clamp(0.0, 1.0);
                        let cx = (nx * (width - 1) as f32).round() as i32;
                        let cy = (nz * (height - 1) as f32).round() as i32;
                        let radius = ((sample.width / span_norm) * width.max(height) as f32 * 0.55)
                            .clamp(1.0, 4.0) as i32;
                        let blend =
                            (0.30 + (sample.width / 14.0).clamp(0.0, 0.28)).clamp(0.22, 0.60);
                        Self::paint_minimap_circle(
                            &mut texture,
                            width,
                            height,
                            cx,
                            cy,
                            radius,
                            sample.tint_rgb,
                            blend,
                        );
                    }
                }
            }
        }

        texture
    }

    /// Handle minimap click - convert to world position
    ///
    /// # Arguments
    ///
    /// * `screen_pos` - Screen position of the click
    ///
    /// # Returns
    ///
    /// World position if click was on minimap and area is visible
    pub fn handle_minimap_click(&self, screen_pos: Vec2) -> Option<Vec3> {
        if let Some(ref minimap_renderer) = self.minimap_renderer {
            // Convert screen position to world coordinates
            if let Some(world_pos) = minimap_renderer.screen_to_world(screen_pos) {
                // Check if area is visible/explored
                if minimap_renderer
                    .is_position_visible(world_pos)
                    .unwrap_or(false)
                {
                    return Some(world_pos);
                }
            }
        }
        None
    }

    /// Bind minimap texture to the active UI renderer.
    ///
    /// Makes the minimap texture available for UI rendering.
    ///
    /// # Arguments
    ///
    /// * `renderer` - UI texture registrar/renderer
    pub fn bind_minimap_texture_to_ui<T: UiTextureRegistrar>(
        &mut self,
        renderer: &mut T,
    ) -> Result<UiTextureId> {
        if let Some(ref mut minimap_renderer) = self.minimap_renderer {
            minimap_renderer.bind_to_ui_renderer(renderer)
        } else {
            Err(anyhow::anyhow!("Minimap renderer not initialized"))
        }
    }

    /// Ensure the minimap texture is registered with the active UI renderer.
    pub fn ensure_minimap_texture_bound<T: UiTextureRegistrar>(
        &mut self,
        renderer: &mut T,
    ) -> Result<()> {
        if let Some(ref mut minimap_renderer) = self.minimap_renderer {
            if minimap_renderer.get_texture_id().is_none() {
                minimap_renderer.bind_to_ui_renderer(renderer)?;
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("Minimap renderer not initialized"))
        }
    }

    /// Schedule a callback to run after the WW3D renderer finishes its main passes.
    pub fn enqueue_post_frame_callback<F>(&mut self, callback: F)
    where
        F: FnOnce(&mut ww3d_engine::RenderFrame) -> RendererResult<()> + Send + 'static,
    {
        self.forward_pass.enqueue_post_frame_callback(callback);
    }

    pub fn enqueue_pre_scene_callback<F>(&mut self, callback: F)
    where
        F: FnOnce(&mut ww3d_engine::RenderFrame) -> RendererResult<()> + Send + 'static,
    {
        self.forward_pass.enqueue_pre_scene_callback(callback);
    }
}

impl ForwardPass {
    fn initialize() -> Result<Self> {
        // Initialize WW3D renderer - this may fail if engine is not initialized
        let clear_color = if std::env::var_os("GENERALS_DEBUG_WW3D_CLEAR_COLOR").is_some() {
            Vec4::new(0.0, 0.55, 0.0, 1.0)
        } else {
            Vec4::new(0.0, 0.0, 0.0, 1.0)
        };
        let renderer_config = WgpuMainRendererConfig {
            clear_color,
            ..WgpuMainRendererConfig::default()
        };
        let renderer = WgpuMainRenderer::from_engine(renderer_config)
            .map_err(|e| anyhow::anyhow!("Failed to initialize WW3D renderer: {e:?}"))?;

        // Get engine device and queue - these are Arc clones of the global engine resources
        let device =
            ww3d_engine::device().map_err(|e| anyhow::anyhow!("WW3D device unavailable: {e:?}"))?;
        let queue =
            ww3d_engine::queue().map_err(|e| anyhow::anyhow!("WW3D queue unavailable: {e:?}"))?;

        info!("ForwardPass initialized successfully");

        Ok(Self {
            renderer,
            mesh_cache: HashMap::new(),
            texture_cache: HashMap::new(),
            pending_texture_stream: VecDeque::new(),
            queued_texture_stream: HashSet::new(),
            fallback_texture: None,
            camera: CameraClass::new(),
            device,
            queue,
        })
    }

    /// Check if the forward pass is ready to render
    /// Returns true if all required resources are available
    fn is_ready(&self) -> bool {
        // Verify engine is still initialized by checking if we can get device/queue
        // The Arc references we hold should still be valid, but engine might have shut down
        ww3d_engine::device().is_ok() && ww3d_engine::queue().is_ok()
    }

    #[allow(unused_assignments)]
    fn prewarm_textures_blocking<I, S>(&mut self, texture_names: I) -> Result<TexturePrewarmStats>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut stats = TexturePrewarmStats::default();
        let mut unique = HashSet::new();
        let mut requested: Vec<(String, String)> = Vec::new();

        for texture_name in texture_names {
            let texture_name = texture_name.as_ref().trim();
            if !Self::is_valid_texture_name(texture_name) {
                continue;
            }
            let cache_key = texture_name.to_ascii_lowercase();
            if !unique.insert(cache_key.clone()) {
                continue;
            }
            stats.requested += 1;
            if self.texture_cache.contains_key(&cache_key) {
                stats.cache_hits += 1;
                continue;
            }
            requested.push((texture_name.to_string(), cache_key));
        }

        // Prime all raw payloads while holding the asset manager lock once, matching C++ upfront loads.
        if let Some(asset_manager_arc) = get_asset_manager() {
            let mut asset_manager = asset_manager_arc
                .lock()
                .map_err(|_| anyhow::anyhow!("Asset manager mutex poisoned"))?;
            asset_manager.prime_textures_raw_blocking(requested.iter().map(|(name, _)| name));
        }

        for (texture_name, cache_key) in requested {
            if self.texture_cache.contains_key(&cache_key) {
                stats.cache_hits += 1;
                continue;
            }

            if self.is_known_missing_texture(&texture_name) {
                stats.missing += 1;
                if let Ok(fallback) = self.ensure_fallback_texture() {
                    self.texture_cache.insert(cache_key, fallback);
                }
                continue;
            }

            if let Ok(texture) = self.create_texture_from_cached_assets(&texture_name) {
                self.texture_cache.insert(cache_key, texture);
                stats.resolved += 1;
            } else {
                let _ = self.prime_texture_raw_blocking(&texture_name);
                if let Ok(texture) = self.create_texture_from_cached_assets(&texture_name) {
                    self.texture_cache.insert(cache_key, texture);
                    stats.resolved += 1;
                } else {
                    self.queue_texture_stream(&texture_name);
                }
            }
        }

        // Drain queued texture stream before first visible menu frame.
        for _ in 0..32 {
            if self.pending_texture_stream.is_empty() {
                break;
            }

            let pending_before = self.pending_texture_stream.len();
            let budget = pending_before.clamp(64, 2048);
            self.stream_pending_textures(budget);
            if self.pending_texture_stream.len() >= pending_before {
                break;
            }
        }
        stats.queued_remaining = self.pending_texture_stream.len();
        Ok(stats)
    }

    #[allow(unused_assignments)]
    fn render(
        &mut self,
        graphics_system: &GraphicsSystem,
        render_items: &[RenderItem],
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
        camera_position: Vec3,
        lighting: Option<&CachedLighting>,
    ) -> Result<()> {
        // Check if renderer is ready before attempting to render
        // This prevents crashes when engine is shutting down or not initialized
        if !self.is_ready() {
            warn!("ForwardPass::render - engine not ready, skipping frame");
            return Ok(());
        }

        // C++ parity: first-use textures should resolve quickly; avoid one-texture-per-frame trickle.
        self.stream_pending_textures(self.texture_stream_budget());

        static FP_FRAME: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let fp_frame = FP_FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if fp_frame < 5 {
            info!("ForwardPass::render #{} begin_frame_start", fp_frame);
        }

        // Begin frame - initialize render state
        self.renderer
            .begin_frame()
            .map_err(|e| anyhow::anyhow!("WW3D renderer begin_frame failed: {e:?}"))?;

        if fp_frame < 5 {
            info!("ForwardPass::render #{} begin_frame_done", fp_frame);
        }

        let mut queued_count_total = 0usize;
        let mut queue_error_total = 0usize;

        // Scope to ensure mutex lock is released before end_frame
        {
            let renderer_handle = self.renderer.renderer_handle();

            // Attempt to lock renderer - handle both poisoned and unavailable cases
            let mut renderer = match renderer_handle.try_lock() {
                Ok(guard) => guard,
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    return Err(anyhow::anyhow!("WW3D renderer handle poisoned - another thread panicked while holding the lock"));
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    warn!("WW3D renderer handle already locked - skipping frame");
                    // Still need to end_frame to maintain state
                    self.renderer.end_frame().map_err(|e| {
                        anyhow::anyhow!(
                            "WW3D renderer end_frame failed after lock contention: {e:?}"
                        )
                    })?;
                    return Ok(());
                }
            };

            // Update camera state - must happen before queueing meshes
            self.camera.set_view_matrix(*view_matrix);
            self.camera.set_projection_matrix(*projection_matrix);
            self.camera.set_position(camera_position);
            renderer.set_camera(self.camera.clone());
            renderer.set_light_environment(Self::build_light_environment(lighting));
            Self::log_visibility_probe(
                render_items,
                view_matrix,
                projection_matrix,
                camera_position,
            );
            Self::log_material_probe(render_items);

            if render_items.is_empty() {
                trace!("ForwardPass::render - presenting empty scene frame");
            }

            // Queue opaque + transparent geometry for rendering
            let mut queued_count = 0;
            let mut error_count = 0;
            let mut hidden_count = 0usize;
            let renderable_passes = [RenderPass::ForwardOpaque, RenderPass::ForwardTransparent];

            for item in render_items {
                if !renderable_passes.contains(&item.render_pass) {
                    continue;
                }
                if item.fow_visibility.visibility_alpha <= 0.01 {
                    hidden_count += 1;
                    continue;
                }

                // Prepare mesh instance - handles missing models gracefully
                match self.prepare_mesh_instance(graphics_system, item) {
                    Ok(Some(mesh)) => {
                        // Queue mesh for rendering
                        if let Err(e) = renderer.queue_mesh(mesh) {
                            error!("Failed to queue mesh for item {}: {e:?}", item.object_id);
                            error_count += 1;
                            // Continue processing other items instead of failing entire frame
                            continue;
                        }
                        queued_count += 1;
                    }
                    Ok(None) => {
                        // Model not available - already logged in prepare_mesh_instance
                        continue;
                    }
                    Err(e) => {
                        error!("Failed to prepare mesh for item {}: {e}", item.object_id);
                        error_count += 1;
                        // Continue processing other items
                        continue;
                    }
                }
            }

            trace!(
                "ForwardPass::render - queued {}/{} opaque+transparent items ({} errors, {} hidden-by-alpha)",
                queued_count,
                render_items.len(),
                error_count,
                hidden_count
            );
            queued_count_total = queued_count;
            queue_error_total = error_count;
        } // Mutex lock released here

        // C++ parity: after 3D scene, flush the 2D UI overlay (Shell menus,
        // WindowManager windows) on top of the rendered scene. This is the
        // post-scene 2D pass where gadget draw callbacks render.
        self.renderer.enqueue_post_frame_callback(|frame| {
            crate::graphics::ui_render_pass::flush_ui_to_frame(frame)
        });

        // End frame - submit queued work to GPU (runs post-frame callbacks first)
        self.renderer
            .end_frame()
            .map_err(|e| anyhow::anyhow!("WW3D renderer end_frame failed: {e:?}"))?;

        let stats = self.renderer.stats();
        if queued_count_total > 0 {
            debug!(
                "ForwardPass presented: queued={} queue_errors={} draw_calls={} meshes={} tris={}",
                queued_count_total,
                queue_error_total,
                stats.draw_calls,
                stats.meshes_rendered,
                stats.triangles_rendered
            );
        }

        Ok(())
    }

    fn build_light_environment(lighting: Option<&CachedLighting>) -> Option<LightEnvironmentClass> {
        let mut env = LightEnvironmentClass::new();
        let have_metadata = lighting
            .map(|v| {
                v.sun_direction.is_some()
                    || v.sun_color.is_some()
                    || v.ambient_color.is_some()
                    || v.fog_color.is_some()
                    || v.fog_range.is_some()
            })
            .unwrap_or(false);

        let ambient = lighting
            .and_then(|v| v.ambient_color)
            .or_else(|| lighting.and_then(|v| v.fog_color))
            .or_else(|| lighting.and_then(|v| v.sun_color))
            .unwrap_or([0.30, 0.30, 0.30]);
        env.set_ambient(Vec3::from_array(ambient));

        let direction = lighting
            .and_then(|v| v.sun_direction)
            .unwrap_or([-0.5, -1.0, -0.5]);
        let color = lighting
            .and_then(|v| v.sun_color)
            .or_else(|| lighting.and_then(|v| v.fog_color))
            .or_else(|| lighting.and_then(|v| v.ambient_color))
            .unwrap_or([1.0, 0.9, 0.8]);

        let direction = Vec3::from_array(direction).normalize_or_zero();
        let mut light = LightClass::directional(direction, Vec3::from_array(color), 1.0);
        light.enabled = true;
        env.add_light(Arc::new(Mutex::new(light)));

        static LOGGED_FALLBACK_LIGHTING: AtomicBool = AtomicBool::new(false);
        if !have_metadata && !LOGGED_FALLBACK_LIGHTING.swap(true, Ordering::Relaxed) {
            warn!(
                "ForwardPass lighting metadata unavailable/incomplete; using fallback ambient+sun lighting"
            );
        }
        Some(env)
    }

    fn log_visibility_probe(
        render_items: &[RenderItem],
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
        camera_position: Vec3,
    ) {
        static PROBE_FRAME_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let frame = PROBE_FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
        if !frame.is_multiple_of(120) {
            return;
        }

        let sample_limit = render_items.len().min(512);
        let view_proj = *projection_matrix * *view_matrix;
        let mut finite = 0usize;
        let mut in_front = 0usize;
        let mut in_ndc = 0usize;
        let mut ndc_samples: Vec<String> = Vec::new();

        for item in render_items.iter().take(sample_limit) {
            let world = item.world_position.extend(1.0);
            let clip = view_proj * world;
            if !clip.x.is_finite()
                || !clip.y.is_finite()
                || !clip.z.is_finite()
                || !clip.w.is_finite()
            {
                continue;
            }
            finite += 1;
            if clip.w <= 0.0 {
                continue;
            }
            in_front += 1;

            let inv_w = 1.0 / clip.w;
            let ndc = clip * inv_w;
            if ndc.x >= -1.2
                && ndc.x <= 1.2
                && ndc.y >= -1.2
                && ndc.y <= 1.2
                && ndc.z >= -0.2
                && ndc.z <= 1.2
            {
                in_ndc += 1;
                if ndc_samples.len() < 3 {
                    ndc_samples.push(format!(
                        "{} ndc=({:.2},{:.2},{:.2}) world=({:.1},{:.1},{:.1})",
                        item.model_name,
                        ndc.x,
                        ndc.y,
                        ndc.z,
                        item.world_position.x,
                        item.world_position.y,
                        item.world_position.z
                    ));
                }
            }
        }

        debug!(
            "VisibilityProbe frame={} items={} sampled={} finite={} in_front={} in_ndc={} cam=({:.1},{:.1},{:.1}) sample={:?}",
            frame,
            render_items.len(),
            sample_limit,
            finite,
            in_front,
            in_ndc,
            camera_position.x,
            camera_position.y,
            camera_position.z,
            ndc_samples
        );

        if sample_limit > 0 && in_front > 0 && in_ndc == 0 {
            warn!(
                "VisibilityProbe anomaly: no items in NDC despite in_front={} sampled={} (cam=({:.1},{:.1},{:.1})) sample={:?}",
                in_front,
                sample_limit,
                camera_position.x,
                camera_position.y,
                camera_position.z,
                ndc_samples
            );
        }
    }

    fn log_material_probe(render_items: &[RenderItem]) {
        static MATERIAL_PROBE_FRAME_COUNTER: AtomicUsize = AtomicUsize::new(0);
        let frame = MATERIAL_PROBE_FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
        if !frame.is_multiple_of(120) {
            return;
        }

        let sample_limit = render_items.len().min(512);
        let mut textured = 0usize;
        let mut near_black_diffuse = 0usize;
        let mut near_zero_opacity = 0usize;
        let mut emissive = 0usize;
        let mut samples: Vec<String> = Vec::new();

        for item in render_items.iter().take(sample_limit) {
            let mat = &item.material;
            if mat.texture_name.is_some() {
                textured += 1;
            }
            let max_diffuse = mat
                .diffuse_color
                .x
                .max(mat.diffuse_color.y)
                .max(mat.diffuse_color.z);
            if max_diffuse <= 0.02 {
                near_black_diffuse += 1;
            }
            if mat.opacity <= 0.02 {
                near_zero_opacity += 1;
            }
            if mat.emissive_color.length_squared() > 0.0001 {
                emissive += 1;
            }

            if samples.len() < 3 {
                samples.push(format!(
                    "{} tex={:?} diffuse=({:.2},{:.2},{:.2}) opacity={:.2} blend={:?}",
                    mat.name,
                    mat.texture_name,
                    mat.diffuse_color.x,
                    mat.diffuse_color.y,
                    mat.diffuse_color.z,
                    mat.opacity,
                    mat.blend_mode
                ));
            }
        }

        debug!(
            "MaterialProbe frame={} items={} sampled={} textured={} black_diffuse={} zero_opacity={} emissive={} sample={:?}",
            frame,
            render_items.len(),
            sample_limit,
            textured,
            near_black_diffuse,
            near_zero_opacity,
            emissive,
            samples
        );

        if sample_limit > 0
            && (near_zero_opacity * 100 / sample_limit >= 90
                || near_black_diffuse * 100 / sample_limit >= 90)
        {
            warn!(
                "MaterialProbe anomaly: mostly non-visible materials (sampled={} textured={} black_diffuse={} zero_opacity={} emissive={}) sample={:?}",
                sample_limit,
                textured,
                near_black_diffuse,
                near_zero_opacity,
                emissive,
                samples
            );
        }
    }

    fn prepare_mesh_instance(
        &mut self,
        graphics_system: &GraphicsSystem,
        item: &RenderItem,
    ) -> Result<Option<Arc<MeshClass>>> {
        let mesh_model = match self.ensure_mesh_model(graphics_system, item)? {
            Some(model) => model,
            None => return Ok(None),
        };

        let mut mesh = MeshClass::new();
        mesh.set_transform(item.world_matrix * item.mesh_local_transform);
        mesh.model = Some(mesh_model);
        mesh.alpha_override = item.fow_visibility.visibility_alpha;
        mesh.is_hidden = item.fow_visibility.visibility_alpha <= 0.01;
        mesh.set_uv_offset_override(item.uv_offset_override.map(|offset| [offset.x, offset.y]));
        if std::env::var_os("GENERALS_FORCE_TWO_SIDED").is_some() {
            static LOGGED_FORCE_TWO_SIDED: AtomicBool = AtomicBool::new(false);
            if !LOGGED_FORCE_TWO_SIDED.swap(true, Ordering::Relaxed) {
                warn!("GENERALS_FORCE_TWO_SIDED enabled: forcing two-sided pipelines for mesh diagnostics");
            }
            mesh.is_decal_instance = true;
        }

        if let Some(w3d_model) = graphics_system.get_model(&item.model_name) {
            if !w3d_model.animations.is_empty() && w3d_model.hierarchy.is_some() {
                if let Some(bone_transforms) = w3d_model.sample_animation(0, item.animation_frame) {
                    let matrices: Vec<Mat4> =
                        bone_transforms.iter().map(Mat4::from_cols_array).collect();
                    mesh.set_bone_palette_slice(&matrices);
                }
            }
        }

        Ok(Some(Arc::new(mesh)))
    }

    fn enqueue_post_frame_callback<F>(&mut self, callback: F)
    where
        F: FnOnce(&mut ww3d_engine::RenderFrame) -> RendererResult<()> + Send + 'static,
    {
        self.renderer.enqueue_post_frame_callback(callback);
    }

    fn enqueue_pre_scene_callback<F>(&mut self, callback: F)
    where
        F: FnOnce(&mut ww3d_engine::RenderFrame) -> RendererResult<()> + Send + 'static,
    {
        self.renderer.enqueue_pre_scene_callback(callback);
    }

    fn ensure_mesh_model(
        &mut self,
        graphics_system: &GraphicsSystem,
        item: &RenderItem,
    ) -> Result<Option<Arc<MeshModelClass>>> {
        let cache_key = format!(
            "{}::{}::{}",
            item.model_name, item.mesh_index, item.material_key
        );

        if let Some(model) = self.mesh_cache.get(&cache_key) {
            return Ok(Some(model.clone()));
        }

        let w3d_model = match graphics_system.get_model(&item.model_name) {
            Some(model) => Arc::clone(model),
            None => {
                warn!("No cached W3D model for '{}'", item.model_name);
                return Ok(None);
            }
        };

        let mesh = match w3d_model.meshes.get(item.mesh_index) {
            Some(mesh) => mesh,
            None => {
                warn!(
                    "Model '{}' missing mesh index {}",
                    item.model_name, item.mesh_index
                );
                return Ok(None);
            }
        };

        if let Some(mesh_model) = w3d_model.ww3d_mesh_models.get(&mesh.name) {
            let mesh_model = Arc::clone(mesh_model);
            self.mesh_cache.insert(cache_key, mesh_model.clone());
            return Ok(Some(mesh_model));
        }

        let mesh_model = Arc::new(self.build_mesh_model(&cache_key, mesh, &item.material)?);
        self.mesh_cache.insert(cache_key, mesh_model.clone());
        Ok(Some(mesh_model))
    }

    fn build_mesh_model(
        &mut self,
        cache_key: &str,
        mesh: &crate::assets::models::W3DMesh,
        material: &W3DMaterial,
    ) -> Result<MeshModelClass> {
        let mut model = MeshModelClass::new(cache_key);
        let axis = if mesh.vertices_in_render_space {
            Mat4::IDENTITY
        } else {
            gameplay_to_render_axis_matrix()
        };

        model.vertices = mesh
            .vertices
            .iter()
            .map(|v| {
                let pos = axis.transform_point3(Vec3::from_array(v.position));
                W3dVectorStruct {
                    x: pos.x,
                    y: pos.y,
                    z: pos.z,
                }
            })
            .collect();
        model.normals = mesh
            .vertices
            .iter()
            .map(|v| {
                let normal = axis
                    .transform_vector3(Vec3::from_array(v.normal))
                    .normalize_or_zero();
                W3dVectorStruct {
                    x: normal.x,
                    y: normal.y,
                    z: normal.z,
                }
            })
            .collect();

        if mesh.has_explicit_vertex_colors && !mesh.vertices.is_empty() {
            let mut color_sum = Vec4::ZERO;
            for vertex in &mesh.vertices {
                color_sum += Vec4::new(
                    vertex.color[0],
                    vertex.color[1],
                    vertex.color[2],
                    vertex.color[3],
                );
            }
            let inv = 1.0 / mesh.vertices.len() as f32;
            let avg = color_sum * inv;
            if avg.x.max(avg.y).max(avg.z) <= 0.05 {
                static LOW_VERTEX_COLOR_WARNINGS: AtomicUsize = AtomicUsize::new(0);
                let count = LOW_VERTEX_COLOR_WARNINGS.fetch_add(1, Ordering::Relaxed);
                if count < 20 {
                    warn!(
                        "Mesh '{}' has explicit vertex colors but near-black average ({:.3},{:.3},{:.3},{:.3}); model '{}'",
                        mesh.name, avg.x, avg.y, avg.z, avg.w, cache_key
                    );
                }
            } else {
                static EXPLICIT_VERTEX_COLOR_DEBUGS: AtomicUsize = AtomicUsize::new(0);
                let count = EXPLICIT_VERTEX_COLOR_DEBUGS.fetch_add(1, Ordering::Relaxed);
                if count < 8 {
                    debug!(
                        "Mesh '{}' explicit vertex-color average ({:.3},{:.3},{:.3},{:.3})",
                        mesh.name, avg.x, avg.y, avg.z, avg.w
                    );
                }
            }
        }

        model.stage_texture_coords = mesh
            .stage_texcoords
            .iter()
            .map(|layer| {
                layer
                    .iter()
                    .map(|uv| W3dTexCoordStruct { u: uv[0], v: uv[1] })
                    .collect()
            })
            .collect();

        model.stage_uv_sources = mesh.stage_uv_channels.clone();

        model.texture_coords = if let Some(stage0) = model.stage_texture_coords.first() {
            stage0.clone()
        } else {
            mesh.vertices
                .iter()
                .map(|v| W3dTexCoordStruct {
                    u: v.uv[0],
                    v: v.uv[1],
                })
                .collect()
        };
        if model.stage_texture_coords.is_empty() && !model.texture_coords.is_empty() {
            model
                .stage_texture_coords
                .push(model.texture_coords.clone());
        }
        model.triangles = mesh
            .indices
            .chunks(3)
            .filter_map(|chunk| {
                if chunk.len() != 3 {
                    return None;
                }
                let i0 = chunk[0] as usize;
                let i1 = chunk[1] as usize;
                let i2 = chunk[2] as usize;
                if i0 >= mesh.vertices.len()
                    || i1 >= mesh.vertices.len()
                    || i2 >= mesh.vertices.len()
                {
                    return None;
                }

                let p0 = axis.transform_point3(Vec3::from_array(mesh.vertices[i0].position));
                let p1 = axis.transform_point3(Vec3::from_array(mesh.vertices[i1].position));
                let p2 = axis.transform_point3(Vec3::from_array(mesh.vertices[i2].position));

                let normal = (p1 - p0).cross(p2 - p0);
                let (normal_vec, distance) = if normal.length_squared() > f32::EPSILON {
                    let n = normal.normalize();
                    (n, n.dot(p0))
                } else {
                    (Vec3::Y, 0.0)
                };

                Some(W3dTriangleStruct {
                    vindex: [chunk[0], chunk[1], chunk[2]],
                    attributes: 0,
                    normal: W3dVectorStruct {
                        x: normal_vec.x,
                        y: normal_vec.y,
                        z: normal_vec.z,
                    },
                    distance,
                })
            })
            .collect();

        model.vertex_count = model.vertices.len() as u32;
        model.index_count = (model.triangles.len() * 3) as u32;
        let (pass_count, vertex_material_count, shader_count, texture_count) =
            if !mesh.passes.is_empty() {
                let texture_total = mesh
                    .per_pass_stage_texture_names
                    .iter()
                    .flat_map(|stages| stages.iter())
                    .map(|names| names.len() as u32)
                    .sum::<u32>();
                (
                    mesh.passes.len() as u32,
                    mesh.vertex_materials.len() as u32,
                    mesh.shaders.len() as u32,
                    texture_total,
                )
            } else {
                (
                    1,
                    1,
                    1,
                    if material.texture_name.is_some() {
                        1
                    } else {
                        0
                    },
                )
            };

        model.material_info = Some(W3dMaterialInfoStruct {
            pass_count,
            vert_matl_count: vertex_material_count.max(1),
            shader_count: shader_count.max(1),
            texture_count,
        });

        if !mesh.vertex_materials.is_empty() {
            model.vertex_materials = mesh.vertex_materials.clone();
        } else {
            model.vertex_materials = vec![Self::build_w3d_vertex_material(material)];
        }

        if !mesh.shaders.is_empty() {
            model.shaders = mesh.shaders.clone();
        }

        if let Some(influences) = &mesh.vertex_influences {
            model.set_vertex_influences(influences.clone());
        }
        model.per_stage_face_texcoord_ids = mesh.per_stage_face_texcoord_ids.clone();

        if !mesh.passes.is_empty() {
            let vertex_material_cache = self.build_vertex_material_cache(mesh, material);
            let mut passes = Vec::with_capacity(mesh.passes.len());
            for pass_index in 0..mesh.passes.len() {
                if let Some(pass) =
                    self.build_material_pass_from_mesh(mesh, pass_index, &vertex_material_cache)?
                {
                    passes.push(pass);
                }
            }
            if passes.is_empty() {
                passes.push(self.build_material_pass(material)?);
            }
            model.material_passes = passes;
        } else {
            model.material_passes = vec![self.build_material_pass(material)?];
        }

        Ok(model)
    }

    fn build_material_pass(&mut self, material: &W3DMaterial) -> Result<MaterialPassClass> {
        let mut pass = MaterialPassClass::new();
        let vertex_material = Arc::new(Self::build_vertex_material(material));
        pass.vertex_material = Some(Arc::clone(&vertex_material));
        pass.set_shader(Self::shader_for_material(material));

        if let Some(texture_name) = material_stage_texture(material, 0) {
            if let Some(texture) = self.ensure_texture(texture_name)? {
                pass.set_texture(0, texture);
            }
        }

        for stage in 1..4 {
            if let Some(texture_name) = material_stage_texture(material, stage) {
                if let Some(texture) = self.ensure_texture(texture_name)? {
                    pass.set_texture(stage, texture);
                }
            }
        }

        Ok(pass)
    }

    fn build_vertex_material_cache(
        &self,
        mesh: &crate::assets::models::W3DMesh,
        fallback: &W3DMaterial,
    ) -> Vec<Arc<VertexMaterialClass>> {
        if mesh.vertex_materials.is_empty() {
            return vec![Arc::new(Self::build_vertex_material(fallback))];
        }

        mesh.vertex_materials
            .iter()
            .enumerate()
            .map(|(index, material)| {
                let name = format!("{}_VM{}", mesh.name, index);
                Arc::new(VertexMaterialClass::from_w3d_material(&name, material))
            })
            .collect()
    }

    fn build_material_pass_from_mesh(
        &mut self,
        mesh: &crate::assets::models::W3DMesh,
        pass_index: usize,
        vertex_materials: &[Arc<VertexMaterialClass>],
    ) -> Result<Option<MaterialPassClass>> {
        if pass_index >= mesh.passes.len() {
            return Ok(None);
        }

        let mut pass = MaterialPassClass::new();
        Self::assign_vertex_material_for_pass(&mut pass, mesh, pass_index, vertex_materials);
        if let Some(shader_id_list) = mesh.per_pass_shader_ids.get(pass_index) {
            if let Some(&shader_id) = shader_id_list.first() {
                if let Some(shader_struct) = mesh.shaders.get(shader_id as usize) {
                    pass.shader = ShaderClass::from_w3d_shader(shader_struct);
                }
            }
        } else if let Some(shader_struct) = mesh.shaders.first() {
            pass.shader = ShaderClass::from_w3d_shader(shader_struct);
        } else {
            pass.set_shader(Self::shader_for_material(&mesh.material));
        }

        if pass.shader.get_color_mask()
            == ww3d_renderer_3d::rendering::shader_system::shader::ColorMaskType::Disable
        {
            static DISABLED_COLOR_MASK_WARNINGS: AtomicUsize = AtomicUsize::new(0);
            let count = DISABLED_COLOR_MASK_WARNINGS.fetch_add(1, Ordering::Relaxed);
            if count < 40 {
                warn!(
                    "Shader color mask disabled for mesh='{}' pass={} (shader_bits=0x{:08X})",
                    mesh.name,
                    pass_index,
                    pass.shader.get_bits()
                );
            }
        }

        let has_bound_texture = self.assign_stage_textures_for_pass(&mut pass, mesh, pass_index)?;
        if !has_bound_texture && pass.shader.get_texturing() != TexturingType::Disable {
            // C++ parity fallback: if no texture resource resolved, don't keep a texture-enabled
            // shader state that would sample black and hide otherwise valid geometry.
            pass.shader.set_texturing_enable(false);
        }
        Self::assign_vertex_colors_for_pass(&mut pass, mesh, pass_index);
        Self::assign_mapper_for_pass(&mut pass, mesh, pass_index);
        pass.pass_index = pass_index;
        Ok(Some(pass))
    }

    fn assign_vertex_material_for_pass(
        pass: &mut MaterialPassClass,
        mesh: &crate::assets::models::W3DMesh,
        pass_index: usize,
        cache: &[Arc<VertexMaterialClass>],
    ) {
        if let Some(vm_ids) = mesh.per_pass_vertex_material_ids.get(pass_index) {
            if let Some(&vm_id) = vm_ids.first() {
                if let Some(vm) = cache.get(vm_id as usize) {
                    pass.vertex_material = Some(Arc::clone(vm));
                    return;
                }
            }
        }

        if let Some(vm) = cache.first() {
            pass.vertex_material = Some(Arc::clone(vm));
        }
    }

    fn assign_stage_textures_for_pass(
        &mut self,
        pass: &mut MaterialPassClass,
        mesh: &crate::assets::models::W3DMesh,
        pass_index: usize,
    ) -> Result<bool> {
        let mut assigned = false;
        if let Some(stage_sets) = mesh.per_pass_stage_texture_names.get(pass_index) {
            for (stage, names) in stage_sets.iter().enumerate() {
                let channel = Self::stage_uv_channel_for(mesh, pass_index, stage);
                pass.set_stage_uv_channel(stage, channel);

                if let Some(texture_name) =
                    names.iter().find(|name| Self::is_valid_texture_name(name))
                {
                    if let Some(texture) = self.ensure_texture(texture_name.as_str())? {
                        pass.set_texture(stage, texture);
                        assigned = true;
                        continue;
                    }
                }

                for fallback in mesh.stage_texture_names_from_ids(pass_index, stage) {
                    if !Self::is_valid_texture_name(&fallback) {
                        continue;
                    }
                    if let Some(texture) = self.ensure_texture(&fallback)? {
                        pass.set_texture(stage, texture);
                        assigned = true;
                        break;
                    }
                }
            }
        }

        if !assigned {
            let channel = Self::stage_uv_channel_for(mesh, pass_index, 0);
            pass.set_stage_uv_channel(0, channel);
            self.apply_base_texture(pass, &mesh.material)?;
            assigned = pass.get_texture(0).is_some();
        }
        Ok(assigned)
    }

    fn stage_uv_channel_for(
        mesh: &crate::assets::models::W3DMesh,
        pass_index: usize,
        stage_index: usize,
    ) -> u8 {
        let preceding_stages = Self::stage_layer_offset(mesh, pass_index);
        let idx = preceding_stages + stage_index;
        mesh.stage_uv_channels
            .get(idx)
            .copied()
            .unwrap_or(stage_index as u8)
    }

    fn stage_layer_offset(mesh: &crate::assets::models::W3DMesh, pass_index: usize) -> usize {
        if !mesh.per_pass_stage_texture_ids.is_empty() {
            mesh.per_pass_stage_texture_ids
                .iter()
                .take(pass_index)
                .map(|stages| stages.len())
                .sum()
        } else if !mesh.per_pass_stage_texture_names.is_empty() {
            mesh.per_pass_stage_texture_names
                .iter()
                .take(pass_index)
                .map(|stages| stages.len())
                .sum()
        } else {
            mesh.passes
                .iter()
                .take(pass_index)
                .map(|info| info.texture_count as usize)
                .sum()
        }
    }

    fn is_valid_texture_name(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        if name.eq_ignore_ascii_case("default") {
            return false;
        }
        name.parse::<usize>().is_err()
    }

    fn assign_vertex_colors_for_pass(
        pass: &mut MaterialPassClass,
        mesh: &crate::assets::models::W3DMesh,
        pass_index: usize,
    ) {
        if let Some(colors) = mesh.per_pass_dcg_colors.get(pass_index) {
            if !colors.is_empty() {
                pass.diffuse_vertex_colors = Some(Self::colors_to_vec4(colors));
            }
        }

        if let Some(colors) = mesh.per_pass_dig_colors.get(pass_index) {
            if !colors.is_empty() {
                pass.illumination_vertex_colors = Some(Self::colors_to_vec4(colors));
            }
        }

        if pass.diffuse_vertex_colors.is_none() && mesh.has_explicit_vertex_colors {
            pass.diffuse_vertex_colors = Some(
                mesh.vertices
                    .iter()
                    .map(|vertex| {
                        Vec4::new(
                            vertex.color[0],
                            vertex.color[1],
                            vertex.color[2],
                            vertex.color[3],
                        )
                    })
                    .collect(),
            );
        }
    }

    fn assign_mapper_for_pass(
        pass: &mut MaterialPassClass,
        mesh: &crate::assets::models::W3DMesh,
        pass_index: usize,
    ) {
        let vm_index = mesh
            .per_pass_vertex_material_ids
            .get(pass_index)
            .and_then(|ids| ids.first())
            .copied()
            .and_then(|id| usize::try_from(id).ok());

        if let Some(index) = vm_index {
            if let Some(mapper_info) = mesh.vertex_mappers.get(index) {
                if let Some(mapper) = mapper_info.stage0.or(mapper_info.stage1) {
                    pass.set_mapper_id(mapper.mapper_type);
                    for (idx, arg) in mapper.args.iter().enumerate() {
                        pass.set_mapper_arg(idx, *arg);
                    }
                    pass.set_mapper_float_args(mapper.float_args);
                }
            }
        }
    }

    fn colors_to_vec4(colors: &[W3dRGBAStruct]) -> Vec<Vec4> {
        colors
            .iter()
            .map(|c| {
                Vec4::new(
                    c.r as f32 / 255.0,
                    c.g as f32 / 255.0,
                    c.b as f32 / 255.0,
                    c.a as f32 / 255.0,
                )
            })
            .collect()
    }

    fn apply_base_texture(
        &mut self,
        pass: &mut MaterialPassClass,
        material: &W3DMaterial,
    ) -> Result<()> {
        if let Some(name) = material_stage_texture(material, 0) {
            if let Some(texture) = self.ensure_texture(name)? {
                pass.set_texture(0, texture);
            }
        }
        Ok(())
    }

    fn ensure_texture(&mut self, texture_name: &str) -> Result<Option<Arc<TextureClass>>> {
        if texture_name.is_empty() {
            return Ok(None);
        }

        let cache_key = texture_name.to_lowercase();
        if let Some(texture) = self.texture_cache.get(&cache_key) {
            return Ok(Some(texture.clone()));
        }

        if self.is_known_missing_texture(texture_name) {
            let fallback = self.ensure_fallback_texture()?;
            self.texture_cache.insert(cache_key, fallback.clone());
            return Ok(Some(fallback));
        }

        if let Ok(texture) = self.create_texture_from_cached_assets(texture_name) {
            self.texture_cache
                .insert(texture_name.to_lowercase(), texture.clone());
            return Ok(Some(texture));
        }

        self.queue_texture_stream(texture_name);
        Ok(Some(self.ensure_fallback_texture()?))
    }

    fn create_texture_from_cached_assets(&self, texture_name: &str) -> Result<Arc<TextureClass>> {
        let asset_manager =
            get_asset_manager().ok_or_else(|| anyhow::anyhow!("Asset manager unavailable"))?;
        let asset_manager = asset_manager
            .lock()
            .map_err(|_| anyhow::anyhow!("Asset manager mutex poisoned"))?;
        let texture_key = texture_name.to_lowercase();

        let raw = asset_manager
            .get_raw_texture(&texture_key)
            .ok_or_else(|| anyhow::anyhow!("Texture '{}' not cached", texture_name))?;
        self.build_texture(texture_name, raw)
    }

    fn is_known_missing_texture(&self, texture_name: &str) -> bool {
        let Some(asset_manager_arc) = get_asset_manager() else {
            return false;
        };
        let Ok(asset_manager) = asset_manager_arc.lock() else {
            return false;
        };
        asset_manager.is_known_missing_texture(texture_name)
    }

    fn prime_texture_raw_blocking(&self, texture_name: &str) -> Result<()> {
        let asset_manager =
            get_asset_manager().ok_or_else(|| anyhow::anyhow!("Asset manager unavailable"))?;
        let mut asset_manager = asset_manager
            .lock()
            .map_err(|_| anyhow::anyhow!("Asset manager mutex poisoned"))?;
        asset_manager.prime_texture_raw_blocking(texture_name);
        Ok(())
    }

    fn queue_texture_stream(&mut self, texture_name: &str) {
        let key = texture_name.to_lowercase();
        if self.texture_cache.contains_key(&key) || self.queued_texture_stream.contains(&key) {
            return;
        }

        self.pending_texture_stream
            .push_back(texture_name.to_string());
        self.queued_texture_stream.insert(key.clone());
    }

    fn texture_stream_budget(&self) -> usize {
        let pending = self.pending_texture_stream.len();
        if pending == 0 {
            0
        } else if pending > 256 {
            64
        } else if pending > 96 {
            32
        } else if pending > 24 {
            16
        } else {
            8
        }
    }

    fn stream_pending_textures(&mut self, per_frame_budget: usize) {
        if per_frame_budget == 0 || self.pending_texture_stream.is_empty() {
            return;
        }

        for _ in 0..per_frame_budget {
            let Some(texture_name) = self.pending_texture_stream.pop_front() else {
                break;
            };
            let cache_key = texture_name.to_lowercase();
            self.queued_texture_stream.remove(&cache_key);

            if self.texture_cache.contains_key(&cache_key) {
                continue;
            }

            if self.is_known_missing_texture(&texture_name) {
                if let Ok(fallback) = self.ensure_fallback_texture() {
                    self.texture_cache.insert(cache_key, fallback);
                }
                continue;
            }

            if let Ok(texture) = self.create_texture_from_cached_assets(&texture_name) {
                self.texture_cache.insert(cache_key, texture);
                continue;
            }

            let _ = self.prime_texture_raw_blocking(&texture_name);
            if let Ok(texture) = self.create_texture_from_cached_assets(&texture_name) {
                self.texture_cache.insert(cache_key, texture);
                continue;
            }

            self.queue_texture_stream(&texture_name);
        }
    }

    fn create_fallback_texture(&self, texture_name: &str) -> Result<Arc<TextureClass>> {
        let raw = RawTexture::solid_color(texture_name.to_string(), 4, 4, [255, 0, 255, 255]);
        self.build_texture(&raw.name, &raw)
    }

    fn ensure_fallback_texture(&mut self) -> Result<Arc<TextureClass>> {
        if let Some(texture) = &self.fallback_texture {
            return Ok(texture.clone());
        }
        let texture = self.create_fallback_texture("__missing_texture__")?;
        self.fallback_texture = Some(texture.clone());
        Ok(texture)
    }

    fn build_texture(&self, texture_name: &str, raw: &RawTexture) -> Result<Arc<TextureClass>> {
        let format = if raw.has_alpha {
            TextureFormat::Rgba8Unorm
        } else {
            TextureFormat::Rgba8Unorm
        };

        let mut texture = TextureClass::with_format(texture_name, raw.width, raw.height, format);
        texture
            .replace_pixels(raw.data.clone())
            .map_err(|e| anyhow::anyhow!("Failed to upload pixels for '{}': {e}", texture_name))?;

        Ok(Arc::new(texture))
    }

    fn build_vertex_material(material: &W3DMaterial) -> VertexMaterialClass {
        let mut vm = VertexMaterialClass::new(&material.name);
        vm.diffuse = glam::Vec3::new(
            material.diffuse_color.x,
            material.diffuse_color.y,
            material.diffuse_color.z,
        );
        vm.specular = glam::Vec3::new(
            material.specular_color.x,
            material.specular_color.y,
            material.specular_color.z,
        );
        vm.emissive = glam::Vec3::new(
            material.emissive_color.x,
            material.emissive_color.y,
            material.emissive_color.z,
        );
        vm.opacity = material.opacity;
        vm.shininess = material.shininess.max(1.0);
        vm.translucency = 1.0 - material.opacity;
        vm
    }

    fn build_w3d_vertex_material(material: &W3DMaterial) -> W3dVertexMaterialStruct {
        W3dVertexMaterialStruct {
            attributes: 0,
            ambient: Self::vec_to_rgba(glam::Vec3::splat(0.2), 1.0),
            diffuse: Self::vec_to_rgba(material.diffuse_color, material.opacity),
            specular: Self::vec_to_rgba(material.specular_color, 1.0),
            emissive: Self::vec_to_rgba(material.emissive_color, 1.0),
            shininess: material.shininess,
            opacity: material.opacity,
            translucency: 1.0 - material.opacity,
        }
    }

    fn vec_to_rgba(color: glam::Vec3, alpha: f32) -> W3dRGBAStruct {
        fn to_u8(value: f32) -> u8 {
            (value.clamp(0.0, 1.0) * 255.0).round() as u8
        }

        W3dRGBAStruct {
            r: to_u8(color.x),
            g: to_u8(color.y),
            b: to_u8(color.z),
            a: to_u8(alpha),
        }
    }

    fn shader_for_material(material: &W3DMaterial) -> ShaderClass {
        match material.blend_mode {
            crate::assets::models::BlendMode::Opaque => ShaderClass::get_opaque_shader(),
            crate::assets::models::BlendMode::Alpha => ShaderClass::get_alpha_shader(),
            crate::assets::models::BlendMode::Additive => ShaderClass::get_additive_shader(),
            crate::assets::models::BlendMode::Modulate => ShaderClass::get_opaque_shader(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::models::BlendMode;

    #[test]
    fn material_pass_classifies_transparent_blend_modes() {
        let mut material = W3DMaterial::default();
        assert_eq!(
            RenderPipeline::render_pass_for_material(&material),
            RenderPass::ForwardOpaque
        );

        material.blend_mode = BlendMode::Alpha;
        assert_eq!(
            RenderPipeline::render_pass_for_material(&material),
            RenderPass::ForwardTransparent
        );

        material.blend_mode = BlendMode::Additive;
        assert_eq!(
            RenderPipeline::render_pass_for_material(&material),
            RenderPass::ForwardTransparent
        );
    }

    #[test]
    fn material_pass_classifies_partial_opacity_as_transparent() {
        let mut material = W3DMaterial::default();
        material.opacity = 0.75;
        assert_eq!(
            RenderPipeline::render_pass_for_material(&material),
            RenderPass::ForwardTransparent
        );
    }

    #[test]
    fn missing_model_debug_cubes_are_opt_in() {
        assert!(!RenderPipeline::missing_model_debug_cubes_enabled_from(
            None
        ));
        assert!(!RenderPipeline::missing_model_debug_cubes_enabled_from(
            Some(std::ffi::OsStr::new("0"))
        ));
        assert!(RenderPipeline::missing_model_debug_cubes_enabled_from(
            Some(std::ffi::OsStr::new("1"))
        ));
        assert!(RenderPipeline::missing_model_debug_cubes_enabled_from(
            Some(std::ffi::OsStr::new("TRUE"))
        ));
    }

    #[test]
    fn transparent_items_sort_back_to_front() {
        let mut mat = W3DMaterial::default();
        mat.blend_mode = BlendMode::Alpha;

        let mut far = RenderItem::new(
            ObjectID(1),
            "Model".to_string(),
            0,
            Vec3::new(0.0, 0.0, 100.0),
            Mat4::IDENTITY,
            &mat,
            RenderPass::ForwardTransparent,
        );
        far.distance = 100.0;

        let mut near = RenderItem::new(
            ObjectID(2),
            "Model".to_string(),
            0,
            Vec3::new(0.0, 0.0, 10.0),
            Mat4::IDENTITY,
            &mat,
            RenderPass::ForwardTransparent,
        );
        near.distance = 10.0;

        assert_eq!(
            RenderPipeline::compare_render_items(&far, &near),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn compare_render_items_tiebreaks_by_object_id_for_determinism() {
        let mat = W3DMaterial::default();
        let mut a = RenderItem::new(
            ObjectID(7),
            "Model".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &mat,
            RenderPass::ForwardOpaque,
        );
        let mut b = RenderItem::new(
            ObjectID(2),
            "Model".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &mat,
            RenderPass::ForwardOpaque,
        );

        a.distance = 0.0;
        b.distance = 0.0;
        a.material_key = "same".to_string();
        b.material_key = "same".to_string();

        assert_eq!(
            RenderPipeline::compare_render_items(&a, &b),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            RenderPipeline::compare_render_items(&b, &a),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn unit_render_collection_uses_presentation_frame_without_logic() {
        // Criterion: main unit mesh identity comes from PresentationFrame only.
        // Not full W3D retail — proves collect path does not need GameLogic for
        // position/model/selected when a frame is available.
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use crate::presentation_frame::PresentationFrame;
        use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("UnitMeshPres");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("PresMeshUnit");
        t.set_health(55.0);
        t.set_model("avhummer");
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("PresMeshUnit".into(), t);
        let id = logic
            .create_object("PresMeshUnit", Team::USA, Vec3::new(15.0, 0.0, -3.0))
            .expect("unit");
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
            o.selection_radius = 14.0;
        }

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        // Poison live world — unit collect must ignore it.
        if let Some(o) = logic.get_object_mut(id) {
            o.set_position(Vec3::new(777.0, 0.0, 777.0));
            o.selected = false;
            o.status.selected = false;
        }

        let inputs = RenderPipeline::collect_unit_render_inputs_from_presentation(&snap);
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].id, id);
        assert!((inputs[0].position.x - 15.0).abs() < 0.01);
        assert!((inputs[0].position.z + 3.0).abs() < 0.01);
        assert_eq!(inputs[0].model_key, "avhummer");
        assert_eq!(inputs[0].template_name, "PresMeshUnit");
        assert!(inputs[0].selected);
        assert!((inputs[0].selection_radius - 14.0).abs() < 0.01);
        assert!(!inputs[0].engine_bridged);
        // FOW is snapshot-owned on unit inputs (matches frame object FOW).
        assert_eq!(
            inputs[0].fow_visibility,
            snap.fow_for_object(id).expect("fow on frame")
        );

        // Structural: production collect prefers presentation unit pass + snapshot FOW.
        let src = include_str!("render_pipeline.rs");
        assert!(
            src.contains("unit_render_inputs()"),
            "collect_render_items must iterate presentation unit_render_inputs"
        );
        assert!(
            src.contains("presentation_unit_pass"),
            "collect_render_items must gate live identity behind presentation_unit_pass"
        );
        assert!(
            src.contains("fow_shell_bypass") && src.contains("snapshot_fow"),
            "collect_render_items must apply presentation FOW without live shroud re-query"
        );
    }
    #[test]
    fn presentation_unit_pass_records_zero_live_identity_reads() {
        // Structural: Live branch is the only counter bump; presentation maps UnitPassSource::Presentation only.
        let src = include_str!("render_pipeline.rs");
        assert!(
            src.contains("debug_last_live_unit_identity_reads"),
            "must track live unit identity residual"
        );
        assert!(
            src.contains("UnitPassSource::Presentation"),
            "presentation path required"
        );
        // When presentation_unit_pass, pass_sources come only from unit_inputs map to Presentation.
        let idx = src
            .find("let pass_sources: Vec<UnitPassSource>")
            .expect("pass_sources");
        let window = &src[idx..idx + 500];
        assert!(
            window.contains("UnitPassSource::Presentation")
                && window.contains("presentation_unit_pass"),
            "pass_sources must gate on presentation_unit_pass: {window}"
        );
    }
    #[test]
    fn collect_prefers_presentation_shell_fow_before_live_is_in_shell_game() {
        let src = include_str!("render_pipeline.rs");
        let idx = src
            .find("let bypass_fow = presentation")
            .expect("bypass_fow presentation");
        let window = &src[idx..idx + 280];
        assert!(
            window.contains("fow_shell_bypass") && window.contains("isInShellGame"),
            "shell FOW must prefer presentation then live: {window}"
        );
    }

    #[test]
    fn roads_and_minimap_fail_closed_with_presentation() {
        let src = include_str!("render_pipeline.rs");
        let roads = src
            .split("fn sync_runtime_map_roads")
            .nth(1)
            .and_then(|s| {
                s.split(
                    "
    pub fn ",
                )
                .next()
            })
            .expect("roads body");
        assert!(
            roads.contains("presentation_frame.as_ref().map(|p| &p.world_env)"),
            "roads must key off presentation frame presence"
        );
        assert!(
            !roads.contains(".filter(|e| !e.road_segments.is_empty()"),
            "empty presentation roads must not fall through to live dual-read filter"
        );
        let mm = src
            .split("fn build_minimap_terrain_base_texture")
            .nth(1)
            .and_then(|s| {
                s.split(
                    "
    fn ",
                )
                .next()
            })
            .expect("minimap body");
        assert!(
            mm.contains("if height_env.is_some()"),
            "minimap must fail-closed on presentation height env without live sample"
        );
        assert!(
            mm.contains("game_logic.and_then(|g| g.terrain_height_at(world))"),
            "live height residual remains for boot path without presentation"
        );
    }

    #[test]
    fn prewarm_skips_live_logic_when_presentation_present() {
        let src = include_str!("render_pipeline.rs");
        let body = src
            .split("fn prewarm_startup_models")
            .nth(1)
            .and_then(|s| {
                s.split(
                    "
    fn ",
                )
                .next()
            })
            .expect("prewarm body");
        let names = body
            .split("let template_names")
            .nth(1)
            .expect("template_names block");
        assert!(
            names.contains("if self.presentation_frame.is_some()"),
            "template prewarm must branch on presentation presence"
        );
        // Live last_parsed_map_settings only in the else branch of presentation check.
        let branch = names
            .find("if self.presentation_frame.is_some()")
            .expect("branch");
        let live = names
            .find("last_parsed_map_settings")
            .expect("live fallback exists for boot path");
        assert!(
            live > branch,
            "live map metadata for names must only appear after presentation branch"
        );
    }

    #[test]
    fn execute_accepts_optional_game_logic_for_presentation_only_path() {
        let src = include_str!("render_pipeline.rs");
        assert!(
            src.contains("game_logic: Option<&GameLogic>"),
            "execute/collect must take Option<&GameLogic>"
        );
        let cnc = include_str!("../cnc_game_engine.rs");
        assert!(
            cnc.contains("last_presentation_frame.is_some()")
                && cnc.contains("Some(&self.game_logic)"),
            "engine must pass None when presentation snapshot exists"
        );
    }
    #[test]
    fn minimap_roads_heightmap_take_optional_game_logic() {
        let src = include_str!("render_pipeline.rs");
        assert!(src.contains(
            "fn refresh_minimap_terrain_base(&mut self, game_logic: Option<&GameLogic>)"
        ));
        assert!(src
            .contains("pub fn sync_runtime_map_roads(&mut self, game_logic: Option<&GameLogic>)"));
        assert!(
            src.contains("game_logic: Option<&GameLogic>,")
                && src.contains("load_heightmap_from_runtime_terrain")
        );
        // Presentation path must refresh minimap without requiring live GameLogic.
        assert!(
            src.contains("self.refresh_minimap_terrain_base(game_logic)"),
            "minimap base refresh must accept Option GameLogic (None with snapshot)"
        );
    }

    #[test]
    fn presentation_fow_never_explored_skip_is_snapshot_owned() {
        use crate::fow_rendering::ObjectVisibility;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use crate::presentation_frame::{PresentationFrame, UnitRenderInput};
        use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("FowSnapSkip");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("FowSkipUnit");
        t.set_health(40.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("FowSkipUnit".into(), t);
        let id = logic
            .create_object("FowSkipUnit", Team::China, Vec3::new(1.0, 0.0, 1.0))
            .expect("unit");

        let mut snap = PresentationFrame::build_from_logic(&logic, 0);
        // Force never-explored FOW on the owned snapshot (simulates post-build shroud).
        if let Some(ro) = snap.objects.iter_mut().find(|o| o.id == id) {
            ro.fow_visibility = ObjectVisibility::HIDDEN;
        }
        let inputs = RenderPipeline::collect_unit_render_inputs_from_presentation(&snap);
        assert_eq!(inputs.len(), 1);
        assert!(!inputs[0].fow_should_render());
        assert!(inputs[0].fow_visibility.never_explored());

        // Fogged (explored-not-visible) still renders with darkened alpha.
        let fogged = UnitRenderInput {
            fow_visibility: ObjectVisibility::FOGGED,
            ..inputs[0].clone()
        };
        assert!(fogged.fow_should_render());
        assert!((fogged.fow_visibility.visibility_alpha - 0.3).abs() < 0.01);
    }
}
