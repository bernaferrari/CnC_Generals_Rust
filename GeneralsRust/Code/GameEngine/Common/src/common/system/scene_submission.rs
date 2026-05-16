//! Scene submission trait: bridges GameLogic draw modules to GameEngineDevice rendering.
//!
//! GameLogic draw modules (laser, tracer, rope, model, projectile stream) compute
//! geometry but cannot depend on GameEngineDevice directly. This trait lives in
//! Common so both sides can reference it: GameLogic submits geometry, GameClient
//! (RenderBridge) implements the trait.
//!
//! ## Pattern
//! 1. `SceneSubmission` trait defined here in Common (no w3d/ww3d deps)
//! 2. GameLogic helpers (`register_scene_submission`, `submit_scene_model`, etc.)
//!    provide thin wrappers via `OnceLock<Arc<dyn SceneSubmission>>`
//! 3. GameClient's `RenderBridge` implements `SceneSubmission`, converting
//!    Common plain-data types to ww3d types internally
//! 4. Draw modules in GameLogic call through helpers — never touch GameClient
//!
//! Reference: C++ W3DDisplay singleton that everything accesses globally.

use super::geometry::Coord3D;
use super::geometry::Matrix3D;

// ---------------------------------------------------------------------------
// Line descriptions (laser, tracer, rope)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SceneLineDesc {
    pub start: Coord3D,
    pub end: Coord3D,
    pub width: f32,
    pub color_r: f32,
    pub color_g: f32,
    pub color_b: f32,
    pub opacity: f32,
    pub texture_name: Option<String>,
    pub tile_factor: f32,
    pub visible: bool,
}

pub type SceneLineId = u64;

// ---------------------------------------------------------------------------
// Model descriptions (W3DModelDraw, W3DPropDraw)
// ---------------------------------------------------------------------------

/// Plain-data model submission descriptor.
///
/// Uses only Common types (Coord3D, Matrix3D) — no w3d/ww3d dependencies.
/// GameClient's RenderBridge converts these to its internal `DrawSubmission`.
///
/// Parity: mirrors the data C++ W3DModelDraw::doDrawModule() passes to
/// W3DDisplay::addRenderObject() (transform, model state, animation, bones).
#[derive(Debug, Clone)]
pub struct SceneModelDesc {
    pub drawable_id: u32,
    pub model_name: String,
    /// Column-major 4x4 world transform
    pub world_transform: Matrix3D,
    /// Model condition flags (ModelConditionFlagType bits)
    pub condition_flags: u64,
    pub opacity: f32,
    pub emissive_tint: [f32; 3],
    pub damage_overlay: f32,
    pub hidden: bool,
    pub selected: bool,
    pub apply_night_map: bool,
    pub apply_snow_map: bool,
    pub sort_level: i32,
    pub transparent: bool,
    pub cast_shadow: bool,
    pub bounding_sphere_center: Coord3D,
    pub bounding_sphere_radius: f32,
    pub animation_name: Option<String>,
    /// Animation time in seconds
    pub animation_time: f32,
    pub bone_overrides: Vec<BoneOverrideDesc>,
    pub mesh_uv_overrides: Vec<MeshUvOverrideDesc>,
}

impl Default for SceneModelDesc {
    fn default() -> Self {
        Self {
            drawable_id: 0,
            model_name: String::new(),
            world_transform: Matrix3D::identity(),
            condition_flags: 0,
            opacity: 1.0,
            emissive_tint: [0.0; 3],
            damage_overlay: 0.0,
            hidden: false,
            selected: false,
            apply_night_map: false,
            apply_snow_map: false,
            sort_level: 0,
            transparent: false,
            cast_shadow: true,
            bounding_sphere_center: Coord3D::new(0.0, 0.0, 0.0),
            bounding_sphere_radius: 0.0,
            animation_name: None,
            animation_time: 0.0,
            bone_overrides: Vec::new(),
            mesh_uv_overrides: Vec::new(),
        }
    }
}

/// Bone override for skeletal animation — submitted via SceneModelDesc.
#[derive(Debug, Clone)]
pub struct BoneOverrideDesc {
    pub bone_index: i32,
    pub bone_name: Option<String>,
    pub transform: Matrix3D,
}

/// Mesh UV override for tread scrolling, etc — submitted via SceneModelDesc.
#[derive(Debug, Clone)]
pub struct MeshUvOverrideDesc {
    pub mesh_name_prefix: String,
    pub u_offset: f32,
    pub v_offset: f32,
}

// ---------------------------------------------------------------------------
// Projectile stream descriptions
// ---------------------------------------------------------------------------

/// Projectile stream polyline data — submitted by W3DProjectileStreamDraw.
#[derive(Debug, Clone)]
pub struct SceneProjectileStreamDesc {
    pub drawable_id: u32,
    /// Segmented polylines — each inner Vec is one continuous line segment.
    pub lines: Vec<Vec<Coord3D>>,
    pub texture_name: String,
    pub width: f32,
    pub tile_factor: f32,
    pub scroll_rate: f32,
}

// ---------------------------------------------------------------------------
// SceneSubmission trait
// ---------------------------------------------------------------------------

/// Bridge trait: GameLogic submits render data, GameClient/GameEngineDevice
/// implements the trait to consume it.
///
/// Uses `&self` because implementations wrap interior-mutable scene state
/// (e.g. `Arc<RwLock<...>>` or `Mutex<RenderBridge>`).
///
/// Parity: mirrors C++ W3DDisplay singleton access — draw modules call a
/// global, the global delegates to the device. Same indirection, type-safe.
pub trait SceneSubmission: Send + Sync {
    // --- Lines (laser, tracer, rope) ---
    fn submit_line(&self, drawable_id: u32, desc: &SceneLineDesc) -> Option<SceneLineId>;
    fn update_line(&self, id: SceneLineId, desc: &SceneLineDesc);
    fn remove_line(&self, id: SceneLineId);

    // --- Models (W3DModelDraw, W3DPropDraw, etc.) ---
    fn submit_model(&self, desc: SceneModelDesc);

    // --- Projectile streams ---
    fn submit_projectile_stream(&self, desc: SceneProjectileStreamDesc);

    // --- Frame lifecycle ---
    /// Called at start of each logic frame (30fps). Clears previous frame's
    /// model submissions so the render loop sees only current state.
    fn begin_logic_frame(&self);

    /// Called at end of each logic frame. Signals that submissions are ready
    /// for the next render pass.
    fn end_logic_frame(&self);
}
