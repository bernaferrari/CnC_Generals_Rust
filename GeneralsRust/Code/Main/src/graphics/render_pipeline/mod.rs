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
use super::render_item::{
    FrozenDirectSceneShroudRenderState, FrozenObjectlessDrawableShroudRenderState,
    GhostLightingRoute, RenderItem, RenderItemBonePaletteSource, RenderItemOwner,
    house_color_from_argb,
};
use crate::assets::textures::RawTexture;
use crate::assets::{ModelPrewarmStats, W3DMaterial, W3DModel};
use ww3d_renderer_3d::RendererResult;
use ww3d_renderer_3d::material_system::{MaterialPassClass, VertexMaterialClass};
use ww3d_renderer_3d::rendering::{
    camera_system::CameraClass,
    lighting_system::{LightClass, LightEnvironmentClass},
    mesh_system::{MeshClass, MeshModelClass},
    shader_system::shader::{ShaderClass, TexturingType},
    wgpu_main_renderer::{WgpuMainRenderer, WgpuMainRendererConfig},
};
use ww3d_renderer_3d::texture_system::{TextureClass, TextureFormat};
use ww3d_renderer_3d::rendering::mesh_system::MeshPassTextureProvider;
use ww3d_renderer_3d::rendering::texture_system::dds_loader::{
    DdsCompression, decode_dxt1, decode_dxt3, decode_dxt5,
};
use ww3d_renderer_3d::w3d_format::{
    W3dMaterialInfoStruct, W3dRGBAStruct, W3dTexCoordStruct, W3dTriangleStruct, W3dVectorStruct,
    W3dVertexMaterialStruct,
};

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

/// C++ `WW3DAssetManager::Get_Texture` parity for the ww3d mesh pass-texture
/// lane: resolve a W3D texture name against the archive-backed TextureManager
/// raw cache and hand the ww3d renderer decoded RGBA8 pixels. The mesh lane's
/// `MaterialPassClass` textures are W3D placeholders (name only), so without
/// this bridge every unit/building material binds the white fallback
/// (windowed-smoke-report.md § TextureProvision).
fn resolve_archive_pass_texture(name: &str) -> Option<TextureClass> {
    let requested = name.trim();
    if requested.is_empty() || requested.eq_ignore_ascii_case("none") {
        return None;
    }
    let asset_manager = get_asset_manager()?;
    let mut asset_manager = asset_manager.lock().unwrap_or_else(|e| e.into_inner());
    // First visible use loads synchronously (W3DAssetManager.cpp:127-225).
    asset_manager.prime_texture_raw_blocking(requested);
    let raw = asset_manager.get_raw_texture(requested)?.clone();
    drop(asset_manager);

    // Mirror TextureManager::create_gpu_texture / ForwardPass::build_texture:
    // block-compressed archive payloads decode to RGBA8 because the mesh bind
    // path (MeshRenderManager::ensure_gpu_texture_view) only uploads 32-bit
    // surfaces.
    let decoded = match raw.dds_compression {
        Some(DdsCompression::Dxt1) => decode_dxt1(&raw.data, raw.width, raw.height),
        Some(DdsCompression::Dxt3) => decode_dxt3(&raw.data, raw.width, raw.height),
        Some(DdsCompression::Dxt5) => decode_dxt5(&raw.data, raw.width, raw.height),
        None => Ok(raw.data.clone()),
    };
    let data = match decoded {
        Ok(data) => data,
        Err(err) => {
            warn!("Pass texture '{}' decode failed: {err}", requested);
            return None;
        }
    };
    let mut texture =
        TextureClass::with_format(requested, raw.width, raw.height, TextureFormat::Rgba8Unorm);
    texture.replace_pixels(data).ok()?;
    Some(texture)
}

/// Install the archive-backed pass-texture resolver on the ww3d scene
/// renderer. Called once from pipeline lifecycle after the forward pass (and
/// its renderer) exists; the mesh manager consults it lazily per texture name
/// and caches the upload, so this adds no per-frame cost.
pub(crate) fn install_archive_pass_texture_provider(renderer: &WgpuMainRenderer) {
    let provider: MeshPassTextureProvider = Arc::new(resolve_archive_pass_texture);
    if let Err(err) = renderer.set_pass_texture_provider(provider) {
        warn!("Failed to install ww3d pass-texture provider: {err:?}");
    }
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
    /// Frozen C++ `fog_alpha / clear_alpha` used by the always-fogged ghost
    /// light environment. `None` means the caller has no presentation frame.
    pub fogged_light_fraction: Option<f32>,
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
    /// C++ `RTS3DScene::renderOneObject` fogged/ghost branch: always-fogged
    /// light environment, no projected shroud pass, frustum cull only.
    Ghost,
    WaterPass, // Water surface rendering
    UIPass,    // 2D UI overlay rendering
}

/// One direct-host Drawable shroud result frozen at Main's render boundary.
///
/// This is intentionally only the already-evaluated GameClient scene-cull
/// state for a current object-to-drawable association. It is not a raw
/// `ObjectShroudStatus`, does not own a clear-frame timer, and must never be
/// used to make a W3D scene-pass/material decision. The collector uses it
/// solely for the C++ `Visibility_Check`-equivalent early cull after frustum
/// acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrozenDirectDrawableShroudState {
    /// Host world identity.  This is transient and is advanced only after a
    /// successful world replacement, so a raw ObjectID cannot cross worlds.
    pub host_epoch: u64,
    pub object_id: ObjectID,
    /// Runtime GameClient Drawable identity at capture time.
    pub drawable_id: u32,
    /// Monotonic runtime identity for this particular object→Drawable binding.
    pub binding_generation: u64,
    /// Exact C++ `Drawable::isDrawableEffectivelyHidden` predicate. This is
    /// deliberately not Rust's broader presentation `visible` flag.
    pub scene_effectively_hidden: bool,
    pub fully_obscured: bool,
}

/// Exact C++ objectless `W3DScene::renderOneObject` shroud rule.
///
/// An objectless Drawable starts at Clear. Its optional
/// `m_shroudStatusObjectID` only forces Shrouded when the controller is
/// Fogged or worse; PartialClear, Clear, Invalid, and a missing controller all
/// remain Clear. This is intentionally independent of FOW alpha.
#[inline]
pub(super) const fn objectless_drawable_scene_status(
    controller_status: Option<gamelogic::common::types::ObjectShroudStatus>,
) -> gamelogic::common::types::ObjectShroudStatus {
    match controller_status {
        Some(status)
            if (status as u8) >= (gamelogic::common::types::ObjectShroudStatus::Fogged as u8) =>
        {
            gamelogic::common::types::ObjectShroudStatus::Shrouded
        }
        _ => gamelogic::common::types::ObjectShroudStatus::Clear,
    }
}

/// One direct Drawable selected by Main's frozen W3D collection for the C++
/// `RTS3DScene::renderOneObject`-equivalent scene dispatch.
///
/// This carries no GameClient handle and does not make a material decision.
/// Its raw status is copied from the same presentation input which reached a
/// real render item; Main owns the synchronous callback that resolves this
/// guarded binding key against GameClient immediately before forward render.
/// One record represents a full Drawable binding, not one source Draw module
/// or mesh, so collection de-duplicates it by the four identity fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrozenDirectDrawableSceneCandidate {
    /// Transient host visual-world identity.
    pub host_epoch: u64,
    pub object_id: ObjectID,
    /// Runtime GameClient Drawable identity captured at the presentation boundary.
    pub drawable_id: u32,
    /// Monotonic identity of this object-to-Drawable association.
    pub binding_generation: u64,
    /// Exact frozen C++ ordinal, never inferred from FOW alpha.
    pub raw_status: gamelogic::common::types::ObjectShroudStatus,
    /// Frozen direct-host death fact used by the Drawable-owned grace window.
    pub effectively_dead: bool,
}

/// Main-owned result of resolving one [`FrozenDirectDrawableSceneCandidate`]
/// against GameClient at the direct W3D scene boundary.
///
/// This deliberately mirrors only the two direct-drawable outcomes that can
/// be produced for a full object-to-Drawable binding.  It contains no
/// GameClient handle or type so the renderer can retain the outcome without
/// extending GameClient ownership into WGPU.  Input converts GameClient's
/// validated result to this type before returning from the execute callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrozenDirectDrawableSceneOutcome {
    /// C++ `Drawable::isDrawableEffectivelyHidden()` exited before the W3D
    /// render branch.  Main drops the matching object-owned render items.
    HiddenDirectDrawable,
    /// A direct Drawable reached the normal W3D branch.
    RenderDrawable {
        /// Exact post-clear-grace C++ `ObjectShroudStatus`.
        final_status: gamelogic::common::types::ObjectShroudStatus,
        /// Exact C++ `ss > OBJECTSHROUD_CLEAR` material-pass eligibility.
        pushes_projected_shroud_pass: bool,
    },
}

/// Full-keyed, GameClient-free direct scene result returned to Main.
///
/// Every identity field must match both the candidate that produced real W3D
/// geometry and the current frozen sidecar.  Main deliberately ignores stale,
/// malformed, or ambiguous records rather than letting an ObjectID alone
/// alter a replacement Drawable's render state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrozenDirectDrawableSceneDecision {
    pub host_epoch: u64,
    pub object_id: ObjectID,
    pub drawable_id: u32,
    pub binding_generation: u64,
    pub outcome: FrozenDirectDrawableSceneOutcome,
}

/// Return whether a frozen direct-host candidate is currently fully obscured.
///
/// The input's direct-host lifetime and the sidecar's same-object identity
/// must both match. Missing or objectless entries deliberately fail open here:
/// their C++ W3D branches have distinct shroud behavior and cannot inherit a
/// direct host Drawable's result.
#[inline]
pub(super) fn frozen_direct_candidate_is_fully_obscured(
    sidecar: &HashMap<ObjectID, FrozenDirectDrawableShroudState>,
    host_epoch: Option<u64>,
    object_id: ObjectID,
    drawable_shroud: crate::presentation_frame::PresentationDrawableShroudFacts,
) -> bool {
    drawable_shroud.lifetime.is_direct_host_object()
        && sidecar.get(&object_id).is_some_and(|state| {
            Some(state.host_epoch) == host_epoch
                && state.object_id == object_id
                && state.drawable_id != 0
                && state.binding_generation != 0
                && state.fully_obscured
        })
}

/// Return whether C++ `RTS3DScene::Visibility_Check` rejects a current direct
/// Drawable before model load.
///
/// Source checks `isDrawableEffectivelyHidden()` (only `m_hidden ||
/// m_hiddenByStealth`) and `getFullyObscuredByShroud()` after frustum
/// acceptance. Both facts come from the same guarded frozen sidecar; missing
/// or objectless entries fail open for their separate C++ branches.
#[inline]
pub(super) fn frozen_direct_candidate_is_scene_culled(
    sidecar: &HashMap<ObjectID, FrozenDirectDrawableShroudState>,
    host_epoch: Option<u64>,
    object_id: ObjectID,
    drawable_shroud: crate::presentation_frame::PresentationDrawableShroudFacts,
) -> bool {
    drawable_shroud.lifetime.is_direct_host_object()
        && sidecar.get(&object_id).is_some_and(|state| {
            Some(state.host_epoch) == host_epoch
                && state.object_id == object_id
                && state.drawable_id != 0
                && state.binding_generation != 0
                && (state.scene_effectively_hidden || state.fully_obscured)
        })
}

/// Reconstruct the scene-dispatch payload for one direct candidate after its
/// immutable Visibility_Check culls have accepted it.
///
/// The raw status/death facts belong to the presentation input, while the
/// binding identity comes only from Main's guarded sidecar. A malformed,
/// stale, or fully-obscured sidecar entry cannot become a scene callback.
#[inline]
pub(super) fn frozen_direct_scene_candidate(
    sidecar: &HashMap<ObjectID, FrozenDirectDrawableShroudState>,
    host_epoch: Option<u64>,
    object_id: ObjectID,
    drawable_shroud: crate::presentation_frame::PresentationDrawableShroudFacts,
) -> Option<FrozenDirectDrawableSceneCandidate> {
    let (raw_status, effectively_dead) = drawable_shroud.direct_game_client_status()?;
    let state = sidecar.get(&object_id)?;
    if Some(state.host_epoch) != host_epoch
        || state.object_id != object_id
        || state.drawable_id == 0
        || state.binding_generation == 0
        || state.scene_effectively_hidden
        || state.fully_obscured
    {
        return None;
    }

    Some(FrozenDirectDrawableSceneCandidate {
        host_epoch: state.host_epoch,
        object_id,
        drawable_id: state.drawable_id,
        binding_generation: state.binding_generation,
        raw_status,
        effectively_dead,
    })
}

/// Whether a Main-owned direct scene decision still describes the exact
/// current frozen direct binding and a candidate which actually produced
/// source geometry in this collection.
#[inline]
pub(super) fn frozen_direct_scene_decision_matches_current_binding(
    sidecar: &HashMap<ObjectID, FrozenDirectDrawableShroudState>,
    host_epoch: Option<u64>,
    candidates: &[FrozenDirectDrawableSceneCandidate],
    decision: FrozenDirectDrawableSceneDecision,
) -> bool {
    if Some(decision.host_epoch) != host_epoch
        || decision.host_epoch == 0
        || decision.object_id.0 == 0
        || decision.drawable_id == 0
        || decision.binding_generation == 0
    {
        return false;
    }

    let Some(state) = sidecar.get(&decision.object_id) else {
        return false;
    };
    if state.host_epoch != decision.host_epoch
        || state.object_id != decision.object_id
        || state.drawable_id != decision.drawable_id
        || state.binding_generation != decision.binding_generation
        || state.scene_effectively_hidden
        || state.fully_obscured
    {
        return false;
    }

    candidates.iter().any(|candidate| {
        candidate.host_epoch == decision.host_epoch
            && candidate.object_id == decision.object_id
            && candidate.drawable_id == decision.drawable_id
            && candidate.binding_generation == decision.binding_generation
    })
}

/// Check the direct scene result's self-contained C++ projected-pass rule.
///
/// This uses the frozen final `ObjectShroudStatus` ordinal, never `ObjectVisibility`
/// or FOW alpha.  A callback result that violates the rule is malformed and
/// cannot be retained on a render item.
#[inline]
pub(super) const fn frozen_direct_scene_outcome_has_valid_pass_eligibility(
    outcome: FrozenDirectDrawableSceneOutcome,
) -> bool {
    match outcome {
        FrozenDirectDrawableSceneOutcome::HiddenDirectDrawable => true,
        FrozenDirectDrawableSceneOutcome::RenderDrawable {
            final_status,
            pushes_projected_shroud_pass,
        } => {
            pushes_projected_shroud_pass
                == ((final_status as u8)
                    > (gamelogic::common::types::ObjectShroudStatus::Clear as u8))
        }
    }
}

/// Apply the current direct-scene decisions to the render items produced by
/// this frozen collection.
///
/// Only an exact full binding that appears in both the candidate ledger and
/// current sidecar can affect object-owned items.  Conflicting duplicate
/// records fail closed.  Hidden direct Drawables disappear before sort/forward;
/// rendered Drawables retain their exact final status/pass eligibility for the
/// later projected-shroud material pass.
pub(super) fn apply_frozen_direct_scene_decisions_to_render_items(
    render_items: &mut Vec<RenderItem>,
    sidecar: &HashMap<ObjectID, FrozenDirectDrawableShroudState>,
    host_epoch: Option<u64>,
    candidates: &[FrozenDirectDrawableSceneCandidate],
    decisions: impl IntoIterator<Item = FrozenDirectDrawableSceneDecision>,
) {
    type DirectBindingKey = (u64, ObjectID, u32, u64);

    let mut accepted_by_binding =
        HashMap::<DirectBindingKey, FrozenDirectDrawableSceneDecision>::new();
    let mut ambiguous_bindings = HashSet::<DirectBindingKey>::new();
    for decision in decisions {
        if !frozen_direct_scene_decision_matches_current_binding(
            sidecar, host_epoch, candidates, decision,
        ) || !frozen_direct_scene_outcome_has_valid_pass_eligibility(decision.outcome)
        {
            continue;
        }

        let binding_key = (
            decision.host_epoch,
            decision.object_id,
            decision.drawable_id,
            decision.binding_generation,
        );
        if let Some(previous) = accepted_by_binding.insert(binding_key, decision) {
            if previous != decision {
                ambiguous_bindings.insert(binding_key);
            }
        }
    }
    for binding_key in ambiguous_bindings {
        accepted_by_binding.remove(&binding_key);
    }

    // Current sidecar validation makes ObjectID a safe short-lived index only
    // after the full binding key above has been accepted.
    let accepted_by_object: HashMap<ObjectID, FrozenDirectDrawableSceneDecision> =
        accepted_by_binding
            .into_values()
            .map(|decision| (decision.object_id, decision))
            .collect();

    render_items.retain(|item| {
        let RenderItemOwner::Object(object_id) = item.owner else {
            return true;
        };
        !matches!(
            accepted_by_object
                .get(&object_id)
                .map(|decision| decision.outcome),
            Some(FrozenDirectDrawableSceneOutcome::HiddenDirectDrawable)
        )
    });

    for item in render_items {
        let RenderItemOwner::Object(object_id) = item.owner else {
            continue;
        };
        let Some(decision) = accepted_by_object.get(&object_id) else {
            continue;
        };
        let FrozenDirectDrawableSceneOutcome::RenderDrawable {
            final_status,
            pushes_projected_shroud_pass,
        } = decision.outcome
        else {
            continue;
        };
        item.set_frozen_direct_scene_shroud(FrozenDirectSceneShroudRenderState {
            final_status,
            pushes_projected_shroud_pass,
        });
    }
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
    /// Exact external HLOD aggregate identities already attempted by the
    /// bounded prewarm path. Collection itself must remain cache-only, and a
    /// missing source token must not cause archive I/O every rendered frame.
    hlod_aggregate_prewarm_attempts: HashSet<String>,

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
    /// Per-object, per-source-Draw-module visual state. Equal W3D names must
    /// not collapse separate retail Draw modules into one timeline: C++ keeps
    /// animation and weapon-recoil state on each W3DModelDraw module.
    drawable_visual_states: HashMap<(u32, u32), ObjectVisualState>,
    /// A v4 client Drawable payload which has passed the host's staged-load
    /// boundary but has not yet seen a frozen full presentation topology.
    /// `set_presentation_frame(Some(..))` consumes this exactly once into the
    /// current-frame candidates below; it never performs archive/model I/O.
    pending_client_drawable_restore:
        Option<crate::save_load::snapshot::ClientDrawableWorldSnapshot>,
    /// Source-identity validated restore records for the one frame currently
    /// installed in `presentation_frame`. Collection removes each record
    /// before it consults the normal model-load path, so an unavailable model
    /// or topology cannot trigger retries or retain a stale saved timeline.
    pending_client_drawable_imports:
        HashMap<(u32, u32), crate::save_load::snapshot::ClientDrawableStateSnapshot>,
    /// Exact active W3D ghost scene frozen at the latest GameClient bridge
    /// boundary. It is not a normal Drawable/FOW-alpha stream.
    #[cfg(feature = "game_client")]
    frozen_ghost_scene: Option<game_client::render_bridge::FrozenGhostSceneFrame>,
    last_frame_time: f32,
    /// When set, collect_render_items prefers presentation-owned transforms/model keys.
    presentation_frame: Option<crate::presentation_frame::PresentationFrame>,
    /// Immutable direct-host shroud visibility captured from GameClient for
    /// this presentation frame. It is replaced, never merged, at every Main
    /// render boundary so object IDs cannot carry across frame/world changes.
    presentation_direct_shroud_states: HashMap<ObjectID, FrozenDirectDrawableShroudState>,
    /// The only host epoch accepted by the direct shroud sidecar currently
    /// installed for this presentation handoff.  The ObjectID map is merely a
    /// short-lived lookup index; each value retains its complete binding key.
    presentation_direct_shroud_host_epoch: Option<u64>,
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
    debug_last_particle_systems_packed: u32,
    debug_last_particle_pack_ok: bool,
    /// C++ W3DView viewport height / display height (Default bar = 0.80).
    tactical_view_height_frac: f32,
    tactical_viewport_width: f32,
    tactical_viewport_height: f32,
}

pub(super) const DEFAULT_SKYBOX_TEXTURES: [&str; 5] = [
    "TSMorningN.tga",
    "TSMorningE.tga",
    "TSMorningS.tga",
    "TSMorningW.tga",
    "TSMorningT.tga",
];

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ObjectAnimationState {
    /// Exact selected W3D animation from the frozen source Draw state. `None`
    /// deliberately means bind pose; it must not silently become W3D clip 0.
    animation_binding_key: Option<crate::assets::W3dAnimationBindingKey>,
    /// Full frozen `Hierarchy.Animation` identity.  The process-local binding
    /// key alone cannot be saved: local index zero on one model says nothing
    /// about a different model after a load.
    animation_identity: String,
    current_frame: f32,
    frame_rate: f32,
    num_frames: u32,
    mode: crate::assets::AuthoredDrawAnimationMode,
}

/// Stable selected Draw identity for renderer-local state.  This deliberately
/// mirrors the identity carried by `ClientDrawableStateSnapshot`, but remains
/// a private render-time record rather than a second save representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FrozenVisualDrawIdentity {
    pub source_template_name: String,
    pub model_key: String,
    pub selected_condition_state_index: u32,
    pub animation: Option<FrozenVisualAnimationIdentity>,
}

/// The serializable portion of a frozen HAnim selection, excluding its moving
/// current frame.  Keeping this separate lets a Draw identity remain `Eq` and
/// prevents a saved frame number from becoming source-selection authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FrozenVisualAnimationIdentity {
    pub hierarchy_animation: String,
    pub mode: crate::save_load::snapshot::ClientDrawableAnimationMode,
}

/// One live `W3DModelDraw::WeaponRecoilInfo` equivalent.  It is renderer-only;
/// snapshot conversion happens at the explicit client Drawable boundary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ObjectWeaponRecoilState {
    pub phase: crate::save_load::snapshot::ClientDrawableRecoilPhase,
    pub shift: f32,
    pub recoil_rate: f32,
}

impl Default for ObjectWeaponRecoilState {
    fn default() -> Self {
        Self {
            phase: crate::save_load::snapshot::ClientDrawableRecoilPhase::Idle,
            shift: 0.0,
            recoil_rate: 0.0,
        }
    }
}

/// Unified renderer-local state for a single `(ObjectId, Draw-module)`.
/// Animation may be absent while recoil remains meaningful: a selected Draw
/// state with no HAnim is bind pose, not an instruction to lose a real gun
/// recoil event.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct ObjectVisualState {
    pub identity: Option<FrozenVisualDrawIdentity>,
    pub animation: Option<ObjectAnimationState>,
    /// A restored `None` animation is an explicit saved bind-pose choice.
    /// Keep it distinct from an ordinary cache miss: the latter may become
    /// available after normal prewarm, while a loaded snapshot must never
    /// invent a timeline that was absent when it was saved.
    pub force_bind_pose: bool,
    pub last_seen_weapon_discharge_sequence: u64,
    pub recoil_slots: [Vec<ObjectWeaponRecoilState>; 3],
    pub loco_acceleration_pitch_rate: f32,
    pub loco_acceleration_roll_rate: f32,
}

/// Clear renderer-local state whose keys are meaningful only within one live
/// game world.  Keep this separate from asset caches: a model or texture can
/// safely serve another map, whereas an `(ObjectId, Draw-module)` timeline
/// cannot because object IDs are reused after `GameLogic::reset`.
///
/// `T` keeps the pure state transition independently testable without making
/// a WGPU-backed [`RenderPipeline`] fixture just to exercise this lifecycle.
fn clear_visual_world_state_components<T>(
    drawable_visual_states: &mut HashMap<(u32, u32), ObjectVisualState>,
    render_items: &mut Vec<T>,
    current_pass: &mut Option<RenderPass>,
    last_frame_time: &mut f32,
) {
    drawable_visual_states.clear();
    render_items.clear();
    *current_pass = None;
    *last_frame_time = 0.0;
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
    /// GPU lifetime for Main's immutable projected terrain-shroud snapshot.
    projected_shroud_uploader: crate::graphics::ProjectedShroudGpuUploader,
    /// Per-frame C++ ghost light environment. Ghost meshes are always fogged
    /// and must not inherit the ordinary object/FOW light environment.
    ghost_lighting_environment: Option<Arc<LightEnvironmentClass>>,
    tactical_view_height_frac: f32,
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
mod hlod_aggregate_render;
mod pipeline_collect;
mod pipeline_debug;
mod pipeline_drawable_state;
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
    include_str!("hlod_aggregate_render.rs"),
    include_str!("pipeline_collect.rs"),
    include_str!("pipeline_drawable_state.rs"),
    include_str!("pipeline_debug.rs"),
    include_str!("pipeline_execute.rs"),
    include_str!("pipeline_lifecycle.rs"),
    include_str!("pipeline_minimap.rs"),
    include_str!("pipeline_prewarm.rs"),
    include_str!("residuals.rs"),
);
