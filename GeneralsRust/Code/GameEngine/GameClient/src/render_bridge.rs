//! Render Bridge — Connects GameLogic draw modules to the WWVegas W3D renderer.
//!
//! Port of the C++ W3DGameClient rendering pipeline that lives between
//! GameLogic's DrawModule hierarchy and WW3D2's scene/rendering layer.
//!
//! ## Responsibilities
//!
//! 1. **Collect** visible draw-module data each frame (model name, transform,
//!    condition flags, animation state, bone overrides).
//! 2. **Map** GameLogic `ModelConditionFlags` to WWVegas shader / render states
//!    (opacity, emissive tint, night-map blend, damage overlay, etc.).
//! 3. **Sort** collected submissions by render order (opaque front-to-back,
//!    transparent back-to-front) before submitting to the scene.
//! 4. **Submit** draw calls into a `ww3d_core::Scene` so the WWVegas renderer
//!    can pick them up during its own render loop.
//!
//! Reference C++ entry-points:
//!   - `W3DGameClient::update()` / `W3DGameClient::draw()`
//!   - `Drawable::friend_DrawModule()`
//!   - `DrawableManager::render()`

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::drawable::drawable_draw_pipeline::{with_drawable_pipeline, MeshVertex};
use ww3d_assets::prototypes::MeshPrototype;
use ww3d_assets::AssetManager;
use ww3d_core::animation::{AnimationController, AnimationMode, Hierarchy, Pivot};
use ww3d_core::lighting::{Light, LightEnvironment, LightType};
use ww3d_core::material::{BlendMode, MaterialInfo, Shader, ShaderType, VertexMaterial};
use ww3d_core::mesh::{Mesh, MeshBuilder, Vertex};
use ww3d_core::texture::{TextureData, TextureManager};
use ww3d_core::{
    AABox, BoundingSphere, Camera, Layer, RenderInfo, RenderObject, Scene, SceneBuilder,
};

// ---------------------------------------------------------------------------
// Re-exports from GameLogic that the bridge needs to inspect
// ---------------------------------------------------------------------------

/// Newtype wrapper for a drawable identifier coming from GameLogic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DrawableId(pub u32);

/// Bitflags that the bridge receives from GameLogic draw modules.
bitflags::bitflags! {
    /// Rendering-relevant model condition flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RenderConditionFlags: u64 {
        const PRISTINE             = 1 <<  0;
        const DAMAGED              = 1 <<  1;
        const REALLY_DAMAGED       = 1 <<  2;
        const RUBBLE               = 1 <<  3;
        const MOVING               = 1 <<  4;
        const FIRING_PRIMARY       = 1 <<  5;
        const FIRING_SECONDARY     = 1 <<  6;
        const SELECTED             = 1 <<  8;
        const NIGHT                = 1 << 15;
        const SNOW                 = 1 << 16;
        const ACTIVELY_CONSTRUCTED = 1 << 11;
        const PARTIALLY_CONSTRUCTED = 1 << 12;
        const AWAITING_CONSTRUCTION = 1 << 13;
        const CONSTRUCTION_COMPLETE = 1 << 14;
        const DOOR_1_OPENING       = 1 << 20;
        const DOOR_1_CLOSING       = 1 << 22;
        const DOOR_2_OPENING       = 1 << 23;
        const DOOR_2_CLOSING       = 1 << 25;
        const DISGUISED            = 1 << 60;
        const TOPPLED              = 1 << 61;
        const FLOODED              = 1 << 62;
        const AFLAME               = 1 << 52;
        const SMOLDERING           = 1 << 53;
    }
}

// ---------------------------------------------------------------------------
// Render-state overrides derived from condition flags
// ---------------------------------------------------------------------------

/// Per-object render-state overrides that the bridge computes from
/// `RenderConditionFlags` and applies to WWVegas render objects.
#[derive(Debug, Clone)]
pub struct RenderStateOverrides {
    pub opacity: f32,
    pub emissive_tint: [f32; 3],
    pub apply_night_map: bool,
    pub apply_snow_map: bool,
    pub construction_tint: Option<[f32; 3]>,
    pub damage_overlay: f32,
    pub selected: bool,
    pub hidden: bool,
    pub blend_override: Option<BlendMode>,
    pub wireframe: bool,
}

impl Default for RenderStateOverrides {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            emissive_tint: [0.0; 3],
            apply_night_map: false,
            apply_snow_map: false,
            construction_tint: None,
            damage_overlay: 0.0,
            selected: false,
            hidden: false,
            blend_override: None,
            wireframe: false,
        }
    }
}

impl RenderStateOverrides {
    pub fn from_condition_flags(flags: RenderConditionFlags) -> Self {
        let mut s = Self::default();

        if flags.contains(RenderConditionFlags::AWAITING_CONSTRUCTION) {
            s.hidden = true;
            return s;
        }

        if flags.contains(RenderConditionFlags::REALLY_DAMAGED) {
            s.damage_overlay = 1.0;
        } else if flags.contains(RenderConditionFlags::DAMAGED) {
            s.damage_overlay = 0.5;
        } else if flags.contains(RenderConditionFlags::RUBBLE) {
            s.damage_overlay = 1.0;
            s.opacity = 0.8;
        }

        if flags.contains(RenderConditionFlags::ACTIVELY_CONSTRUCTED) {
            s.construction_tint = Some([0.6, 0.6, 0.6]);
            s.opacity = 1.0;
        } else if flags.contains(RenderConditionFlags::PARTIALLY_CONSTRUCTED) {
            s.construction_tint = Some([0.5, 0.5, 0.5]);
            s.opacity = 0.7;
        }

        s.apply_night_map = flags.contains(RenderConditionFlags::NIGHT);
        s.apply_snow_map = flags.contains(RenderConditionFlags::SNOW);

        if flags.contains(RenderConditionFlags::AFLAME) {
            s.emissive_tint = [1.0, 0.4, 0.05];
        } else if flags.contains(RenderConditionFlags::SMOLDERING) {
            s.emissive_tint = [0.3, 0.15, 0.05];
        }

        s.selected = flags.contains(RenderConditionFlags::SELECTED);

        if flags.contains(RenderConditionFlags::DISGUISED) {
            s.emissive_tint = [
                s.emissive_tint[0] + 0.05,
                s.emissive_tint[1] + 0.05,
                s.emissive_tint[2] + 0.05,
            ];
        }

        if flags.contains(RenderConditionFlags::TOPPLED)
            || flags.contains(RenderConditionFlags::FLOODED)
        {
            s.opacity = 0.6;
        }

        s
    }
}

// ---------------------------------------------------------------------------
// Bone override data for animated models
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BoneOverride {
    pub bone_index: i32,
    pub bone_name: Option<String>,
    pub transform: glam::Mat4,
}

#[derive(Debug, Clone)]
pub struct MeshUvOverride {
    pub mesh_name_prefix: String,
    pub u_offset: f32,
    pub v_offset: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubObjectVisibility {
    pub sub_object_name: String,
    pub hidden: bool,
}

// ---------------------------------------------------------------------------
// Per-frame draw submission
// ---------------------------------------------------------------------------

/// Per-frame projectile stream submission.
///
/// Carries segmented polyline data written by GameLogic's
/// `W3DProjectileStreamDraw::do_draw_module()` through DRAWABLE_STATE, ready
/// for the Device renderer to consume.
#[derive(Debug, Clone)]
pub struct ProjectileStreamSubmission {
    /// Drawable (object) ID that owns this stream.
    pub drawable_id: u32,
    /// Segmented polylines — each inner Vec is one continuous line segment.
    pub lines: Vec<Vec<glam::Vec3>>,
    /// Texture name for the stream visual.
    pub texture_name: String,
    /// Stream width in world units.
    pub width: f32,
    /// UV tile repeat factor along the stream.
    pub tile_factor: f32,
    /// UV scroll speed along the stream axis.
    pub scroll_rate: f32,
}

#[derive(Debug, Clone)]
pub struct DrawSubmission {
    pub drawable_id: DrawableId,
    pub model_name: String,
    pub world_transform: glam::Mat4,
    pub condition_flags: RenderConditionFlags,
    pub render_state: RenderStateOverrides,
    pub bone_overrides: Vec<BoneOverride>,
    pub mesh_uv_overrides: Vec<MeshUvOverride>,
    pub sub_object_visibility: Vec<SubObjectVisibility>,
    pub animation_name: Option<String>,
    pub animation_mode: Option<AnimationMode>,
    pub animation_time: f32,
    pub bounding_sphere: BoundingSphere,
    pub bounding_box: AABox,
    pub sort_level: i32,
    pub opaque: bool,
    pub transparent: bool,
    pub cast_shadow: bool,
}

impl Default for DrawSubmission {
    fn default() -> Self {
        Self {
            drawable_id: DrawableId(0),
            model_name: String::new(),
            world_transform: glam::Mat4::IDENTITY,
            condition_flags: RenderConditionFlags::empty(),
            render_state: RenderStateOverrides::default(),
            bone_overrides: Vec::new(),
            mesh_uv_overrides: Vec::new(),
            sub_object_visibility: Vec::new(),
            animation_name: None,
            animation_mode: None,
            animation_time: 0.0,
            bounding_sphere: BoundingSphere::zero(),
            bounding_box: AABox::zero(),
            sort_level: 0,
            opaque: true,
            transparent: false,
            cast_shadow: true,
        }
    }
}

// ---------------------------------------------------------------------------
// W3D render object wrapper
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct BridgeRenderObject {
    submission: DrawSubmission,
    model: Option<Arc<dyn RenderObject>>,
}

impl BridgeRenderObject {
    fn apply_render_state(&self, render_obj: &mut dyn RenderObject) {
        let state = &self.submission.render_state;

        if state.opacity < 1.0 {
            if let Some(mesh) = render_obj.as_any_mut().downcast_mut::<Mesh>() {
                mesh.set_alpha_override(state.opacity);
            }
        }

        let emissive_strength = state.emissive_tint.iter().cloned().fold(0.0_f32, f32::max);
        if emissive_strength > 0.0 {
            if let Some(mesh) = render_obj.as_any_mut().downcast_mut::<Mesh>() {
                mesh.set_material_pass_emissive_override(emissive_strength.clamp(0.0, 1.0));
            }
        }

        if state.damage_overlay > 0.0 {
            if let Some(mesh) = render_obj.as_any_mut().downcast_mut::<Mesh>() {
                let current = mesh.get_alpha_override();
                mesh.set_alpha_override(current * (1.0 - state.damage_overlay * 0.3));
            }
        }
    }
}

impl RenderObject for BridgeRenderObject {
    fn class_id(&self) -> ww3d_core::RenderObjClassId {
        self.model
            .as_ref()
            .map(|m| m.class_id())
            .unwrap_or(ww3d_core::RenderObjClassId::Mesh)
    }

    fn name(&self) -> &str {
        &self.submission.model_name
    }

    fn set_name(&mut self, name: String) {
        self.submission.model_name = name;
    }

    fn clone_object(&self) -> Box<dyn RenderObject> {
        Box::new(BridgeRenderObject {
            submission: self.submission.clone(),
            model: self.model.clone(),
        })
    }

    fn render(&mut self, info: &RenderInfo) -> ww3d_core::errors::W3DResult<()> {
        if self.submission.render_state.hidden {
            return Ok(());
        }

        let world = glam_to_ww_mat4(self.submission.world_transform);

        if let Some(ref model) = self.model {
            let mut instance = model.clone_object();
            instance.set_transform(world);
            self.apply_render_state(instance.as_mut());
            instance.render(info)?;
        }

        Ok(())
    }

    fn get_obj_space_bounding_sphere(&self) -> BoundingSphere {
        self.submission.bounding_sphere
    }

    fn get_obj_space_bounding_box(&self) -> AABox {
        self.submission.bounding_box
    }

    fn get_transform(&self) -> ww3d_core::glam::Mat4 {
        glam_to_ww_mat4(self.submission.world_transform)
    }

    fn set_transform(&mut self, transform: ww3d_core::glam::Mat4) {
        self.submission.world_transform = ww_to_glam_mat4(transform);
    }

    fn get_sort_level(&self) -> i32 {
        self.submission.sort_level
    }

    fn set_sort_level(&mut self, level: i32) {
        self.submission.sort_level = level;
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// ---------------------------------------------------------------------------
// Main Render Bridge
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct RenderBridgeStats {
    pub submissions_received: usize,
    pub culled: usize,
    pub rendered: usize,
    pub opaque_draws: usize,
    pub transparent_draws: usize,
    pub hidden: usize,
    pub bridge_time_s: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshUvOverrideStateSummary {
    pub mesh_name_prefix: String,
    pub u_offset_bits: u32,
    pub v_offset_bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubObjectVisibilityStateSummary {
    pub sub_object_name: String,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderObjectStateSummary {
    pub drawable_id: u32,
    pub model_name: String,
    pub layer_name: String,
    pub transparent: bool,
    pub sort_level: i32,
    pub condition_bits: u64,
    pub opacity_bits: u32,
    pub damage_overlay_bits: u32,
    pub selected: bool,
    pub night: bool,
    pub snow: bool,
    pub hidden: bool,
    pub mesh_uv_override_count: usize,
    pub mesh_uv_overrides: Vec<MeshUvOverrideStateSummary>,
    pub sub_object_visibility_count: usize,
    pub sub_object_visibility: Vec<SubObjectVisibilityStateSummary>,
    pub bone_override_count: usize,
    pub animation_name: Option<String>,
    pub world_translation_bits: [u32; 3],
    pub bounding_radius_bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectileStreamStateSummary {
    pub drawable_id: u32,
    pub line_count: usize,
    pub point_count: usize,
    pub texture_name: String,
    pub width_bits: u32,
    pub tile_factor_bits: u32,
    pub scroll_rate_bits: u32,
}

#[derive(Debug, Clone)]
pub struct RenderFrameStateSummary {
    pub stats: RenderBridgeStats,
    pub objects: Vec<RenderObjectStateSummary>,
    pub projectile_streams: Vec<ProjectileStreamStateSummary>,
    pub fingerprint: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelResolution {
    Asset,
}

#[derive(Debug, Clone)]
pub struct DrainedDrawSubmission {
    pub submission: DrawSubmission,
    pub is_transparent: bool,
    pub model_resolution: Option<ModelResolution>,
}

pub struct RenderBridge {
    scene: Scene,
    pending: Vec<DrawSubmission>,
    pending_projectile_streams: Vec<ProjectileStreamSubmission>,
    scene_lines:
        HashMap<game_engine::common::system::scene_submission::SceneLineId, SceneLineEntry>,
    camera: Option<Camera>,
    model_cache: HashMap<String, Arc<dyn RenderObject>>,
    model_resolution: HashMap<String, ModelResolution>,
    asset_manager: AssetManager,
    asset_search_paths: Vec<PathBuf>,
    stats: RenderBridgeStats,
    last_frame_objects: Vec<RenderObjectStateSummary>,
    render_info: RenderInfo,
    elapsed_time: f32,
}

struct SceneLineEntry {
    start: glam::Vec3,
    end: glam::Vec3,
    width: f32,
    color: [f32; 4],
    texture_name: String,
    tile_factor: f32,
    visible: bool,
}

static NEXT_LINE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl RenderBridge {
    pub fn new() -> Self {
        let scene = SceneBuilder::new("GameLogic Bridge Scene".to_string()).build();

        Self {
            scene,
            pending: Vec::with_capacity(2048),
            pending_projectile_streams: Vec::with_capacity(64),
            scene_lines: HashMap::new(),
            camera: None,
            model_cache: HashMap::new(),
            model_resolution: HashMap::new(),
            asset_manager: AssetManager::new(),
            asset_search_paths: Vec::new(),
            stats: RenderBridgeStats::default(),
            last_frame_objects: Vec::new(),
            render_info: RenderInfo::new(),
            elapsed_time: 0.0,
        }
    }

    pub fn begin_frame(&mut self, camera: &Camera, delta_time: f32) {
        self.pending.clear();
        self.pending_projectile_streams.clear();
        self.last_frame_objects.clear();
        let mut frame_camera = camera.clone();
        self.elapsed_time += delta_time;

        self.render_info = RenderInfo {
            view_projection: frame_camera.view_projection_matrix(),
            view: frame_camera.view_matrix(),
            projection: frame_camera.projection_matrix(),
            camera_position: frame_camera.position(),
            delta_time,
            elapsed_time: self.elapsed_time,
        };

        self.camera = Some(frame_camera);
    }

    pub fn submit(&mut self, submission: DrawSubmission) {
        self.pending.push(submission);
    }

    pub fn submit_projectile_stream(&mut self, submission: ProjectileStreamSubmission) {
        self.pending_projectile_streams.push(submission);
    }

    pub fn drain_projectile_stream_submissions(&mut self) -> Vec<ProjectileStreamSubmission> {
        std::mem::take(&mut self.pending_projectile_streams)
    }

    pub fn flush(&mut self) {
        let start = std::time::Instant::now();

        let mut stats = RenderBridgeStats {
            submissions_received: self.pending.len(),
            ..Default::default()
        };

        // Collect model names needed first (no borrow conflict)
        let model_names: Vec<String> = self
            .pending
            .iter()
            .filter(|s| !s.render_state.hidden)
            .map(|s| s.model_name.to_lowercase())
            .collect();

        // Phase 2.5: Ensure models are loaded in the cache.
        for name in &model_names {
            if !self.model_cache.contains_key(name) {
                if let Some((render_obj, resolution)) = self.resolve_model(name) {
                    self.model_cache.insert(name.clone(), render_obj);
                    self.model_resolution.insert(name.clone(), resolution);
                }
            }
        }

        // Phase 1: Filter hidden objects.
        let visible: Vec<DrawSubmission> = self
            .pending
            .drain(..)
            .filter(|s| {
                if s.render_state.hidden {
                    stats.hidden += 1;
                    false
                } else {
                    true
                }
            })
            .collect();

        // Phase 2: Frustum cull.
        let mut camera = match &self.camera {
            Some(c) => c.clone(),
            None => return,
        };

        let after_cull: Vec<DrawSubmission> = visible
            .into_iter()
            .filter(|s| {
                let local_center = ww_to_game_vec3(s.bounding_sphere.center);
                let world_center = s.world_transform.transform_point3(local_center);
                let world_sphere =
                    BoundingSphere::new(game_to_ww_vec3(world_center), s.bounding_sphere.radius);

                if camera.is_sphere_visible(&world_sphere) {
                    true
                } else {
                    stats.culled += 1;
                    false
                }
            })
            .collect();

        let resolved: Vec<DrawSubmission> = after_cull
            .into_iter()
            .filter(|s| {
                if self.model_cache.contains_key(&s.model_name.to_lowercase()) {
                    true
                } else {
                    stats.culled += 1;
                    false
                }
            })
            .collect();

        // Phase 3: Partition into opaque / transparent.
        let mut opaque: Vec<DrawSubmission> = Vec::new();
        let mut transparent: Vec<DrawSubmission> = Vec::new();

        for s in resolved {
            if s.transparent {
                transparent.push(s);
            } else {
                opaque.push(s);
            }
        }

        // Phase 4: Sort opaque front-to-back, transparent back-to-front.
        opaque.sort_by(|a, b| {
            let dist_a = distance_sq_to_camera(a, &camera);
            let dist_b = distance_sq_to_camera(b, &camera);
            dist_a
                .partial_cmp(&dist_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        transparent.sort_by(|a, b| {
            let dist_a = distance_sq_to_camera(a, &camera);
            let dist_b = distance_sq_to_camera(b, &camera);
            dist_b
                .partial_cmp(&dist_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        stats.opaque_draws = opaque.len();
        stats.transparent_draws = transparent.len();
        stats.rendered = opaque.len() + transparent.len();

        let mut last_frame_objects = Vec::with_capacity(stats.rendered);
        last_frame_objects.extend(
            opaque
                .iter()
                .map(|s| RenderObjectStateSummary::from_submission(s, "opaque", false)),
        );
        last_frame_objects.extend(
            transparent
                .iter()
                .map(|s| RenderObjectStateSummary::from_submission(s, "transparent", true)),
        );

        // Phase 5: Rebuild the scene layers.
        self.scene.clear();

        {
            let mut opaque_layer = Layer::new("opaque".to_string());
            opaque_layer.set_sort_level(0);
            for s in opaque {
                let key = s.model_name.to_lowercase();
                let model = self.model_cache.get(&key).cloned();
                let obj = BridgeRenderObject {
                    submission: s,
                    model,
                };
                opaque_layer.add_object(Box::new(obj));
            }
            self.scene.add_layer(opaque_layer);
        }

        if !transparent.is_empty() {
            let mut trans_layer = Layer::new("transparent".to_string());
            trans_layer.set_sort_level(100);
            for s in transparent {
                let key = s.model_name.to_lowercase();
                let model = self.model_cache.get(&key).cloned();
                let obj = BridgeRenderObject {
                    submission: s,
                    model,
                };
                trans_layer.add_object(Box::new(obj));
            }
            self.scene.add_layer(trans_layer);
        }

        stats.bridge_time_s = start.elapsed().as_secs_f32();
        self.stats = stats;
        self.last_frame_objects = last_frame_objects;
    }

    fn resolve_model(&mut self, name: &str) -> Option<(Arc<dyn RenderObject>, ModelResolution)> {
        if let Some(obj) = self.asset_manager.create_render_obj(name) {
            return Some((
                Arc::from(Box::new(WrapRenderObj(obj)) as Box<dyn RenderObject>),
                ModelResolution::Asset,
            ));
        }

        for search_path in &self.asset_search_paths {
            let candidate = search_path.join(format!("{name}.w3d"));
            if candidate.exists() {
                let known_meshes = mesh_prototype_names(&self.asset_manager);
                if self.asset_manager.load_3d_assets(&candidate).is_ok() {
                    register_newly_loaded_meshes_for_drawable_pipeline(
                        &self.asset_manager,
                        name,
                        &candidate,
                        &known_meshes,
                    );
                    if let Some(obj) = self.asset_manager.create_render_obj(name) {
                        return Some((
                            Arc::from(Box::new(WrapRenderObj(obj)) as Box<dyn RenderObject>),
                            ModelResolution::Asset,
                        ));
                    }
                }
            }
        }

        None
    }

    /// Drain all processed submissions from the scene layers after `flush()`.
    ///
    /// Returns drained draw submissions. The scene is cleared after draining
    /// so the submissions are not rendered a second time.
    ///
    /// This is the bridge point where the GameClient drawable pipeline hands
    /// off its culled/sorted submissions to the main `RenderPipeline`.
    pub fn drain_scene_submissions(&mut self) -> Vec<DrainedDrawSubmission> {
        let mut result = Vec::new();

        for i in 0..self.scene.layer_count() {
            let is_transparent = self
                .scene
                .get_layer(i)
                .map(|l| l.name() == "transparent")
                .unwrap_or(false);

            if let Some(layer) = self.scene.get_layer(i) {
                for obj in layer.objects_slice() {
                    if let Some(bridge_obj) = obj.as_any().downcast_ref::<BridgeRenderObject>() {
                        let key = bridge_obj.submission.model_name.to_lowercase();
                        result.push(DrainedDrawSubmission {
                            submission: bridge_obj.submission.clone(),
                            is_transparent,
                            model_resolution: self.model_resolution.get(&key).copied(),
                        });
                    }
                }
            }
        }

        self.scene.clear();
        result
    }

    pub fn render_state_summary(&self) -> RenderFrameStateSummary {
        let objects = self.last_frame_objects.clone();
        let projectile_streams = self
            .pending_projectile_streams
            .iter()
            .map(ProjectileStreamStateSummary::from_submission)
            .collect::<Vec<_>>();

        let fingerprint = stable_render_fingerprint(&self.stats, &objects, &projectile_streams);

        RenderFrameStateSummary {
            stats: self.stats.clone(),
            objects,
            projectile_streams,
            fingerprint,
        }
    }

    pub fn end_frame(&mut self) {
        if !self.pending.is_empty() {
            self.flush();
        }

        if self.camera.is_some() {
            for i in 0..self.scene.layer_count() {
                if let Some(layer) = self.scene.get_layer_mut(i) {
                    let _ = layer.render(&self.render_info);
                }
            }
        }

        self.pending.clear();
        self.pending_projectile_streams.clear();
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    pub fn render_info(&self) -> &RenderInfo {
        &self.render_info
    }

    pub fn stats(&self) -> &RenderBridgeStats {
        &self.stats
    }

    pub fn add_asset_search_path<P: Into<PathBuf>>(&mut self, path: P) {
        self.asset_search_paths.push(path.into());
    }

    pub fn asset_manager(&self) -> &AssetManager {
        &self.asset_manager
    }

    pub fn asset_manager_mut(&mut self) -> &mut AssetManager {
        &mut self.asset_manager
    }

    pub fn mark_model_loaded(&mut self, model_name: &str) {
        let key = model_name.to_lowercase();
        if self.model_cache.contains_key(&key) {
            return;
        }
        if let Some((render_obj, resolution)) = self.resolve_model(&key) {
            self.model_cache.insert(key.clone(), render_obj);
            self.model_resolution.insert(key, resolution);
        }
    }

    pub fn mark_models_loaded<I, S>(&mut self, model_names: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut seen = HashSet::new();
        for model_name in model_names {
            let model_name = model_name.as_ref().trim();
            if model_name.is_empty() {
                continue;
            }
            let key = model_name.to_lowercase();
            if seen.insert(key.clone()) && !self.model_cache.contains_key(&key) {
                if let Some((render_obj, resolution)) = self.resolve_model(&key) {
                    self.model_cache.insert(key.clone(), render_obj);
                    self.model_resolution.insert(key, resolution);
                }
            }
        }
    }

    pub fn is_model_loaded(&self, model_name: &str) -> bool {
        self.model_cache.contains_key(&model_name.to_lowercase())
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn clear_model_cache(&mut self) {
        self.model_cache.clear();
        self.model_resolution.clear();
    }

    /// Get a snapshot of all visible scene lines for rendering.
    ///
    /// Lines persist across frames until explicitly removed via `remove_line`.
    /// Only lines with `visible == true` are returned.
    pub fn visible_scene_lines(
        &self,
    ) -> Vec<(
        game_engine::common::system::scene_submission::SceneLineId,
        &SceneLineEntry,
    )> {
        self.scene_lines
            .iter()
            .filter(|(_, entry)| entry.visible)
            .map(|(id, entry)| (*id, entry))
            .collect()
    }
}

fn mesh_prototype_names(asset_manager: &AssetManager) -> HashSet<String> {
    asset_manager
        .prototypes()
        .filter_map(|(name, prototype)| {
            prototype
                .as_any()
                .downcast_ref::<MeshPrototype>()
                .map(|_| name.to_ascii_lowercase())
        })
        .collect()
}

fn register_newly_loaded_meshes_for_drawable_pipeline(
    asset_manager: &AssetManager,
    requested_name: &str,
    path: &Path,
    known_meshes: &HashSet<String>,
) {
    let mesh_entries = asset_manager
        .prototypes()
        .filter_map(|(key, prototype)| {
            let key_lower = key.to_ascii_lowercase();
            if known_meshes.contains(&key_lower) {
                return None;
            }
            prototype
                .as_any()
                .downcast_ref::<MeshPrototype>()
                .map(|mesh| (key.as_str(), mesh))
        })
        .collect::<Vec<_>>();
    if mesh_entries.is_empty() {
        return;
    }

    let Some((vertices, indices, texture_name)) = drawable_mesh_data_from_prototypes(&mesh_entries)
    else {
        return;
    };
    let keys = drawable_mesh_keys(requested_name, path, &mesh_entries);

    with_drawable_pipeline(|pipeline| {
        if let Ok(mut guard) = pipeline.lock() {
            for key in keys {
                guard.insert_mesh_with_texture(
                    &key,
                    vertices.clone(),
                    indices.clone(),
                    texture_name.clone(),
                );
            }
        }
    });
}

fn drawable_mesh_keys(
    requested_name: &str,
    path: &Path,
    mesh_entries: &[(&str, &MeshPrototype)],
) -> Vec<String> {
    let mut keys = Vec::new();
    push_drawable_mesh_key(&mut keys, requested_name);
    if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
        push_drawable_mesh_key(&mut keys, file_name);
    }
    if let Some(file_stem) = path.file_stem().and_then(|name| name.to_str()) {
        push_drawable_mesh_key(&mut keys, file_stem);
    }
    for (prototype_key, mesh) in mesh_entries {
        push_drawable_mesh_key(&mut keys, prototype_key);
        push_drawable_mesh_key(&mut keys, &mesh.name);
    }
    keys
}

fn push_drawable_mesh_key(keys: &mut Vec<String>, key: &str) {
    let key = key.trim();
    if key.is_empty() {
        return;
    }
    let key = key.to_ascii_lowercase();
    if !keys.iter().any(|existing| existing == &key) {
        keys.push(key);
    }
}

fn drawable_mesh_data_from_prototypes(
    mesh_entries: &[(&str, &MeshPrototype)],
) -> Option<(Vec<MeshVertex>, Vec<u32>, Option<String>)> {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut texture_name = None;

    for (_, mesh) in mesh_entries {
        let base_vertex = u32::try_from(vertices.len()).ok()?;
        vertices.extend(mesh.vertices.iter().enumerate().map(|(index, position)| {
            let normal = mesh.normals.get(index).copied().unwrap_or_default();
            let uv = mesh
                .stage_texcoords
                .first()
                .and_then(|coords| coords.get(index))
                .copied()
                .unwrap_or_default();
            MeshVertex {
                position: [position.x, position.y, position.z],
                normal: [normal.x, normal.y, normal.z],
                uv: [uv.u, uv.v],
                color: [1.0, 1.0, 1.0, 1.0],
            }
        }));

        for triangle in &mesh.triangles {
            for index in triangle.vindex {
                indices.push(base_vertex.checked_add(index)?);
            }
        }

        if texture_name.is_none() {
            texture_name = first_mesh_texture_name(mesh);
        }
    }

    if vertices.is_empty() || indices.is_empty() {
        return None;
    }
    Some((vertices, indices, texture_name))
}

fn first_mesh_texture_name(mesh: &MeshPrototype) -> Option<String> {
    mesh.textures.iter().find_map(|texture| {
        let end = texture
            .name
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(texture.name.len());
        std::str::from_utf8(&texture.name[..end])
            .ok()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
    })
}

impl RenderObjectStateSummary {
    fn from_submission(
        submission: &DrawSubmission,
        layer_name: &str,
        transparent_layer: bool,
    ) -> Self {
        let translation = submission
            .world_transform
            .transform_point3(glam::Vec3::ZERO);
        Self {
            drawable_id: submission.drawable_id.0,
            model_name: submission.model_name.clone(),
            layer_name: layer_name.to_string(),
            transparent: transparent_layer || submission.transparent,
            sort_level: submission.sort_level,
            condition_bits: submission.condition_flags.bits(),
            opacity_bits: submission.render_state.opacity.to_bits(),
            damage_overlay_bits: submission.render_state.damage_overlay.to_bits(),
            selected: submission.render_state.selected,
            night: submission.render_state.apply_night_map,
            snow: submission.render_state.apply_snow_map,
            hidden: submission.render_state.hidden,
            mesh_uv_override_count: submission.mesh_uv_overrides.len(),
            mesh_uv_overrides: submission
                .mesh_uv_overrides
                .iter()
                .map(MeshUvOverrideStateSummary::from_override)
                .collect(),
            sub_object_visibility_count: submission.sub_object_visibility.len(),
            sub_object_visibility: submission
                .sub_object_visibility
                .iter()
                .map(SubObjectVisibilityStateSummary::from_visibility)
                .collect(),
            bone_override_count: submission.bone_overrides.len(),
            animation_name: submission.animation_name.clone(),
            world_translation_bits: [
                translation.x.to_bits(),
                translation.y.to_bits(),
                translation.z.to_bits(),
            ],
            bounding_radius_bits: submission.bounding_sphere.radius.to_bits(),
        }
    }
}

impl MeshUvOverrideStateSummary {
    fn from_override(uv_override: &MeshUvOverride) -> Self {
        Self {
            mesh_name_prefix: uv_override.mesh_name_prefix.clone(),
            u_offset_bits: uv_override.u_offset.to_bits(),
            v_offset_bits: uv_override.v_offset.to_bits(),
        }
    }
}

impl SubObjectVisibilityStateSummary {
    fn from_visibility(visibility: &SubObjectVisibility) -> Self {
        Self {
            sub_object_name: visibility.sub_object_name.clone(),
            hidden: visibility.hidden,
        }
    }
}

impl ProjectileStreamStateSummary {
    fn from_submission(submission: &ProjectileStreamSubmission) -> Self {
        Self {
            drawable_id: submission.drawable_id,
            line_count: submission.lines.len(),
            point_count: submission.lines.iter().map(Vec::len).sum(),
            texture_name: submission.texture_name.clone(),
            width_bits: submission.width.to_bits(),
            tile_factor_bits: submission.tile_factor.to_bits(),
            scroll_rate_bits: submission.scroll_rate.to_bits(),
        }
    }
}

impl Default for RenderBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Adapter that wraps a `ww3d_assets::RenderObj` as a `ww3d_core::RenderObject`.
#[derive(Debug)]
struct WrapRenderObj(Box<dyn ww3d_assets::assets::RenderObj>);

impl RenderObject for WrapRenderObj {
    fn class_id(&self) -> ww3d_core::RenderObjClassId {
        ww3d_core::RenderObjClassId::Mesh
    }

    fn name(&self) -> &str {
        self.0.get_name()
    }

    fn set_name(&mut self, name: String) {
        self.0.set_name(&name);
    }

    fn clone_object(&self) -> Box<dyn RenderObject> {
        Box::new(WrapRenderObj(self.0.clone_box()))
    }

    fn render(&mut self, _info: &RenderInfo) -> ww3d_core::errors::W3DResult<()> {
        self.0.render();
        Ok(())
    }

    fn get_obj_space_bounding_sphere(&self) -> BoundingSphere {
        self.0
            .get_obj_space_bounding_sphere()
            .map(|(center, radius)| BoundingSphere::new(center, radius))
            .unwrap_or(BoundingSphere::zero())
    }

    fn get_obj_space_bounding_box(&self) -> AABox {
        self.0
            .get_obj_space_bounding_box()
            .map(|(min, max)| AABox { min, max })
            .unwrap_or(AABox {
                min: glam::Vec3::ZERO,
                max: glam::Vec3::ZERO,
            })
    }

    fn get_transform(&self) -> ww3d_core::glam::Mat4 {
        *self.0.get_transform()
    }

    fn set_transform(&mut self, transform: ww3d_core::glam::Mat4) {
        self.0.set_transform(transform);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

use game_engine::common::system::scene_submission::{
    SceneModelDesc, SceneProjectileStreamDesc, SceneSubmission as SceneSubmissionTrait,
};

impl SceneSubmissionTrait for RenderBridge {
    fn submit_line(
        &self,
        _drawable_id: u32,
        desc: &game_engine::common::system::scene_submission::SceneLineDesc,
    ) -> Option<game_engine::common::system::scene_submission::SceneLineId> {
        let mut guard = THE_RENDER_BRIDGE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(bridge) = guard.as_mut() {
            let id = NEXT_LINE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let entry = SceneLineEntry {
                start: glam::Vec3::new(desc.start.x, desc.start.y, desc.start.z),
                end: glam::Vec3::new(desc.end.x, desc.end.y, desc.end.z),
                width: desc.width,
                color: [desc.color_r, desc.color_g, desc.color_b, desc.opacity],
                texture_name: desc.texture_name.clone().unwrap_or_default(),
                tile_factor: desc.tile_factor,
                visible: desc.visible,
            };
            bridge.scene_lines.insert(id, entry);
            log::debug!(
                "SceneSubmission::submit_line drawable_id={} id={}",
                _drawable_id,
                id
            );
            return Some(id);
        }
        None
    }

    fn update_line(
        &self,
        id: game_engine::common::system::scene_submission::SceneLineId,
        desc: &game_engine::common::system::scene_submission::SceneLineDesc,
    ) {
        let mut guard = THE_RENDER_BRIDGE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(bridge) = guard.as_mut() {
            if let Some(entry) = bridge.scene_lines.get_mut(&id) {
                entry.start = glam::Vec3::new(desc.start.x, desc.start.y, desc.start.z);
                entry.end = glam::Vec3::new(desc.end.x, desc.end.y, desc.end.z);
                entry.width = desc.width;
                entry.color = [desc.color_r, desc.color_g, desc.color_b, desc.opacity];
                entry.texture_name = desc.texture_name.clone().unwrap_or_default();
                entry.tile_factor = desc.tile_factor;
                entry.visible = desc.visible;
            }
        }
        log::debug!("SceneSubmission::update_line id={}", id);
    }

    fn remove_line(&self, id: game_engine::common::system::scene_submission::SceneLineId) {
        let mut guard = THE_RENDER_BRIDGE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(bridge) = guard.as_mut() {
            bridge.scene_lines.remove(&id);
        }
        log::debug!("SceneSubmission::remove_line id={}", id);
    }

    fn submit_model(&self, desc: SceneModelDesc) {
        let mut guard = THE_RENDER_BRIDGE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(bridge) = guard.as_mut() {
            let submission = DrawSubmission::from_scene_model_desc(desc);
            bridge.submit(submission);
        }
    }

    fn submit_projectile_stream(&self, desc: SceneProjectileStreamDesc) {
        let mut guard = THE_RENDER_BRIDGE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(bridge) = guard.as_mut() {
            let ps = ProjectileStreamSubmission::from_scene_desc(desc);
            bridge.submit_projectile_stream(ps);
        }
    }

    fn begin_logic_frame(&self) {
        let mut guard = THE_RENDER_BRIDGE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(bridge) = guard.as_mut() {
            bridge.pending.clear();
            bridge.pending_projectile_streams.clear();
        }
    }

    fn end_logic_frame(&self) {}
}

impl DrawSubmission {
    fn from_scene_model_desc(desc: SceneModelDesc) -> Self {
        let condition_flags = RenderConditionFlags::from_bits_truncate(desc.condition_flags);
        let render_state = RenderStateOverrides::from_condition_flags(condition_flags);

        let world_transform = game_logic_matrix3d_to_glam(&desc.world_transform);

        let bone_overrides = desc
            .bone_overrides
            .into_iter()
            .map(|b| BoneOverride {
                bone_index: b.bone_index,
                bone_name: b.bone_name,
                transform: game_logic_matrix3d_to_glam(&b.transform),
            })
            .collect();

        let mesh_uv_overrides = desc
            .mesh_uv_overrides
            .into_iter()
            .map(|uv| MeshUvOverride {
                mesh_name_prefix: uv.mesh_name_prefix,
                u_offset: uv.u_offset,
                v_offset: uv.v_offset,
            })
            .collect();

        let bs_center = ww3d_core::glam::Vec3::new(
            desc.bounding_sphere_center.x,
            desc.bounding_sphere_center.y,
            desc.bounding_sphere_center.z,
        );

        Self {
            drawable_id: DrawableId(desc.drawable_id),
            model_name: desc.model_name,
            world_transform,
            condition_flags,
            render_state,
            bone_overrides,
            mesh_uv_overrides,
            sub_object_visibility: Vec::new(),
            animation_name: desc.animation_name,
            animation_mode: None,
            animation_time: desc.animation_time,
            bounding_sphere: BoundingSphere::new(bs_center, desc.bounding_sphere_radius),
            bounding_box: AABox::zero(),
            sort_level: desc.sort_level,
            opaque: !desc.transparent,
            transparent: desc.transparent,
            cast_shadow: desc.cast_shadow,
        }
    }
}

impl ProjectileStreamSubmission {
    fn from_scene_desc(desc: SceneProjectileStreamDesc) -> Self {
        let lines = desc
            .lines
            .into_iter()
            .map(|line| {
                line.into_iter()
                    .map(|c| glam::Vec3::new(c.x, c.y, c.z))
                    .collect()
            })
            .collect();

        Self {
            drawable_id: desc.drawable_id,
            lines,
            texture_name: desc.texture_name,
            width: desc.width,
            tile_factor: desc.tile_factor,
            scroll_rate: desc.scroll_rate,
        }
    }
}

fn game_logic_matrix3d_to_glam(m: &game_engine::common::system::geometry::Matrix3D) -> glam::Mat4 {
    let mut cols = [0.0f32; 16];
    for i in 0..4 {
        for j in 0..4 {
            cols[j * 4 + i] = m.m[i][j];
        }
    }
    glam::Mat4::from_cols_array(&cols)
}

// Global singleton instance
use std::sync::Mutex;
lazy_static::lazy_static! {
    pub static ref THE_RENDER_BRIDGE: Mutex<Option<RenderBridge>> = Mutex::new(None);
}

pub fn init_render_bridge() {
    let mut guard = THE_RENDER_BRIDGE.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(RenderBridge::new());
}

pub fn get_render_bridge() -> &'static Mutex<Option<RenderBridge>> {
    &THE_RENDER_BRIDGE
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn distance_sq_to_camera(submission: &DrawSubmission, camera: &Camera) -> f32 {
    let obj_pos = submission
        .world_transform
        .transform_point3(glam::Vec3::ZERO);
    let cam_pos = ww_to_game_vec3(camera.position());
    (obj_pos - cam_pos).length_squared()
}

fn stable_render_fingerprint(
    stats: &RenderBridgeStats,
    objects: &[RenderObjectStateSummary],
    projectile_streams: &[ProjectileStreamStateSummary],
) -> u64 {
    let mut hash = Fnv1a64::new();
    hash.write_usize(stats.submissions_received);
    hash.write_usize(stats.culled);
    hash.write_usize(stats.rendered);
    hash.write_usize(stats.opaque_draws);
    hash.write_usize(stats.transparent_draws);
    hash.write_usize(stats.hidden);

    for object in objects {
        hash.write_u32(object.drawable_id);
        hash.write_str(&object.model_name);
        hash.write_str(&object.layer_name);
        hash.write_bool(object.transparent);
        hash.write_i32(object.sort_level);
        hash.write_u64(object.condition_bits);
        hash.write_u32(object.opacity_bits);
        hash.write_u32(object.damage_overlay_bits);
        hash.write_bool(object.selected);
        hash.write_bool(object.night);
        hash.write_bool(object.snow);
        hash.write_bool(object.hidden);
        hash.write_usize(object.mesh_uv_override_count);
        for uv_override in &object.mesh_uv_overrides {
            hash.write_str(&uv_override.mesh_name_prefix);
            hash.write_u32(uv_override.u_offset_bits);
            hash.write_u32(uv_override.v_offset_bits);
        }
        hash.write_usize(object.sub_object_visibility_count);
        for visibility in &object.sub_object_visibility {
            hash.write_str(&visibility.sub_object_name);
            hash.write_bool(visibility.hidden);
        }
        hash.write_usize(object.bone_override_count);
        if let Some(animation_name) = &object.animation_name {
            hash.write_bool(true);
            hash.write_str(animation_name);
        } else {
            hash.write_bool(false);
        }
        for bits in object.world_translation_bits {
            hash.write_u32(bits);
        }
        hash.write_u32(object.bounding_radius_bits);
    }

    for stream in projectile_streams {
        hash.write_u32(stream.drawable_id);
        hash.write_usize(stream.line_count);
        hash.write_usize(stream.point_count);
        hash.write_str(&stream.texture_name);
        hash.write_u32(stream.width_bits);
        hash.write_u32(stream.tile_factor_bits);
        hash.write_u32(stream.scroll_rate_bits);
    }

    hash.finish()
}

struct Fnv1a64(u64);

impl Fnv1a64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    fn write_bool(&mut self, value: bool) {
        self.write_bytes(&[value as u8]);
    }

    fn write_i32(&mut self, value: i32) {
        self.write_bytes(&value.to_le_bytes());
    }

    fn write_u32(&mut self, value: u32) {
        self.write_bytes(&value.to_le_bytes());
    }

    fn write_u64(&mut self, value: u64) {
        self.write_bytes(&value.to_le_bytes());
    }

    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }

    fn write_str(&mut self, value: &str) {
        self.write_usize(value.len());
        self.write_bytes(value.as_bytes());
    }

    fn finish(self) -> u64 {
        self.0
    }
}

#[inline]
fn ww_to_game_vec3(v: ww3d_core::glam::Vec3) -> glam::Vec3 {
    glam::Vec3::new(v.x, v.y, v.z)
}

#[inline]
fn game_to_ww_vec3(v: glam::Vec3) -> ww3d_core::glam::Vec3 {
    ww3d_core::glam::Vec3::new(v.x, v.y, v.z)
}

#[inline]
fn glam_to_ww_mat4(m: glam::Mat4) -> ww3d_core::glam::Mat4 {
    let cols = m.to_cols_array();
    ww3d_core::glam::Mat4::from_cols_array(&cols)
}

#[inline]
fn ww_to_glam_mat4(m: ww3d_core::glam::Mat4) -> glam::Mat4 {
    let cols = m.to_cols_array();
    glam::Mat4::from_cols_array(&cols)
}

#[inline]
pub fn game_logic_to_wwvegas_transform(m: glam::Mat4) -> glam::Mat4 {
    m
}

#[inline]
pub fn game_logic_to_wwvegas_vec3(v: glam::Vec3) -> glam::Vec3 {
    v
}

pub fn apply_render_state_to_material(
    _material: &mut VertexMaterial,
    _overrides: &RenderStateOverrides,
) {
}

pub fn projectile_stream_to_flat_points(
    submission: &ProjectileStreamSubmission,
) -> Vec<glam::Vec3> {
    let zero = glam::Vec3::ZERO;
    let mut flat = Vec::new();
    for (i, line) in submission.lines.iter().enumerate() {
        if i > 0 {
            flat.push(zero);
        }
        flat.extend_from_slice(line);
    }
    flat
}

pub fn create_default_game_scene() -> Scene {
    SceneBuilder::new("Game World".to_string()).build()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Mat4 as GameMat4, Vec3 as GameVec3};
    use ww3d_core::glam::{Mat4 as WwMat4, Vec3 as WwVec3};

    fn register_test_model(bridge: &mut RenderBridge, name: &str) {
        bridge.asset_manager_mut().add_prototype(
            name.to_string(),
            Box::new(ww3d_assets::prototypes::MeshPrototype::new(
                name.to_string(),
            )),
        );
    }

    #[test]
    fn test_render_condition_flags_from_empty() {
        let flags = RenderConditionFlags::empty();
        let overrides = RenderStateOverrides::from_condition_flags(flags);
        assert!(!overrides.hidden);
        assert!(!overrides.selected);
        assert!(!overrides.apply_night_map);
        assert!(!overrides.apply_snow_map);
        assert!((overrides.damage_overlay - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_render_condition_flags_night() {
        let flags = RenderConditionFlags::NIGHT;
        let overrides = RenderStateOverrides::from_condition_flags(flags);
        assert!(overrides.apply_night_map);
        assert!(!overrides.apply_snow_map);
    }

    #[test]
    fn test_render_condition_flags_snow() {
        let flags = RenderConditionFlags::SNOW;
        let overrides = RenderStateOverrides::from_condition_flags(flags);
        assert!(overrides.apply_snow_map);
        assert!(!overrides.apply_night_map);
    }

    #[test]
    fn test_render_condition_flags_damaged() {
        let flags = RenderConditionFlags::DAMAGED;
        let overrides = RenderStateOverrides::from_condition_flags(flags);
        assert!((overrides.damage_overlay - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_render_condition_flags_really_damaged() {
        let flags = RenderConditionFlags::REALLY_DAMAGED;
        let overrides = RenderStateOverrides::from_condition_flags(flags);
        assert!((overrides.damage_overlay - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_render_condition_flags_awaiting_construction_hidden() {
        let flags = RenderConditionFlags::AWAITING_CONSTRUCTION;
        let overrides = RenderStateOverrides::from_condition_flags(flags);
        assert!(overrides.hidden);
    }

    #[test]
    fn test_render_condition_flags_selected() {
        let flags = RenderConditionFlags::SELECTED;
        let overrides = RenderStateOverrides::from_condition_flags(flags);
        assert!(overrides.selected);
    }

    #[test]
    fn test_render_condition_flags_aflame_emissive() {
        let flags = RenderConditionFlags::AFLAME;
        let overrides = RenderStateOverrides::from_condition_flags(flags);
        assert!((overrides.emissive_tint[0] - 1.0).abs() < f32::EPSILON);
        assert!((overrides.emissive_tint[1] - 0.4).abs() < 0.01);
    }

    #[test]
    fn test_bridge_submit_and_flush() {
        let mut bridge = RenderBridge::new();
        register_test_model(&mut bridge, "AVComanche");
        let mut camera = Camera::perspective(
            "test".to_string(),
            60.0_f32.to_radians(),
            16.0 / 9.0,
            0.1,
            1000.0,
        );
        camera.set_position(WwVec3::new(0.0, 50.0, -100.0));
        camera.look_at(WwVec3::ZERO, WwVec3::Y);

        bridge.begin_frame(&camera, 0.016);

        let submission = DrawSubmission {
            drawable_id: DrawableId(1),
            model_name: "AVComanche".to_string(),
            world_transform: GameMat4::IDENTITY,
            condition_flags: RenderConditionFlags::PRISTINE,
            render_state: RenderStateOverrides::default(),
            bounding_sphere: BoundingSphere::new(WwVec3::ZERO, 10.0),
            bounding_box: AABox::new(WwVec3::new(-5.0, 0.0, -5.0), WwVec3::new(5.0, 10.0, 5.0)),
            opaque: true,
            transparent: false,
            cast_shadow: true,
            ..Default::default()
        };
        bridge.submit(submission);

        let hidden = DrawSubmission {
            drawable_id: DrawableId(2),
            model_name: "Scaffold".to_string(),
            render_state: RenderStateOverrides {
                hidden: true,
                ..Default::default()
            },
            ..Default::default()
        };
        bridge.submit(hidden);

        bridge.flush();

        let stats = bridge.stats();
        assert_eq!(stats.submissions_received, 2);
        assert_eq!(stats.hidden, 1);
        assert_eq!(stats.culled, 0);
        assert_eq!(stats.rendered, 1);
        assert_eq!(stats.opaque_draws, 1);
    }

    #[test]
    fn test_bridge_frustum_culling() {
        let mut bridge = RenderBridge::new();
        let mut camera = Camera::perspective(
            "test".to_string(),
            60.0_f32.to_radians(),
            16.0 / 9.0,
            0.1,
            100.0,
        );
        camera.set_position(WwVec3::new(0.0, 10.0, 0.0));
        camera.look_at(WwVec3::new(0.0, 10.0, 1.0), WwVec3::Y);

        bridge.begin_frame(&camera, 0.016);

        let near = DrawSubmission {
            drawable_id: DrawableId(1),
            bounding_sphere: BoundingSphere::new(WwVec3::ZERO, 1.0),
            opaque: true,
            ..Default::default()
        };
        bridge.submit(near);

        let far = DrawSubmission {
            drawable_id: DrawableId(2),
            bounding_sphere: BoundingSphere::new(WwVec3::ZERO, 1.0),
            world_transform: GameMat4::from_translation(GameVec3::new(0.0, 10.0, 500.0)),
            opaque: true,
            ..Default::default()
        };
        bridge.submit(far);

        bridge.flush();

        let stats = bridge.stats();
        assert_eq!(stats.submissions_received, 2);
        assert!(stats.culled >= 1);
    }

    #[test]
    fn test_draw_submission_default() {
        let s = DrawSubmission::default();
        assert_eq!(s.drawable_id, DrawableId(0));
        assert!(s.model_name.is_empty());
        assert!(!s.transparent);
        assert!(s.opaque);
    }

    #[test]
    fn test_model_cache() {
        let mut bridge = RenderBridge::new();
        assert!(!bridge.is_model_loaded("TestModel"));
        bridge.mark_model_loaded("TestModel");
        assert!(!bridge.is_model_loaded("TestModel"));
        assert!(!bridge.is_model_loaded("testmodel"));
        bridge.clear_model_cache();
        assert!(!bridge.is_model_loaded("TestModel"));
    }

    #[test]
    fn drained_submissions_skip_unresolved_models_like_cpp() {
        let mut bridge = RenderBridge::new();
        let mut camera = Camera::perspective(
            "fallback_scene".to_string(),
            60.0_f32.to_radians(),
            16.0 / 9.0,
            0.1,
            1000.0,
        );
        camera.set_position(WwVec3::new(0.0, 50.0, -100.0));
        camera.look_at(WwVec3::ZERO, WwVec3::Y);

        bridge.begin_frame(&camera, 0.016);
        bridge.submit(DrawSubmission {
            drawable_id: DrawableId(1),
            model_name: "MissingModel".to_string(),
            bounding_sphere: BoundingSphere::new(WwVec3::ZERO, 10.0),
            opaque: true,
            ..Default::default()
        });
        bridge.flush();

        let drained = bridge.drain_scene_submissions();
        assert!(drained.is_empty());
        assert!(!bridge.is_model_loaded("MissingModel"));
    }

    #[test]
    fn drained_submissions_identify_asset_model_resolution_like_cpp() {
        let mut bridge = RenderBridge::new();
        bridge.asset_manager_mut().add_prototype(
            "realmodel".to_string(),
            Box::new(ww3d_assets::prototypes::MeshPrototype::new(
                "RealModel".to_string(),
            )),
        );

        let mut camera = Camera::perspective(
            "asset_scene".to_string(),
            60.0_f32.to_radians(),
            16.0 / 9.0,
            0.1,
            1000.0,
        );
        camera.set_position(WwVec3::new(0.0, 50.0, -100.0));
        camera.look_at(WwVec3::ZERO, WwVec3::Y);

        bridge.begin_frame(&camera, 0.016);
        bridge.submit(DrawSubmission {
            drawable_id: DrawableId(1),
            model_name: "RealModel".to_string(),
            bounding_sphere: BoundingSphere::new(WwVec3::ZERO, 10.0),
            opaque: true,
            ..Default::default()
        });
        bridge.flush();

        let drained = bridge.drain_scene_submissions();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].model_resolution, Some(ModelResolution::Asset));

        bridge.clear_model_cache();
        assert!(!bridge.is_model_loaded("RealModel"));
        bridge.begin_frame(&camera, 0.016);
        bridge.submit(DrawSubmission {
            drawable_id: DrawableId(2),
            model_name: "RealModel".to_string(),
            bounding_sphere: BoundingSphere::new(WwVec3::ZERO, 10.0),
            opaque: true,
            ..Default::default()
        });
        bridge.flush();
        let drained_after_clear = bridge.drain_scene_submissions();
        assert_eq!(
            drained_after_clear[0].model_resolution,
            Some(ModelResolution::Asset)
        );
    }

    #[test]
    fn drawable_mesh_keys_cover_requested_file_and_loaded_prototype_names() {
        let mut mesh = ww3d_assets::prototypes::MeshPrototype::new("TankBody".to_string());
        let entries = vec![("GUTank_Body", &mesh)];
        let keys = drawable_mesh_keys("GUTank", std::path::Path::new("Art/GUTank.w3d"), &entries);

        assert_eq!(
            keys,
            vec![
                "gutank".to_string(),
                "gutank.w3d".to_string(),
                "gutank_body".to_string(),
                "tankbody".to_string(),
            ]
        );

        mesh.name = "GUTank".to_string();
        let entries = vec![("GUTank", &mesh)];
        let deduped = drawable_mesh_keys("GUTank", std::path::Path::new("GUTank.w3d"), &entries);
        assert_eq!(
            deduped,
            vec!["gutank".to_string(), "gutank.w3d".to_string()]
        );
    }

    #[test]
    fn drawable_mesh_data_from_prototypes_concatenates_real_w3d_geometry() {
        use ww3d_core::{W3dTexCoordStruct, W3dTriangleStruct, W3dVectorStruct};

        let mut body = ww3d_assets::prototypes::MeshPrototype::new("Body".to_string());
        body.vertices = vec![
            W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            W3dVectorStruct {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            W3dVectorStruct {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
        ];
        body.normals = vec![
            W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 1.0
            };
            3
        ];
        body.stage_texcoords = vec![vec![
            W3dTexCoordStruct { u: 0.0, v: 0.0 },
            W3dTexCoordStruct { u: 1.0, v: 0.0 },
            W3dTexCoordStruct { u: 0.0, v: 1.0 },
        ]];
        body.triangles = vec![W3dTriangleStruct {
            vindex: [0, 1, 2],
            attributes: 0,
            normal: W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            distance: 0.0,
        }];

        let mut turret = ww3d_assets::prototypes::MeshPrototype::new("Turret".to_string());
        turret.vertices = vec![
            W3dVectorStruct {
                x: 2.0,
                y: 0.0,
                z: 0.0,
            },
            W3dVectorStruct {
                x: 3.0,
                y: 0.0,
                z: 0.0,
            },
            W3dVectorStruct {
                x: 2.0,
                y: 1.0,
                z: 0.0,
            },
        ];
        turret.triangles = vec![W3dTriangleStruct {
            vindex: [0, 1, 2],
            attributes: 0,
            normal: W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            distance: 0.0,
        }];

        let entries = vec![("Body", &body), ("Turret", &turret)];
        let (vertices, indices, texture_name) =
            drawable_mesh_data_from_prototypes(&entries).expect("mesh data should convert");

        assert_eq!(vertices.len(), 6);
        assert_eq!(indices, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(vertices[1].position, [1.0, 0.0, 0.0]);
        assert_eq!(vertices[1].normal, [0.0, 0.0, 1.0]);
        assert_eq!(vertices[1].uv, [1.0, 0.0]);
        assert_eq!(vertices[4].position, [3.0, 0.0, 0.0]);
        assert_eq!(texture_name, None);
    }

    #[test]
    fn test_render_state_summary_for_fixed_gameplay_scene() {
        let mut bridge = RenderBridge::new();
        register_test_model(&mut bridge, "AVTank");
        register_test_model(&mut bridge, "ABPowerPlant");
        let mut camera = Camera::perspective(
            "fixed_scene".to_string(),
            60.0_f32.to_radians(),
            16.0 / 9.0,
            0.1,
            1000.0,
        );
        camera.set_position(WwVec3::new(0.0, 80.0, -180.0));
        camera.look_at(WwVec3::new(0.0, 0.0, 0.0), WwVec3::Y);

        bridge.begin_frame(&camera, 1.0 / 30.0);

        let tank_flags = RenderConditionFlags::PRISTINE
            | RenderConditionFlags::MOVING
            | RenderConditionFlags::SELECTED
            | RenderConditionFlags::NIGHT;
        bridge.submit(DrawSubmission {
            drawable_id: DrawableId(100),
            model_name: "AVTank".to_string(),
            world_transform: GameMat4::from_translation(GameVec3::new(0.0, 0.0, 60.0)),
            condition_flags: tank_flags,
            render_state: RenderStateOverrides::from_condition_flags(tank_flags),
            mesh_uv_overrides: vec![
                MeshUvOverride {
                    mesh_name_prefix: "TREADSL".to_string(),
                    u_offset: 0.25,
                    v_offset: 0.0,
                },
                MeshUvOverride {
                    mesh_name_prefix: "TREADSR".to_string(),
                    u_offset: 0.25,
                    v_offset: 0.0,
                },
            ],
            sub_object_visibility: vec![
                SubObjectVisibility {
                    sub_object_name: "muzzleflash01".to_string(),
                    hidden: true,
                },
                SubObjectVisibility {
                    sub_object_name: "payloadcrate".to_string(),
                    hidden: false,
                },
            ],
            bounding_sphere: BoundingSphere::new(WwVec3::ZERO, 12.0),
            bounding_box: AABox::new(WwVec3::new(-6.0, 0.0, -8.0), WwVec3::new(6.0, 8.0, 8.0)),
            opaque: true,
            transparent: false,
            cast_shadow: true,
            ..Default::default()
        });

        let damaged_flags = RenderConditionFlags::DAMAGED | RenderConditionFlags::SNOW;
        bridge.submit(DrawSubmission {
            drawable_id: DrawableId(101),
            model_name: "ABPowerPlant".to_string(),
            world_transform: GameMat4::from_translation(GameVec3::new(40.0, 0.0, 90.0)),
            condition_flags: damaged_flags,
            render_state: RenderStateOverrides::from_condition_flags(damaged_flags),
            bounding_sphere: BoundingSphere::new(WwVec3::ZERO, 20.0),
            opaque: true,
            transparent: false,
            cast_shadow: true,
            ..Default::default()
        });

        bridge.flush();

        let summary = bridge.render_state_summary();
        assert_eq!(summary.stats.submissions_received, 2);
        assert_eq!(summary.stats.rendered, 2);
        assert_eq!(summary.objects.len(), 2);
        assert_eq!(summary.projectile_streams.len(), 0);
        assert_eq!(summary.objects[0].drawable_id, 100);
        assert_eq!(summary.objects[0].mesh_uv_override_count, 2);
        assert_eq!(
            summary.objects[0].mesh_uv_overrides,
            vec![
                MeshUvOverrideStateSummary {
                    mesh_name_prefix: "TREADSL".to_string(),
                    u_offset_bits: 0.25_f32.to_bits(),
                    v_offset_bits: 0.0_f32.to_bits(),
                },
                MeshUvOverrideStateSummary {
                    mesh_name_prefix: "TREADSR".to_string(),
                    u_offset_bits: 0.25_f32.to_bits(),
                    v_offset_bits: 0.0_f32.to_bits(),
                },
            ]
        );
        assert_eq!(summary.objects[0].sub_object_visibility_count, 2);
        assert_eq!(
            summary.objects[0].sub_object_visibility,
            vec![
                SubObjectVisibilityStateSummary {
                    sub_object_name: "muzzleflash01".to_string(),
                    hidden: true,
                },
                SubObjectVisibilityStateSummary {
                    sub_object_name: "payloadcrate".to_string(),
                    hidden: false,
                },
            ]
        );
        assert!(summary.objects[0].selected);
        assert!(summary.objects[0].night);
        assert_eq!(summary.objects[1].drawable_id, 101);
        assert!(summary.objects[1].snow);
        assert_eq!(summary.fingerprint, 0x9a2e2a9a0a0e585e);
    }

    #[test]
    fn test_render_state_summary_tracks_hidden_shroud_and_fx_streams() {
        let mut bridge = RenderBridge::new();
        register_test_model(&mut bridge, "FXExplosion");
        let mut camera = Camera::perspective(
            "fx_scene".to_string(),
            60.0_f32.to_radians(),
            4.0 / 3.0,
            0.1,
            1000.0,
        );
        camera.set_position(WwVec3::new(0.0, 40.0, -100.0));
        camera.look_at(WwVec3::new(0.0, 0.0, 0.0), WwVec3::Y);

        bridge.begin_frame(&camera, 1.0 / 30.0);

        let hidden_flags = RenderConditionFlags::AWAITING_CONSTRUCTION;
        bridge.submit(DrawSubmission {
            drawable_id: DrawableId(200),
            model_name: "ShroudCell".to_string(),
            condition_flags: hidden_flags,
            render_state: RenderStateOverrides::from_condition_flags(hidden_flags),
            bounding_sphere: BoundingSphere::new(WwVec3::ZERO, 8.0),
            ..Default::default()
        });

        let fire_flags = RenderConditionFlags::AFLAME;
        bridge.submit(DrawSubmission {
            drawable_id: DrawableId(201),
            model_name: "FXExplosion".to_string(),
            world_transform: GameMat4::from_translation(GameVec3::new(-20.0, 0.0, 40.0)),
            condition_flags: fire_flags,
            render_state: RenderStateOverrides::from_condition_flags(fire_flags),
            bounding_sphere: BoundingSphere::new(WwVec3::ZERO, 6.0),
            opaque: false,
            transparent: true,
            cast_shadow: false,
            ..Default::default()
        });

        RenderBridge::submit_projectile_stream(
            &mut bridge,
            ProjectileStreamSubmission {
                drawable_id: 202,
                lines: vec![
                    vec![GameVec3::new(0.0, 0.0, 0.0), GameVec3::new(8.0, 0.0, 0.0)],
                    vec![GameVec3::new(8.0, 0.0, 0.0), GameVec3::new(12.0, 4.0, 0.0)],
                ],
                texture_name: "EXLaser".to_string(),
                width: 2.5,
                tile_factor: 1.25,
                scroll_rate: 0.75,
            },
        );

        bridge.flush();

        let summary = bridge.render_state_summary();
        assert_eq!(summary.stats.submissions_received, 2);
        assert_eq!(summary.stats.hidden, 1);
        assert_eq!(summary.stats.rendered, 1);
        assert_eq!(summary.stats.transparent_draws, 1);
        assert_eq!(summary.objects.len(), 1);
        assert_eq!(summary.objects[0].drawable_id, 201);
        assert!(summary.objects[0].transparent);
        assert_eq!(summary.projectile_streams.len(), 1);
        assert_eq!(summary.projectile_streams[0].drawable_id, 202);
        assert_eq!(summary.projectile_streams[0].line_count, 2);
        assert_eq!(summary.projectile_streams[0].point_count, 4);
        assert_eq!(summary.fingerprint, 0x91b4d089c7211e84);
    }

    #[test]
    fn test_create_default_game_scene() {
        let scene = create_default_game_scene();
        assert_eq!(scene.name(), "Game World");
    }

    #[test]
    fn test_game_logic_to_wwvegas_transform_identity() {
        let m = GameMat4::IDENTITY;
        assert_eq!(game_logic_to_wwvegas_transform(m), GameMat4::IDENTITY);
    }

    #[test]
    fn test_game_logic_to_wwvegas_vec3() {
        let v = GameVec3::new(1.0, 2.0, 3.0);
        let converted = game_logic_to_wwvegas_vec3(v);
        assert!((converted.x - 1.0).abs() < f32::EPSILON);
        assert!((converted.y - 2.0).abs() < f32::EPSILON);
        assert!((converted.z - 3.0).abs() < f32::EPSILON);
    }
}
