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
use std::path::PathBuf;
use std::sync::Arc;

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

// ---------------------------------------------------------------------------
// Per-frame draw submission
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DrawSubmission {
    pub drawable_id: DrawableId,
    pub model_name: String,
    pub world_transform: glam::Mat4,
    pub condition_flags: RenderConditionFlags,
    pub render_state: RenderStateOverrides,
    pub bone_overrides: Vec<BoneOverride>,
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

pub struct RenderBridge {
    scene: Scene,
    pending: Vec<DrawSubmission>,
    camera: Option<Camera>,
    model_cache: HashMap<String, Arc<dyn RenderObject>>,
    asset_manager: AssetManager,
    asset_search_paths: Vec<PathBuf>,
    stats: RenderBridgeStats,
    render_info: RenderInfo,
    elapsed_time: f32,
}

impl RenderBridge {
    pub fn new() -> Self {
        let scene = SceneBuilder::new("GameLogic Bridge Scene".to_string()).build();

        Self {
            scene,
            pending: Vec::with_capacity(2048),
            camera: None,
            model_cache: HashMap::new(),
            asset_manager: AssetManager::new(),
            asset_search_paths: Vec::new(),
            stats: RenderBridgeStats::default(),
            render_info: RenderInfo::new(),
            elapsed_time: 0.0,
        }
    }

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

    pub fn submit(&mut self, submission: DrawSubmission) {
        self.pending.push(submission);
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
                if let Some(render_obj) = self.resolve_model(name) {
                    self.model_cache.insert(name.clone(), render_obj);
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

        // Phase 3: Partition into opaque / transparent.
        let mut opaque: Vec<DrawSubmission> = Vec::new();
        let mut transparent: Vec<DrawSubmission> = Vec::new();

        for s in after_cull {
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
    }

    fn resolve_model(&mut self, name: &str) -> Option<Arc<dyn RenderObject>> {
        if let Some(obj) = self.asset_manager.create_render_obj(name) {
            return Some(Arc::from(Box::new(WrapRenderObj(obj)) as Box<dyn RenderObject>));
        }

        for search_path in &self.asset_search_paths {
            let candidate = search_path.join(format!("{name}.w3d"));
            if candidate.exists() {
                if self.asset_manager.load_3d_assets(&candidate).is_ok() {
                    if let Some(obj) = self.asset_manager.create_render_obj(name) {
                        return Some(Arc::from(Box::new(WrapRenderObj(obj)) as Box<dyn RenderObject>));
                    }
                }
            }
        }

        let fallback = ww3d_core::create_cube_mesh(name.to_string(), 1.0);
        Some(Arc::new(fallback))
    }

    pub fn end_frame(&mut self) {}

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
        if let Some(render_obj) = self.resolve_model(&key) {
            self.model_cache.insert(key, render_obj);
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
            if seen.insert(key.clone()) {
                if !self.model_cache.contains_key(&key) {
                    if let Some(render_obj) = self.resolve_model(&key) {
                        self.model_cache.insert(key, render_obj);
                    }
                }
            }
        }
    }

    pub fn is_model_loaded(&self, model_name: &str) -> bool {
        self.model_cache
            .contains_key(&model_name.to_lowercase())
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn clear_model_cache(&mut self) {
        self.model_cache.clear();
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

    fn set_name(&mut self, _name: String) {}

    fn clone_object(&self) -> Box<dyn RenderObject> {
        Box::new(WrapRenderObj(self.0.clone_box()))
    }

    fn render(&mut self, _info: &RenderInfo) -> ww3d_core::errors::W3DResult<()> {
        self.0.render();
        Ok(())
    }

    fn get_obj_space_bounding_sphere(&self) -> BoundingSphere {
        BoundingSphere::zero()
    }

    fn get_obj_space_bounding_box(&self) -> AABox {
        AABox::zero()
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

// Global singleton instance
use std::sync::Mutex;
lazy_static::lazy_static! {
    pub static ref THE_RENDER_BRIDGE: Mutex<Option<RenderBridge>> = Mutex::new(None);
}

pub fn init_render_bridge() {
    let mut guard = THE_RENDER_BRIDGE.lock().unwrap();
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

pub fn create_default_game_scene() -> Scene {
    let scene = SceneBuilder::new("Game World".to_string()).build();
    scene
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Mat4 as GameMat4, Vec3 as GameVec3};
    use ww3d_core::glam::{Mat4 as WwMat4, Vec3 as WwVec3};

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
        assert!(bridge.is_model_loaded("TestModel"));
        assert!(bridge.is_model_loaded("testmodel"));
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
