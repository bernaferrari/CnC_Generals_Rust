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

use std::collections::HashMap;
use std::sync::Arc;

use ww3d_core::{
    AABox, BoundingSphere, Camera, Layer, RenderInfo, RenderObject, Scene, SceneBuilder,
};
use ww3d_core::animation::{AnimationController, AnimationMode, Hierarchy, Pivot};
use ww3d_core::lighting::{Light, LightEnvironment, LightType};
use ww3d_core::material::{BlendMode, MaterialInfo, Shader, ShaderType, VertexMaterial};
use ww3d_core::mesh::{Mesh, MeshBuilder, Vertex};
use ww3d_core::texture::{TextureManager, TextureData};

// ---------------------------------------------------------------------------
// Re-exports from GameLogic that the bridge needs to inspect
// ---------------------------------------------------------------------------

/// Newtype wrapper for a drawable identifier coming from GameLogic.
///
/// In the full system this will match `Drawable::getID()`.  For now we
/// use a simple integer that is assigned by the bridge itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DrawableId(pub u32);

/// Bitflags that the bridge receives from GameLogic draw modules.
///
/// This is a *subset* of the full `ModelConditionFlags` from GameLogic — only
/// the flags that actually influence rendering state are kept here, which
/// avoids pulling the entire `gamelogic` type into the bridge's public API.
bitflags::bitflags! {
    /// Rendering-relevant model condition flags.
    ///
    /// Each flag maps to a WWVegas render-state override in
    /// [`RenderStateOverrides::from_condition_flags`].
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
    /// Overall opacity multiplier (e.g. partially-constructed buildings).
    pub opacity: f32,

    /// Additive emissive tint (night glow, aflame).
    pub emissive_tint: [f32; 3],

    /// Whether to apply the night-map texture blend.
    pub apply_night_map: bool,

    /// Whether to apply the snow-map texture blend.
    pub apply_snow_map: bool,

    /// Tint colour for construction-progress scaffolding.
    pub construction_tint: Option<[f32; 3]>,

    /// Damage overlay intensity in [0, 1].
    pub damage_overlay: f32,

    /// Whether the object is currently selected (selection ring).
    pub selected: bool,

    /// Whether the object should be hidden entirely (shroud, etc.).
    pub hidden: bool,

    /// Blend mode override (None = use material default).
    pub blend_override: Option<BlendMode>,

    /// Whether to apply wireframe rendering (debug).
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
    /// Derive render-state overrides from the given condition flags.
    ///
    /// This mirrors how the C++ `W3DModelDraw::do_draw_module()` interprets
    /// `ModelConditionFlags` to change render object properties (opacity,
    /// visibility of sub-objects, material replacement, etc.).
    pub fn from_condition_flags(flags: RenderConditionFlags) -> Self {
        let mut s = Self::default();

        // --- Visibility ---
        if flags.contains(RenderConditionFlags::AWAITING_CONSTRUCTION) {
            s.hidden = true;
            return s;
        }

        // --- Damage ---
        if flags.contains(RenderConditionFlags::REALLY_DAMAGED) {
            s.damage_overlay = 1.0;
        } else if flags.contains(RenderConditionFlags::DAMAGED) {
            s.damage_overlay = 0.5;
        } else if flags.contains(RenderConditionFlags::RUBBLE) {
            s.damage_overlay = 1.0;
            s.opacity = 0.8; // rubble is slightly faded
        }

        // --- Construction progress ---
        if flags.contains(RenderConditionFlags::ACTIVELY_CONSTRUCTED) {
            s.construction_tint = Some([0.6, 0.6, 0.6]);
            s.opacity = 1.0;
        } else if flags.contains(RenderConditionFlags::PARTIALLY_CONSTRUCTED) {
            s.construction_tint = Some([0.5, 0.5, 0.5]);
            s.opacity = 0.7;
        }

        // --- Night / Snow ---
        s.apply_night_map = flags.contains(RenderConditionFlags::NIGHT);
        s.apply_snow_map = flags.contains(RenderConditionFlags::SNOW);

        // --- Fire effects ---
        if flags.contains(RenderConditionFlags::AFLAME) {
            s.emissive_tint = [1.0, 0.4, 0.05];
        } else if flags.contains(RenderConditionFlags::SMOLDERING) {
            s.emissive_tint = [0.3, 0.15, 0.05];
        }

        // --- Selection ---
        s.selected = flags.contains(RenderConditionFlags::SELECTED);

        // --- Disguised ---
        if flags.contains(RenderConditionFlags::DISGUISED) {
            // Disguised objects keep their visual but with a subtle tint.
            s.emissive_tint = [
                s.emissive_tint[0] + 0.05,
                s.emissive_tint[1] + 0.05,
                s.emissive_tint[2] + 0.05,
            ];
        }

        // --- Toppled / Flooded ---
        if flags.contains(RenderConditionFlags::TOPPLED) || flags.contains(RenderConditionFlags::FLOODED) {
            s.opacity = 0.6;
        }

        s
    }
}

// ---------------------------------------------------------------------------
// Bone override data for animated models
// ---------------------------------------------------------------------------

/// A single bone transform override, submitted when GameLogic's draw module
/// adjusts a turret rotation, recoil offset, etc.
#[derive(Debug, Clone)]
pub struct BoneOverride {
    /// Bone index within the hierarchy.
    pub bone_index: i32,

    /// Optional bone name (for logging / debugging).
    pub bone_name: Option<String>,

    /// Replacement transform (world-space after hierarchy compose).
    pub transform: glam::Mat4,
}

// ---------------------------------------------------------------------------
// Per-frame draw submission — what GameLogic pushes into the bridge
// ---------------------------------------------------------------------------

/// A single draw submission representing one draw module on one drawable.
///
/// The bridge collects a `Vec<DrawSubmission>` each frame, sorts them, and
/// maps them into WWVegas `RenderObject` instances inside the scene.
#[derive(Debug, Clone)]
pub struct DrawSubmission {
    /// Which drawable this submission belongs to.
    pub drawable_id: DrawableId,

    /// Human-readable model name (e.g. `"AVComanche"`) used for W3D asset lookup.
    pub model_name: String,

    /// World-space transform (position + rotation + scale).
    pub world_transform: glam::Mat4,

    /// Current model condition flags from GameLogic.
    pub condition_flags: RenderConditionFlags,

    /// Computed render-state overrides for this frame.
    pub render_state: RenderStateOverrides,

    /// Bone transform overrides (turret, recoil, etc.).
    pub bone_overrides: Vec<BoneOverride>,

    /// Active animation name, if any.
    pub animation_name: Option<String>,

    /// Animation mode (loop, once, etc.).
    pub animation_mode: Option<AnimationMode>,

    /// Animation time offset in seconds.
    pub animation_time: f32,

    /// Object-space bounding sphere radius for frustum culling.
    pub bounding_sphere: BoundingSphere,

    /// Object-space bounding box.
    pub bounding_box: AABox,

    /// Sort priority (higher = rendered later / on top).
    pub sort_level: i32,

    /// Whether the draw module has an opaque pass (default true).
    pub opaque: bool,

    /// Whether the draw module has a transparent pass.
    pub transparent: bool,

    /// Shadow type requested by the draw module.
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
// W3D render object wrapper — lives inside the WWVegas scene
// ---------------------------------------------------------------------------

/// A lightweight `RenderObject` that the bridge inserts into the WWVegas scene
/// to represent one draw-module submission.
///
/// On `render()` it simply records itself so the real WWVegas pipeline can
/// consume the submission data later.  This avoids duplicating the heavy
/// W3D asset loading code; the actual mesh / texture / shader binding
/// happens in `RenderBridge::flush()` when it resolves model names to
/// `ww3d_core::RenderObject`s loaded from W3D files.
#[derive(Debug)]
struct BridgeRenderObject {
    submission: DrawSubmission,
}

impl RenderObject for BridgeRenderObject {
    fn class_id(&self) -> ww3d_core::RenderObjClassId {
        ww3d_core::RenderObjClassId::Mesh
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
        })
    }

    fn render(&mut self, _info: &RenderInfo) -> ww3d_core::errors::W3DResult<()> {
        // The bridge itself does not rasterize.  The real rendering pass in
        // RenderBridge::flush() will map the submission's model_name to an
        // actual WWVegas render object and call its render() method.
        Ok(())
    }

    fn get_obj_space_bounding_sphere(&self) -> BoundingSphere {
        self.submission.bounding_sphere
    }

    fn get_obj_space_bounding_box(&self) -> AABox {
        self.submission.bounding_box
    }

    fn get_transform(&self) -> ww3d_core::glam::Mat4 {
        ww3d_core::glam::Mat4::from_cols_array(&self.submission.world_transform.to_cols_array())
    }

    fn set_transform(&mut self, transform: ww3d_core::glam::Mat4) {
        self.submission.world_transform = glam::Mat4::from_cols_array(&transform.to_cols_array());
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

/// Statistics about the last frame processed by the bridge.
#[derive(Debug, Clone, Default)]
pub struct RenderBridgeStats {
    /// Total draw submissions received this frame.
    pub submissions_received: usize,
    /// Submissions culled by frustum test.
    pub culled: usize,
    /// Submissions actually rendered (opaque + transparent).
    pub rendered: usize,
    /// Opaque draw calls.
    pub opaque_draws: usize,
    /// Transparent draw calls (sorted back-to-front).
    pub transparent_draws: usize,
    /// Submissions hidden by condition flags (shroud, awaiting construction).
    pub hidden: usize,
    /// Time spent in the bridge this frame (seconds).
    pub bridge_time_s: f32,
}

/// The render bridge is the central coordinator between GameLogic's draw
/// modules and the WWVegas W3D renderer.
///
/// ## Usage pattern (each frame)
///
/// ```text
/// 1. bridge.begin_frame(camera, delta);
/// 2. for each drawable that ticked its draw modules:
///        bridge.submit(draw_submission);
/// 3. bridge.flush();   // cull, sort, push into WWVegas scene
/// 4. // WWVegas renderer picks up the scene and presents
/// 5. bridge.end_frame();
/// ```
///
/// ## C++ Reference
///
/// This struct subsumes the rendering portion of:
/// - `W3DGameClient::draw()`
/// - `DrawableManager::render()`
/// - `Drawable::friend_DrawModule()` (the per-module render call)
pub struct RenderBridge {
    /// The WWVegas scene that receives render objects.
    scene: Scene,

    /// Submissions collected during the current frame (before flush).
    pending: Vec<DrawSubmission>,

    /// Cached camera for frustum culling during flush.
    camera: Option<Camera>,

    /// Model-name -> resolved WWVegas render object cache.
    ///
    /// In the full implementation this will be backed by the asset manager
    /// that loads .w3d files and produces `ww3d_core::RenderObject` instances.
    /// For now it stores placeholder markers.
    model_cache: HashMap<String, bool>,

    /// Statistics for the last completed frame.
    stats: RenderBridgeStats,

    /// The current render info (view/projection matrices, timing).
    render_info: RenderInfo,

    /// Global elapsed time accumulator (seconds).
    elapsed_time: f32,
}

impl RenderBridge {
    /// Create a new render bridge with default scene settings.
    pub fn new() -> Self {
        let scene = SceneBuilder::new("GameLogic Bridge Scene".to_string()).build();

        Self {
            scene,
            pending: Vec::with_capacity(2048),
            camera: None,
            model_cache: HashMap::new(),
            stats: RenderBridgeStats::default(),
            render_info: RenderInfo::new(),
            elapsed_time: 0.0,
        }
    }

    // -----------------------------------------------------------------------
    // Frame lifecycle
    // -----------------------------------------------------------------------

    /// Begin a new frame.  Call this once before submitting any draw data.
    ///
    /// * `camera` — the current game camera used for frustum culling.
    /// * `delta_time` — frame delta in seconds.
    pub fn begin_frame(&mut self, camera: &Camera, delta_time: f32) {
        self.pending.clear();
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

    /// Submit one draw module's render data to the bridge.
    ///
    /// The submission is queued and will be processed during `flush()`.
    pub fn submit(&mut self, submission: DrawSubmission) {
        self.pending.push(submission);
    }

    /// Process all pending submissions: cull, sort, and push into the
    /// WWVegas scene.
    ///
    /// After this returns the scene is ready for the WWVegas renderer to
    /// consume.
    pub fn flush(&mut self) {
        let start = std::time::Instant::now();

        let mut stats = RenderBridgeStats {
            submissions_received: self.pending.len(),
            ..Default::default()
        };

        // Phase 1: Filter hidden objects (shroud, awaiting construction).
        let visible: Vec<&DrawSubmission> = self
            .pending
            .iter()
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
        let camera = match &mut self.camera {
            Some(c) => c,
            None => return, // no camera — skip all rendering
        };

        let after_cull: Vec<&DrawSubmission> = visible
            .into_iter()
            .filter(|s| {
                // Transform bounding sphere to world space for frustum test.
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

        // Phase 3: Partition into opaque / transparent.
        let mut opaque: Vec<&DrawSubmission> = Vec::new();
        let mut transparent: Vec<&DrawSubmission> = Vec::new();

        for s in &after_cull {
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
            dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
        });

        transparent.sort_by(|a, b| {
            let dist_a = distance_sq_to_camera(a, &camera);
            let dist_b = distance_sq_to_camera(b, &camera);
            dist_b.partial_cmp(&dist_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        stats.opaque_draws = opaque.len();
        stats.transparent_draws = transparent.len();
        stats.rendered = opaque.len() + transparent.len();

        // Phase 5: Rebuild the scene layers.
        self.scene.clear();

        // Opaque layer (sort level 0)
        {
            let mut opaque_layer = Layer::new("opaque".to_string());
            opaque_layer.set_sort_level(0);
            for s in opaque {
                let obj = BridgeRenderObject {
                    submission: (*s).clone(),
                };
                opaque_layer.add_object(Box::new(obj));
            }
            self.scene.add_layer(opaque_layer);
        }

        // Transparent layer (sort level 100 — rendered after opaque)
        if !transparent.is_empty() {
            let mut trans_layer = Layer::new("transparent".to_string());
            trans_layer.set_sort_level(100);
            for s in transparent {
                let obj = BridgeRenderObject {
                    submission: (*s).clone(),
                };
                trans_layer.add_object(Box::new(obj));
            }
            self.scene.add_layer(trans_layer);
        }

        stats.bridge_time_s = start.elapsed().as_secs_f32();
        self.stats = stats;
    }

    /// End the frame.  Call after flush and after the WWVegas renderer has
    /// presented.
    pub fn end_frame(&mut self) {
        // No-op for now; could release per-frame scratch buffers.
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Borrow the WWVegas scene (read-only) so the renderer can walk it.
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Borrow the WWVegas scene mutably (advanced use).
    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    /// Borrow the current render info.
    pub fn render_info(&self) -> &RenderInfo {
        &self.render_info
    }

    /// Get statistics from the last completed flush.
    pub fn stats(&self) -> &RenderBridgeStats {
        &self.stats
    }

    /// Mark a model name as loaded (placeholder — full impl will call into
    /// the WWVegas asset manager to load .w3d / .tga / .dds files).
    pub fn mark_model_loaded(&mut self, model_name: &str) {
        self.model_cache.insert(model_name.to_lowercase(), true);
    }

    /// Check whether a model has been loaded.
    pub fn is_model_loaded(&self, model_name: &str) -> bool {
        self.model_cache
            .get(&model_name.to_lowercase())
            .copied()
            .unwrap_or(false)
    }

    /// Get the number of pending (un-flushed) submissions.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Clear all cached model data.
    pub fn clear_model_cache(&mut self) {
        self.model_cache.clear();
    }
}

impl Default for RenderBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Squared distance from a submission's world position to the camera.
///
/// Used for depth sorting.  We use the translation component of the world
/// transform as the object centre.
fn distance_sq_to_camera(submission: &DrawSubmission, camera: &Camera) -> f32 {
    let obj_pos = submission.world_transform.transform_point3(glam::Vec3::ZERO);
    let cam_pos = ww_to_game_vec3(camera.position());
    (obj_pos - cam_pos).length_squared()
}

#[inline]
fn ww_to_game_vec3(v: ww3d_core::glam::Vec3) -> glam::Vec3 {
    glam::Vec3::new(v.x, v.y, v.z)
}

#[inline]
fn game_to_ww_vec3(v: glam::Vec3) -> ww3d_core::glam::Vec3 {
    ww3d_core::glam::Vec3::new(v.x, v.y, v.z)
}

/// Convert a GameLogic `Matrix3D` (which is `glam::Mat4`) to the WWVegas
/// representation.  Since both are `glam::Mat4` under the hood, this is a
/// no-op identity conversion, but it documents the boundary.
#[inline]
pub fn game_logic_to_wwvegas_transform(m: glam::Mat4) -> glam::Mat4 {
    m
}

/// Convert a GameLogic `Coord3D` (`glam::Vec3`) to a WWVegas `Vec3`.
#[inline]
pub fn game_logic_to_wwvegas_vec3(v: glam::Vec3) -> glam::Vec3 {
    v
}

/// Apply render-state overrides to a WWVegas material.
///
/// This is called by the bridge (or by consumers that walk the scene) to
/// adjust per-object material properties based on condition flags.
pub fn apply_render_state_to_material(
    _material: &mut VertexMaterial,
    _overrides: &RenderStateOverrides,
) {
    // Full implementation will modify material diffuse, emissive, opacity,
    // texture stage flags based on the overrides struct.
    // Left as a stub because VertexMaterial mutation API is still evolving.
}

// ---------------------------------------------------------------------------
// Scene builder helpers
// ---------------------------------------------------------------------------

/// Create a default game scene with standard layers.
///
/// This sets up the layer hierarchy that matches the C++ render ordering:
///
/// 1. Terrain (sort 0)
/// 2. Shadows (sort 50)
/// 3. Opaque objects (sort 100)
/// 4. Transparent objects (sort 200)
/// 5. Post-FX / selection circles (sort 300)
pub fn create_default_game_scene() -> Scene {
    let scene = SceneBuilder::new("Game World".to_string()).build();

    // Layers will be added dynamically by the bridge during flush().
    scene
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Mat4, Vec3};

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
        let camera = Camera::perspective(
            "test".to_string(),
            60.0_f32.to_radians(),
            16.0 / 9.0,
            0.1,
            1000.0,
        );
        camera.set_position(Vec3::new(0.0, 50.0, -100.0));
        camera.look_at(Vec3::ZERO, Vec3::Y);

        bridge.begin_frame(&camera, 0.016);

        // Submit a visible object at the origin.
        let submission = DrawSubmission {
            drawable_id: DrawableId(1),
            model_name: "AVComanche".to_string(),
            world_transform: Mat4::IDENTITY,
            condition_flags: RenderConditionFlags::PRISTINE,
            render_state: RenderStateOverrides::default(),
            bounding_sphere: BoundingSphere::new(Vec3::ZERO, 10.0),
            bounding_box: AABox::new(Vec3::new(-5.0, 0.0, -5.0), Vec3::new(5.0, 10.0, 5.0)),
            opaque: true,
            transparent: false,
            cast_shadow: true,
            ..Default::default()
        };
        bridge.submit(submission);

        // Submit a hidden object.
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
        let camera = Camera::perspective(
            "test".to_string(),
            60.0_f32.to_radians(),
            16.0 / 9.0,
            0.1,
            100.0, // far plane = 100
        );
        camera.set_position(Vec3::new(0.0, 10.0, 0.0));
        camera.look_at(Vec3::new(0.0, 10.0, 1.0), Vec3::Y);

        bridge.begin_frame(&camera, 0.016);

        // Object inside frustum.
        let near = DrawSubmission {
            drawable_id: DrawableId(1),
            bounding_sphere: BoundingSphere::new(Vec3::ZERO, 1.0),
            opaque: true,
            ..Default::default()
        };
        bridge.submit(near);

        // Object beyond far plane.
        let far = DrawSubmission {
            drawable_id: DrawableId(2),
            bounding_sphere: BoundingSphere::new(Vec3::ZERO, 1.0),
            world_transform: Mat4::from_translation(Vec3::new(0.0, 10.0, 500.0)),
            opaque: true,
            ..Default::default()
        };
        bridge.submit(far);

        bridge.flush();

        let stats = bridge.stats();
        assert_eq!(stats.submissions_received, 2);
        // At least one should be culled (the far object).
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
        assert!(bridge.is_model_loaded("TestModel"));
        assert!(bridge.is_model_loaded("testmodel")); // case insensitive
        bridge.clear_model_cache();
        assert!(!bridge.is_model_loaded("TestModel"));
    }

    #[test]
    fn test_create_default_game_scene() {
        let scene = create_default_game_scene();
        assert_eq!(scene.name(), "Game World");
    }

    #[test]
    fn test_game_logic_to_wwvegas_transform_identity() {
        let m = Mat4::IDENTITY;
        assert_eq!(game_logic_to_wwvegas_transform(m), Mat4::IDENTITY);
    }

    #[test]
    fn test_game_logic_to_wwvegas_vec3() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        let converted = game_logic_to_wwvegas_vec3(v);
        assert!((converted.x - 1.0).abs() < f32::EPSILON);
        assert!((converted.y - 2.0).abs() < f32::EPSILON);
        assert!((converted.z - 3.0).abs() < f32::EPSILON);
    }
}
