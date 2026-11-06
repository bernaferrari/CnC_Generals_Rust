//! # View Module
//!
//! Provides camera and viewport management for 3D RTS gameplay.
//! Handles 3D perspective and orthographic projections, camera movement,
//! rotation, zooming, view frustum culling, and world-screen transformations.

use std::cell::RefCell;
use std::f32::consts::PI;

use game_engine::common::ini::get_global_data;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::display::cinematic_camera::{
    CameraPath, CameraPitchTransition, CameraPositionTransition, CameraRotateTransition,
    CameraWaypoint, CameraZoomTransition,
};
use crate::drawable::drawable_manager::{with_drawable_manager, with_drawable_manager_ref};
use crate::drawable::{DrawableType, Vector3 as DrawVec3};
use crate::gui::{HintType, WindowStatus};
use crate::terrain::terrain_visual::get_terrain_visual;
use gamelogic::helpers::{TheGameLogic, TheTerrainLogic};
use gamelogic::scripting::engine::get_named_object_tracker;
use glam::{Mat4, Vec3, Vec4};
use std::cell::Cell;

#[cfg(test)]
#[path = "view_owned_shake_tests.rs"]
mod owned_shake_tests;

mod command_markers;
mod camera_script;
mod camera_math;
#[cfg(test)]
mod tests;
pub use command_markers::draw_live_command_markers;
use camera_math::{ease_ratios, frames_to_ms, ms_to_frames, normalize_angle, travel_camera_angle};
use command_markers::{
    draw_move_hint_and_locater_models, draw_move_hint_and_locater_models_with_owner,
    hide_or_move_model, spawn_world_model,
};

/// Unique identifier for view instances
pub type ViewId = u32;

/// Default view dimensions and settings
pub const DEFAULT_VIEW_WIDTH: i32 = 640;
pub const DEFAULT_VIEW_HEIGHT: i32 = 480;
pub const DEFAULT_VIEW_ORIGIN_X: i32 = 0;
pub const DEFAULT_VIEW_ORIGIN_Y: i32 = 0;
/// Default **horizontal** FOV. C++ `View::m_FOV` (View.h:173, View.cpp:53).
pub const DEFAULT_FOV_DEGREES: f32 = 50.0;
pub const DEFAULT_FOV_RADIANS: f32 = DEFAULT_FOV_DEGREES * PI / 180.0;

/// C++ `CameraClass::Set_View_Plane(hfov, -1)` (WW3D2/camera.cpp:257-261):
/// `vfov = 2*atan(tan(hfov/2)/aspect)` so 50° horizontal is preserved.
pub fn vertical_fov_from_horizontal(hfov_radians: f32, aspect: f32) -> f32 {
    let aspect = aspect.max(0.01);
    2.0 * ((hfov_radians * 0.5).tan() / aspect).atan()
}
const LOGIC_FRAMES_PER_SECOND: f32 = 30.0;
const FRAME_LENGTH_MS: f32 = 1000.0 / LOGIC_FRAMES_PER_SECOND;
const DEFAULT_NEAR_CLIP: f32 = 1.0;
const DEFAULT_FAR_CLIP: f32 = 20000.0;
const MIN_CAPPED_ZOOM: f32 = 0.5;
const MAX_GROUND_LEVEL: f32 = 120.0;
const TERRAIN_SAMPLE_SIZE: f32 = 40.0;
const SCROLL_RESOLUTION: f32 = 250.0;
const PATHFIND_CELL_SIZE_F: f32 = 10.0;
const MAX_REQUEST_CACHE_SIZE: usize = 50;
const DEFAULT_CAMERA_HEIGHT: f32 = 200.0;
const DEFAULT_CAMERA_PITCH_DEG: f32 = 37.5;
pub const LETTER_BOX_FADE_TIME_MS: f32 = 1000.0;

/// C++ `TheDisplay->isLetterBoxed()` as seen by `W3DView::buildCameraTransform`.
pub fn set_display_letter_boxed(enabled: bool) {
    #[cfg(test)]
    {
        TEST_LETTER_BOXED.with(|flag| flag.set(enabled));
        return;
    }
    #[cfg(not(test))]
    {
        let _ = crate::core::game_client::with_live_game_client_mut(|client| {
            client.display_letter_boxed = enabled;
        });
    }
}

/// C++ `TheDisplay->isLetterBoxed()`.
pub fn is_display_letter_boxed() -> bool {
    #[cfg(test)]
    {
        return TEST_LETTER_BOXED.with(|flag| flag.get());
    }
    #[cfg(not(test))]
    {
        crate::core::game_client::with_live_game_client_mut(|client| client.display_letter_boxed)
            .unwrap_or(false)
    }
}

#[cfg(test)]
thread_local! {
    static TEST_LETTER_BOXED: Cell<bool> = const { Cell::new(false) };
}

fn ground_height_at(x: f32, y: f32) -> f32 {
    TheTerrainLogic::get()
        .map(|terrain| terrain.get_ground_height(x, y, None))
        .unwrap_or(0.0)
}

/// C++ `getHeightAroundPos` (W3DView.cpp:117): raw max of the terrain
/// samples — below-sea terrain pulls the zoom target down (no 0.0 floor).
fn height_around_pos(x: f32, y: f32) -> f32 {
    let sample = TERRAIN_SAMPLE_SIZE;
    [
        ground_height_at(x, y),
        ground_height_at(x + sample, y - sample),
        ground_height_at(x - sample, y - sample),
        ground_height_at(x + sample, y + sample),
        ground_height_at(x - sample, y + sample),
    ]
    .into_iter()
    .fold(f32::NEG_INFINITY, f32::max)
}

/// Absolute pitch (radians) of the global default camera boom
/// (`camera_offset_from_global` builds its offset at this elevation).
fn global_camera_pitch_radians() -> f32 {
    let pitch_deg = get_global_data()
        .map(|global| {
            let global = global.read();
            if global.camera_pitch.abs() < 0.1 {
                DEFAULT_CAMERA_PITCH_DEG
            } else {
                global.camera_pitch
            }
        })
        .unwrap_or(DEFAULT_CAMERA_PITCH_DEG);
    pitch_deg * PI / 180.0
}

fn camera_offset_from_global(ground_level: f32) -> Point3 {
    let (height, yaw_deg) = get_global_data()
        .map(|global| {
            let global = global.read();
            let height = if global.camera_height.abs() < 1.0 {
                DEFAULT_CAMERA_HEIGHT
            } else {
                global.camera_height
            };
            (height, global.camera_yaw)
        })
        .unwrap_or((DEFAULT_CAMERA_HEIGHT, 0.0));
    let z = ground_level + height;
    let pitch = global_camera_pitch_radians();
    let yaw = yaw_deg * PI / 180.0;
    let tan_pitch = pitch.tan();
    let y = if tan_pitch.abs() < 1.0e-4 {
        -z
    } else {
        -z / tan_pitch
    };
    Point3::new(-y * yaw.tan(), y, z)
}

fn rotate_vec_around_x(v: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    Vec3::new(v.x, v.y * c - v.z * s, v.y * s + v.z * c)
}

fn rotate_vec_around_z(v: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

fn intersect_terrain_ray(start: Vec3, end: Vec3) -> Option<Vec3> {
    get_terrain_visual().ok().and_then(|guard| {
        guard
            .as_ref()
            .and_then(|visual| visual.intersect_terrain(start, end))
    })
}

fn ray_sphere_t(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = origin - center;
    let a = dir.dot(dir);
    if a.abs() < 1.0e-12 {
        return None;
    }
    let b = 2.0 * oc.dot(dir);
    let c = oc.dot(oc) - radius * radius;
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return None;
    }
    let sqrt_disc = disc.sqrt();
    let t0 = (-b - sqrt_disc) / (2.0 * a);
    let t1 = (-b + sqrt_disc) / (2.0 * a);
    if t0 >= 0.0 {
        Some(t0)
    } else if t1 >= 0.0 {
        Some(t1)
    } else {
        None
    }
}

/// Hosted point pick against the receiving GameClient's canonical native
/// drawable snapshot and receiving Main camera. No tactical-view or drawable
/// manager singleton is consulted.
pub fn pick_native_drawable_from_facts(
    screen: &IPoint2,
    window_query: &mut dyn FnMut(i32, i32) -> bool,
    camera: &game_engine::common::message_stream::TranslatorPickCamera,
    drawables: &[game_engine::common::message_stream::TranslatorPickDrawableFact],
) -> Option<u32> {
    if window_query(screen.x, screen.y) {
        return None;
    }
    let [width, height] = camera.viewport_size;
    if width <= 0 || height <= 0 {
        return None;
    }
    let x = ((screen.x - camera.viewport_origin[0]) as f32 / width as f32) * 2.0 - 1.0;
    let y = 1.0 - ((screen.y - camera.viewport_origin[1]) as f32 / height as f32) * 2.0;
    let view = Mat4::from_cols_array(&camera.view_matrix);
    let projection = Mat4::from_cols_array(&camera.projection_matrix);
    let inverse = (projection * view).inverse();
    if !inverse.is_finite() {
        return None;
    }
    let near = inverse * Vec4::new(x, y, -1.0, 1.0);
    let far = inverse * Vec4::new(x, y, 1.0, 1.0);
    if near.w.abs() < 1.0e-6 || far.w.abs() < 1.0e-6 {
        return None;
    }
    let near = near.truncate() / near.w;
    let far = far.truncate() / far.w;
    let origin = Vec3::from_array(camera.camera_position);
    // C++ W3DView builds a finite camera-origin ray using the active camera's
    // depth. The unprojected far-plane point supplies that receiving camera's
    // endpoint; do not replace it with a process-wide/fixed clip distance.
    let direction = far - origin;
    if direction.length_squared() < 1.0e-12 {
        return None;
    }
    let mut best = None;
    let mut best_t = f32::MAX;
    for drawable in drawables {
        let center = Vec3::from_array(drawable.center);
        if let Some(t) = ray_sphere_t(origin, direction, center, drawable.radius.max(1.0)) {
            if t <= 1.0 && t < best_t {
                best_t = t;
                best = Some(drawable.object_id);
            }
        }
    }
    best
}

pub fn world_to_screen_from_pick_camera(
    world: &Point3,
    camera: &game_engine::common::message_stream::TranslatorPickCamera,
) -> Option<IPoint2> {
    let [width, height] = camera.viewport_size;
    if width <= 0 || height <= 0 {
        return None;
    }
    let matrix = Mat4::from_cols_array(&camera.projection_matrix)
        * Mat4::from_cols_array(&camera.view_matrix);
    let clip = matrix * Vec4::new(world.x, world.y, world.z, 1.0);
    if !clip.is_finite() || clip.w.abs() < 1.0e-6 || clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    (ndc.is_finite()
        && (-1.0..=1.0).contains(&ndc.x)
        && (-1.0..=1.0).contains(&ndc.y)
        && (-1.0..=1.0).contains(&ndc.z))
    .then(|| {
        IPoint2::new(
            (((ndc.x + 1.0) * 0.5 * width as f32 + camera.viewport_origin[0] as f32).round())
                as i32,
            (((1.0 - ndc.y) * 0.5 * height as f32 + camera.viewport_origin[1] as f32).round())
                as i32,
        )
    })
}
fn named_object_transform(object_name: &str, bone_name: &str) -> Option<(Vec3, Vec3)> {
    if object_name.is_empty() {
        return None;
    }
    let tracker = get_named_object_tracker();
    let object_id = tracker.get_object_id(object_name).ok().flatten()?;
    let object = TheGameLogic::find_object_by_id(object_id)?;
    let object = object.read().ok()?;
    let pos = object.get_position();
    let mut eye = Vec3::new(pos.x, pos.y, pos.z);
    let mut target = eye + Vec3::Y;
    if !bone_name.is_empty() {
        if let Some(drawable) = object.get_drawable() {
            if let Ok(drawable) = drawable.read() {
                if let Some(transform) = drawable.get_bone_transform(bone_name) {
                    eye = transform.w_axis.truncate();
                    let forward = (-transform.z_axis.truncate()).normalize_or(Vec3::Y);
                    target = eye + forward;
                }
            }
        }
    }
    Some((eye, target))
}

/// Basic 2D point
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2 {
    pub x: f32,
    pub y: f32,
}

impl Point2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

/// C++ `View` stores and passes `Coord3D` (`BaseType.h`).
pub type Coord3D = game_engine::common::system::geometry::Coord3D;
/// Compatibility name used by older GameClient call sites.
pub type Point3 = Coord3D;

/// Basic 2D vector
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

impl Vector2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    /// Euclidean length (C++ `Coord2D::Length`).
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
}

/// Basic 3D vector
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    pub fn magnitude(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn normalize(&self) -> Self {
        let mag = self.magnitude();
        if mag > 0.0 {
            Self::new(self.x / mag, self.y / mag, self.z / mag)
        } else {
            Self::zero()
        }
    }

    pub fn dot(&self, other: &Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(&self, other: &Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }
}

impl std::ops::Add for Vector3 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl std::ops::Sub for Vector3 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl std::ops::Mul<f32> for Vector3 {
    type Output = Self;

    fn mul(self, scalar: f32) -> Self {
        Self::new(self.x * scalar, self.y * scalar, self.z * scalar)
    }
}

/// Integer 2D point for screen coordinates
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IPoint2 {
    pub x: i32,
    pub y: i32,
}

impl IPoint2 {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Camera shake intensity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CameraShakeType {
    Subtle = 0,
    Normal,
    Strong,
    Severe,
    CineExtreme, // For cinematics only
    CineInsane,  // For cinematics only
}

// Keep the retained View enum and its serde discriminants unchanged while
// sharing the authoring kind with the backend-independent impulse helper.
impl From<CameraShakeType> for game_engine::common::ini::ini_fx_list::CameraShakeType {
    fn from(kind: CameraShakeType) -> Self {
        match kind {
            CameraShakeType::Subtle => Self::Subtle,
            CameraShakeType::Normal => Self::Normal,
            CameraShakeType::Strong => Self::Strong,
            CameraShakeType::Severe => Self::Severe,
            CameraShakeType::CineExtreme => Self::CineExtreme,
            CameraShakeType::CineInsane => Self::CineInsane,
        }
    }
}

/// Return values for world-to-screen transformations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldToScreenReturn {
    /// Point is visible on screen (inside camera frustum)
    InsideFrustum = 0,
    /// Point is valid but outside visible screen area
    OutsideFrustum,
    /// No valid transformation possible
    Invalid,
}

/// Types of objects that can be picked/selected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PickType {
    Terrain = 0,
    Selectable = 1,
    Shrubbery = 2,
    Mines = 3,
    ForceAttackable = 4,
    AllDrawables = 0b11110, // All types except terrain
}

/// Camera locking modes for following objects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraLockType {
    /// Camera follows the object directly
    Follow,
    /// Camera is tethered with maximum distance
    Tether,
}

/// View projection modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionMode {
    /// 3D perspective projection (typical for RTS games)
    Perspective,
    /// Orthographic projection (for special views)
    Orthographic,
}

/// Viewport post-process filter type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterType {
    Null,
    BlackAndWhite,
    Crossfade,
    MotionBlur,
}

/// Viewport post-process filter mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterMode {
    Null,
    BWBlackAndWhite,
    BWRedAndWhite,
    BWGreenAndWhite,
    CrossfadeFbMask,
    MBInAndOutAlpha,
    MBInAndOutSaturate,
    MBInAlpha,
    MBOutAlpha,
    MBInSaturate,
    MBOutSaturate,
    MBEndPanAlpha,
    MBPanAlpha,
    MBPanAlpha1,
    MBPanAlpha2,
    MBPanAlpha3,
}

impl FilterMode {
    fn from_pan_amount(amount: i32) -> Self {
        match amount.clamp(0, 3) {
            0 => Self::MBPanAlpha,
            1 => Self::MBPanAlpha1,
            2 => Self::MBPanAlpha2,
            _ => Self::MBPanAlpha3,
        }
    }
}

/// CPU description of the active viewport filter for Display compositing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewFilterComposite {
    pub filter: FilterType,
    pub mode: FilterMode,
    pub fade: f32,
    pub scroll_delta: Vector2,
    /// C++ `ScreenMotionBlurFilter::m_zoomToPos` when `setViewFilterPos` ran.
    pub zoom_to: Option<Point3>,
}

/// Errors that can occur in view operations
#[derive(Error, Debug)]
pub enum ViewError {
    #[error("Invalid transformation matrix")]
    InvalidTransformation,
    #[error("Point outside valid range")]
    OutOfRange,
    #[error("Invalid camera parameters")]
    InvalidParameters,
    #[error("View not properly initialized")]
    NotInitialized,
}

/// Saved view location for camera positions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewLocation {
    /// Whether this location contains valid data
    valid: bool,
    /// World position the camera is looking at
    position: Point3,
    /// Rotation angle around Z axis (radians)
    angle: f32,
    /// Pitch angle around X axis (radians)
    pitch: f32,
    /// Current zoom level
    zoom: f32,
}

impl ViewLocation {
    /// Create a new empty view location
    pub fn new() -> Self {
        Self {
            valid: false,
            position: Point3::origin(),
            angle: 0.0,
            pitch: 0.0,
            zoom: 0.0,
        }
    }

    /// Initialize view location with specific parameters
    pub fn init(&mut self, x: f32, y: f32, z: f32, angle: f32, pitch: f32, zoom: f32) {
        self.position = Point3::new(x, y, z);
        self.angle = angle;
        self.pitch = pitch;
        self.zoom = zoom;
        self.valid = true;
    }

    /// Check if this location contains valid data
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Get the position
    pub fn position(&self) -> &Point3 {
        &self.position
    }

    /// Get the angle
    pub fn angle(&self) -> f32 {
        self.angle
    }

    /// Get the pitch
    pub fn pitch(&self) -> f32 {
        self.pitch
    }

    /// Get the zoom
    pub fn zoom(&self) -> f32 {
        self.zoom
    }
}

impl Default for ViewLocation {
    fn default() -> Self {
        Self::new()
    }
}

/// Main view/camera system for 3D RTS gameplay
#[derive(Debug, Clone)]
pub struct View {
    /// Unique identifier for this view
    id: u32,

    /// View dimensions in pixels
    width: i32,
    height: i32,

    /// View origin on display (top-left corner)
    origin_x: i32,
    origin_y: i32,

    /// World position the camera is looking at
    position: Point3,

    /// Camera rotation angle around Z axis (radians)
    angle: f32,
    /// Camera pitch angle around X axis (radians)
    pitch_angle: f32,

    /// Current zoom level (higher = more zoomed out)
    zoom: f32,
    /// Height above ground the camera should maintain
    height_above_ground: f32,

    /// Zoom and height constraints
    max_zoom: f32,
    min_zoom: f32,
    max_height_above_ground: f32,
    min_height_above_ground: f32,
    zoom_limited: bool,

    /// Default camera settings
    default_angle: f32,
    default_pitch_angle: f32,

    /// Field of view angle (radians)
    fov: f32,

    /// Camera locking for following objects
    camera_lock_id: Option<u32>,
    /// C++ `View::m_cameraLockDrawable`.
    camera_lock_drawable_id: Option<u32>,
    camera_lock_type: CameraLockType,
    lock_distance: f32,
    snap_immediate: bool,
    /// C++ `W3DView::m_doingScriptedCameraLock`.
    doing_scripted_camera_lock: bool,

    /// C++ W3DView::update static followFactor; -1 when unlocked.
    follow_factor: f32,

    /// Mouse input state
    mouse_locked: bool,

    /// Camera adjustment settings
    ok_to_adjust_height: bool,

    /// Current projection mode
    projection_mode: ProjectionMode,

    /// Guard band bias for rendering margins
    guard_band_bias: Vector2,

    /// Debug information
    terrain_height_under_camera: f32,
    current_height_above_ground: f32,

    /// Active screen filter state.
    filter_state: super::view_filter::ViewFilterState,
    wireframe_enabled: bool,
    wireframe_next_enabled: bool,
    wireframe_pending_frames: u8,
    freeze_time_for_camera_movement: bool,
    freeze_time_for_camera_movement_active: bool,

    /// Camera animation state
    camera_move: Option<CameraPositionTransition>,
    camera_path: Option<CameraPath>,
    camera_rotate: Option<CameraRotateTransition>,
    camera_zoom: Option<CameraZoomTransition>,
    camera_pitch: Option<CameraPitchTransition>,
    rotate_camera_toward: Option<RotateCameraToward>,
    shake_intensity: f32,
    shake_angle_cos: f32,
    shake_angle_sin: f32,
    shake_offset: Vector2,
    /// C++ `W3DView::m_cameraOffset` boom from look-at to eye.
    camera_offset: Point3,
    /// C++ `W3DView::m_groundLevel`.
    ground_level: f32,
    /// C++ `W3DView::m_FXPitch`.
    fx_pitch: f32,
    is_camera_slaved: bool,
    use_real_zoom_cam: bool,
    camera_slave_object_name: String,
    camera_slave_object_bone_name: String,
    camera_constraint_lo: Vector2,
    camera_constraint_hi: Vector2,
    camera_constraint_valid: bool,
    scroll_amount: Vector2,
    camera_has_moved_since_request: bool,
    location_requests: Vec<(IPoint2, Point3)>,
    slave_eye: Option<Point3>,
    slave_target: Option<Point3>,
}

/// State for `rotateCameraTowardObject` / `rotateCameraTowardPosition`.
///
/// Mirrors C++ `W3DView::TRotateCameraInfo` (W3DView.h line 53).
#[derive(Debug, Clone)]
struct RotateCameraToward {
    num_frames: i32,
    cur_frame: i32,
    num_hold_frames: i32,
    ease_in: f32,
    ease_out: f32,
    track_object: bool,
    target_object_id: Option<u32>,
    target_position: Point3,
    start_angle: f32,
    end_angle: f32,
}

impl RotateCameraToward {
    fn total_frames(&self) -> i32 {
        self.num_frames + self.num_hold_frames
    }
}

fn parabolic_ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let t_prime = if t < 0.5 {
        0.5 * (2.0 * t) * (2.0 * t)
    } else {
        let t2 = (t - 0.5) * 2.0;
        let t2 = t2.sqrt();
        0.5 + 0.5 * t2
    };
    t_prime * 0.5 + t * 0.5
}

impl View {
    /// Create a new view with default settings
    pub fn new() -> Self {
        // Per-client tactical view is the C++ singleton; tests may construct
        // additional Views with the same id without process-wide sequencing.
        let id = 1u32;

        Self {
            id,
            width: 0,
            height: 0,
            origin_x: 0,
            origin_y: 0,
            position: Point3::new(0.0, 0.0, 0.0),
            angle: 0.0,
            pitch_angle: 0.0,
            zoom: 0.0,
            height_above_ground: 0.0,
            max_zoom: 1.3,
            min_zoom: 0.2,
            max_height_above_ground: 500.0,
            min_height_above_ground: 50.0,
            zoom_limited: true,
            default_angle: 0.0,
            default_pitch_angle: 0.0,
            fov: DEFAULT_FOV_RADIANS,
            camera_lock_id: None,
            camera_lock_drawable_id: None,
            camera_lock_type: CameraLockType::Follow,
            lock_distance: 0.0,
            snap_immediate: false,
            follow_factor: -1.0,
            doing_scripted_camera_lock: false,

            mouse_locked: false,
            ok_to_adjust_height: true,
            projection_mode: ProjectionMode::Perspective,
            guard_band_bias: Vector2::new(0.0, 0.0),
            terrain_height_under_camera: 0.0,
            current_height_above_ground: 0.0,
            filter_state: super::view_filter::ViewFilterState::new(),
            wireframe_enabled: false,
            wireframe_next_enabled: false,
            wireframe_pending_frames: 0,
            freeze_time_for_camera_movement: false,
            freeze_time_for_camera_movement_active: false,
            camera_move: None,
            camera_path: None,
            camera_rotate: None,
            camera_zoom: None,
            camera_pitch: None,
            rotate_camera_toward: None,
            shake_intensity: 0.0,
            shake_angle_cos: 0.0,
            shake_angle_sin: 0.0,
            shake_offset: Vector2::zero(),
            camera_offset: camera_offset_from_global(10.0),
            ground_level: 10.0,
            fx_pitch: 1.0,
            is_camera_slaved: false,
            use_real_zoom_cam: false,
            camera_slave_object_name: String::new(),
            camera_slave_object_bone_name: String::new(),
            camera_constraint_lo: Vector2::zero(),
            camera_constraint_hi: Vector2::zero(),
            camera_constraint_valid: false,
            scroll_amount: Vector2::zero(),
            camera_has_moved_since_request: true,
            location_requests: Vec::new(),
            slave_eye: None,
            slave_target: None,
        }
    }

    /// Initialize the view with default dimensions and settings
    pub fn init(&mut self) {
        self.width = DEFAULT_VIEW_WIDTH;
        self.height = DEFAULT_VIEW_HEIGHT;
        self.origin_x = DEFAULT_VIEW_ORIGIN_X;
        self.origin_y = DEFAULT_VIEW_ORIGIN_Y;
        self.position = Point3::new(0.0, 0.0, 0.0);
        self.angle = 0.0;
        self.pitch_angle = 0.0;
        self.camera_lock_id = None;
        self.camera_lock_drawable_id = None;
        self.follow_factor = -1.0;
        self.zoom_limited = true;

        if let Some(global) = get_global_data() {
            let data = global.read();
            self.max_height_above_ground = data.max_camera_height;
            self.min_height_above_ground = data.min_camera_height;
            if self.min_height_above_ground > self.max_height_above_ground {
                self.max_height_above_ground = self.min_height_above_ground;
            }
        }

        self.zoom = self.max_zoom;
        self.height_above_ground = self.max_height_above_ground;
        self.ok_to_adjust_height = false;
        self.default_angle = 0.0;
        self.default_pitch_angle = 0.0;
        self.ground_level = 10.0;
        self.camera_offset = camera_offset_from_global(self.ground_level);
        self.fx_pitch = 1.0;
        self.is_camera_slaved = false;
        self.use_real_zoom_cam = false;
        self.camera_has_moved_since_request = true;
    }

    /// Reset the view to default state
    pub fn reset(&mut self) {
        self.zoom_limited = true;
        self.camera_path = None;
        self.doing_scripted_camera_lock = false;

        self.is_camera_slaved = false;
        self.use_real_zoom_cam = false;
        self.fx_pitch = 1.0;
        self.fov = DEFAULT_FOV_RADIANS;
        self.location_requests.clear();
        self.camera_has_moved_since_request = true;
        self.slave_eye = None;
        self.slave_target = None;
        self.filter_state.reset();
        self.wireframe_enabled = false;
        self.wireframe_next_enabled = false;
        self.wireframe_pending_frames = 0;
        self.freeze_time_for_camera_movement = false;
        self.freeze_time_for_camera_movement_active = false;
    }

    /// Get the unique ID of this view
    pub fn id(&self) -> ViewId {
        self.id
    }

    // Dimension accessors
    pub fn width(&self) -> i32 {
        self.width
    }
    pub fn height(&self) -> i32 {
        self.height
    }
    pub fn set_width(&mut self, width: i32) {
        self.width = width;
    }
    pub fn set_height(&mut self, height: i32) {
        self.height = height;
    }

    // Origin accessors
    pub fn origin(&self) -> (i32, i32) {
        (self.origin_x, self.origin_y)
    }
    pub fn set_origin(&mut self, x: i32, y: i32) {
        self.origin_x = x;
        self.origin_y = y;
    }

    // Position accessors
    pub fn position(&self) -> &Point3 {
        &self.position
    }
    pub fn set_position(&mut self, pos: &Point3) {
        self.position = *pos;
    }

    /// Host-camera pose mirror (Main owns the live W3D camera; the leftover
    /// GameClient View must track it for the audio listener and HUD
    /// projections). Raw field assignment: unlike the player-facing
    /// `set_position`/`set_angle`/`set_zoom` setters this does NOT cancel
    /// scripted camera (C++ GameAudio only reads TheTacticalView), and it
    /// does not rescan terrain per frame — `init_height_for_map` stays a
    /// view-init / map-install concern.
    ///
    /// `pitch` is Main's absolute orbit elevation; the view stores pitch as a
    /// tilt relative to the global camera-offset pitch (C++ `m_pitchAngle`,
    /// clamped ±PI/5), so the leftover projection tracks the pitched render
    /// camera instead of sitting at a flat default.
    pub fn sync_pose_from_host(&mut self, pos: &Point3, angle: f32, pitch: f32, zoom: f32) {
        self.position = *pos;
        self.angle = angle;
        self.pitch_angle = (pitch - global_camera_pitch_radians()).clamp(-PI / 5.0, PI / 5.0);
        self.zoom = zoom;
        self.rebuild_real_zoom_fov();
    }

    /// C++ W3DView.cpp:3097-3212 — scripted pans expand m_cameraConstraint.
    fn widen_camera_constraint_for_scripted(&mut self, x: f32, y: f32) {
        if !self.camera_constraint_valid {
            return;
        }
        self.camera_constraint_lo.x = self.camera_constraint_lo.x.min(x);
        self.camera_constraint_hi.x = self.camera_constraint_hi.x.max(x);
        self.camera_constraint_lo.y = self.camera_constraint_lo.y.min(y);
        self.camera_constraint_hi.y = self.camera_constraint_hi.y.max(y);
    }

    /// C++ `m_shakeIntensity` after `W3DView::shake`.
    pub fn camera_shake_intensity(&self) -> f32 {
        self.shake_intensity
    }

    /// C++ `m_shakeOffset` after `W3DView::update` processes impulse shake.
    pub fn impulse_shake_offset(&self) -> Vector2 {
        self.shake_offset
    }

    /// Advance the damped-oscillation impulse (C++ `W3DView::update` shake block).
    pub fn tick_impulse_shake(&mut self) {
        let offset = game_engine::common::view_impulse::ViewImpulse {
            intensity: &mut self.shake_intensity,
            cosine: &mut self.shake_angle_cos,
            sine: &mut self.shake_angle_sin,
        }
        .step();
        self.shake_offset = Vector2::new(offset[0], offset[1]);
    }

    /// Clear impulse shake so FXList tests start from rest.
    pub fn reset_camera_shake(&mut self) {
        self.shake_intensity = 0.0;
        self.shake_angle_cos = 0.0;
        self.shake_angle_sin = 0.0;
        self.shake_offset = Vector2::zero();
    }

    /// Center the view on the given world coordinate.
    ///
    /// C++ `W3DView::lookAt` stores the 3D look-at point (z forced to 0).
    /// Elevated targets are ray-cast onto the heightmap so the object sits
    /// in the screen center.
    pub fn look_at(&mut self, target: &Point3) {
        let mut pos = *target;
        let ground = ground_height_at(pos.x, pos.y);
        if target.z > PATHFIND_CELL_SIZE_F + ground {
            let (start, end) = self.look_at_pick_ray(*target);
            if let Some(hit) = intersect_terrain_ray(start, end) {
                pos.x = hit.x;
                pos.y = hit.y;
            }
        }
        pos.z = 0.0;
        self.position = pos;
        self.camera_has_moved_since_request = true;
        // C++ W3DView::lookAt: cancel rotate / waypoint path / scripted lock.
        self.cancel_scripted_camera_from_player_look_at();
    }

    /// Scroll the view by a screen-space delta converted through the 3D camera.
    ///
    /// C++ `W3DView::scrollBy` unprojects two device points `SCROLL_RESOLUTION`
    /// apart so pan follows yaw/pitch.
    pub fn scroll_by(&mut self, delta: &Vector2) {
        if delta.x == 0.0 && delta.y == 0.0 {
            return;
        }
        self.scroll_amount = *delta;
        let width = self.width.max(1) as f32;
        let height = self.height.max(1) as f32;
        let aspect = width / height;
        let start = IPoint2::new(self.origin_x + self.width, self.origin_y + self.height);
        let end = IPoint2::new(
            start.x + (delta.x * SCROLL_RESOLUTION).round() as i32,
            start.y + (delta.y * SCROLL_RESOLUTION * aspect).round() as i32,
        );
        if let (Ok(world_start), Ok(world_end)) = (
            self.screen_to_world_at_z(&start, self.ground_level),
            self.screen_to_world_at_z(&end, self.ground_level),
        ) {
            self.position.x += world_end.x - world_start.x;
            self.position.y += world_end.y - world_start.y;
        } else {
            self.position.x += delta.x;
            self.position.y += delta.y;
        }
        self.camera_has_moved_since_request = true;
        // C++ W3DView::scrollBy: only `m_doingRotateCamera = false`.
        self.cancel_scripted_camera_from_player_scroll();
    }

    /// Stamp C++ `W3DView::m_scrollAmount` without moving leftover pose.
    /// Live pan (`camera_scroll_world_delta`) writes the same-frame screen
    /// delta so `filter_composite` can feed `ScreenMotionBlurFilter`.
    pub fn record_scroll_amount(&mut self, delta: Vector2) {
        self.scroll_amount = delta;
    }

    pub fn scroll_amount(&self) -> Vector2 {
        self.scroll_amount
    }

    // Angle and rotation
    pub fn angle(&self) -> f32 {
        self.angle
    }
    pub fn set_angle(&mut self, angle: f32) {
        self.angle = angle;
        // C++ W3DView::setAngle cancels scripted rotate/pitch/zoom/path/lock.
        self.cancel_scripted_camera_from_player_set();
    }

    pub fn pitch(&self) -> f32 {
        self.pitch_angle
    }
    pub fn set_pitch(&mut self, pitch: f32) {
        // Limit pitch to reasonable range for RTS camera
        let limit = PI / 5.0; // 36 degrees
        self.pitch_angle = pitch.clamp(-limit, limit);
        // C++ W3DView::setPitch cancels scripted rotate/pitch/zoom/path/lock.
        self.cancel_scripted_camera_from_player_set();
    }

    /// Reset angle and pitch to default values
    pub fn set_angle_and_pitch_to_default(&mut self) {
        self.angle = self.default_angle;
        self.pitch_angle = self.default_pitch_angle;
    }

    pub fn set_default_view(&mut self, pitch: f32, _angle: f32, max_height: f32) {
        self.default_pitch_angle = pitch;
        let global_max_height = get_global_data()
            .map(|global| global.read().max_camera_height)
            .unwrap_or(self.max_height_above_ground);
        self.max_height_above_ground =
            (global_max_height * max_height).max(self.min_height_above_ground);
    }

    // Zoom and height
    pub fn zoom(&self) -> f32 {
        self.zoom
    }
    pub fn set_zoom(&mut self, zoom: f32) {
        // C++ W3DView::setZoom clamps m_zoom unconditionally; `zoomLimited`
        // gates setHeightAboveGround, not zoom (W3DView.cpp:1930-1949).
        self.zoom = zoom.clamp(self.min_zoom, self.max_zoom);
        self.rebuild_real_zoom_fov();
        // C++ W3DView::setZoom cancels scripted rotate/pitch/zoom/path/lock.
        self.cancel_scripted_camera_from_player_set();
        self.camera_constraint_valid = false;
    }

    pub fn height_above_ground(&self) -> f32 {
        self.height_above_ground
    }
    pub fn set_height_above_ground(&mut self, height: f32) {
        // C++ W3DView::setHeightAboveGround: clamp only when zoomLimited.
        self.height_above_ground = if self.zoom_limited {
            height.clamp(self.min_height_above_ground, self.max_height_above_ground)
        } else {
            height
        };
        self.cancel_scripted_camera_from_player_set();
        self.camera_constraint_valid = false;
        self.camera_has_moved_since_request = true;
    }

    pub fn zoom_in(&mut self) {
        // C++ View::zoomIn only steps height (View.cpp:104-107);
        // okToAdjustHeight is owned by GameLogic (GameLogic.cpp:2237).
        self.set_height_above_ground(self.height_above_ground - 10.0);
    }

    pub fn zoom_out(&mut self) {
        self.set_height_above_ground(self.height_above_ground + 10.0);
    }

    pub fn set_zoom_to_default(&mut self) {
        let terrain_height_max = height_around_pos(self.position.x, self.position.y);
        let desired_height = terrain_height_max + self.max_height_above_ground;
        let desired_zoom = if self.camera_offset.z.abs() > 1.0e-4 {
            desired_height / self.camera_offset.z
        } else {
            self.max_zoom
        };
        self.zoom = if self.zoom_limited {
            desired_zoom.clamp(self.min_zoom, self.max_zoom)
        } else {
            desired_zoom
        };
        self.height_above_ground = self.max_height_above_ground;
        self.cancel_scripted_camera_from_player_set();
        self.camera_constraint_valid = false;
        self.camera_has_moved_since_request = true;
    }

    /// C++ `setAngle`/`setPitch`/`setZoom`/`setHeightAboveGround`.
    pub fn cancel_scripted_camera_from_player_set(&mut self) {
        self.camera_path = None;
        self.camera_move = None;
        self.camera_rotate = None;
        self.camera_zoom = None;
        self.camera_pitch = None;
        self.rotate_camera_toward = None;
        self.doing_scripted_camera_lock = false;
    }

    /// C++ `W3DView::lookAt`: rotate + waypoint path + scripted lock.
    pub fn cancel_scripted_camera_from_player_look_at(&mut self) {
        self.camera_path = None;
        self.camera_move = None;
        self.camera_rotate = None;
        self.rotate_camera_toward = None;
        self.doing_scripted_camera_lock = false;
    }

    /// C++ `W3DView::scrollBy`: only `m_doingRotateCamera = false`.
    pub fn cancel_scripted_camera_from_player_scroll(&mut self) {
        self.camera_rotate = None;
    }

    /// C++ `W3DView::initHeightForMap`.
    pub fn init_height_for_map(&mut self) {
        self.ground_level =
            ground_height_at(self.position.x, self.position.y).min(MAX_GROUND_LEVEL);
        self.camera_offset = camera_offset_from_global(self.ground_level);
        self.camera_constraint_valid = false;
        self.camera_has_moved_since_request = true;
    }

    /// C++ `W3DView::cameraEnableSlaveMode`.
    pub fn camera_enable_slave_mode(&mut self, object_name: &str, bone_name: &str) {
        self.is_camera_slaved = true;
        self.camera_slave_object_name = object_name.to_string();
        self.camera_slave_object_bone_name = bone_name.to_string();
        self.apply_slave_camera();
    }

    /// C++ `W3DView::cameraDisableSlaveMode`.
    pub fn camera_disable_slave_mode(&mut self) {
        self.is_camera_slaved = false;
        self.slave_eye = None;
        self.slave_target = None;
    }

    pub fn is_camera_slaved(&self) -> bool {
        self.is_camera_slaved
    }

    /// C++ `W3DView::cameraEnableRealZoomMode`.
    pub fn camera_enable_real_zoom_mode(&mut self) {
        self.use_real_zoom_cam = true;
        self.fx_pitch = 1.0;
        self.rebuild_real_zoom_fov();
    }

    /// C++ `W3DView::cameraDisableRealZoomMode`.
    pub fn camera_disable_real_zoom_mode(&mut self) {
        self.use_real_zoom_cam = false;
        self.fx_pitch = 1.0;
        self.fov = DEFAULT_FOV_RADIANS;
    }

    pub fn is_real_zoom_cam(&self) -> bool {
        self.use_real_zoom_cam
    }
    pub fn fx_pitch(&self) -> f32 {
        self.fx_pitch
    }

    pub fn set_fx_pitch(&mut self, pitch: f32) {
        self.fx_pitch = if pitch.is_finite() { pitch } else { 1.0 };
    }

    pub fn ground_level(&self) -> f32 {
        self.ground_level
    }

    pub fn camera_offset(&self) -> Point3 {
        self.camera_offset
    }

    // Zoom limits
    pub fn max_zoom(&self) -> f32 {
        self.max_zoom
    }
    pub fn set_zoom_limited(&mut self, limited: bool) {
        self.zoom_limited = limited;
    }
    pub fn is_zoom_limited(&self) -> bool {
        self.zoom_limited
    }

    /// Horizontal field of view in radians (C++ `View::getFieldOfView`).
    pub fn field_of_view(&self) -> f32 {
        self.fov
    }
    /// Set horizontal field of view in radians (C++ `View::setFieldOfView`).
    pub fn set_field_of_view(&mut self, fov_radians: f32) {
        self.fov = fov_radians.clamp(0.1, PI - 0.1);
    }

    // Camera locking
    pub fn camera_lock_id(&self) -> Option<u32> {
        self.camera_lock_id
    }
    pub fn set_camera_lock(&mut self, id: Option<u32>) {
        // C++ W3DView::setCameraLock (W3DView.cpp:1757-1765): do not lock
        // onto an object while camera movement is disabled.
        if id.is_some()
            && get_global_data()
                .map(|g| g.read().disable_camera_movement)
                .unwrap_or(false)
        {
            return;
        }
        self.camera_lock_id = id;
        self.lock_distance = 0.0;
        self.camera_lock_type = CameraLockType::Follow;
        // C++ W3DView::setCameraLock clears m_doingScriptedCameraLock.
        self.doing_scripted_camera_lock = false;
        if id.is_some() {
            // A lock engages: a leftover scripted waypoint path would keep
            // fighting the follow each frame.
            self.camera_path = None;
        } else {
            self.follow_factor = -1.0;
        }
    }

    pub fn camera_lock_drawable_id(&self) -> Option<u32> {
        self.camera_lock_drawable_id
    }
    /// C++ `View::setCameraLockDrawable` — also zeroes `m_lockDist`.
    pub fn set_camera_lock_drawable(&mut self, id: Option<u32>) {
        self.camera_lock_drawable_id = id;
        self.lock_distance = 0.0;
    }

    pub fn snap_to_camera_lock(&mut self) {
        self.snap_immediate = true;
    }

    pub fn set_snap_mode(&mut self, lock_type: CameraLockType, distance: f32) {
        self.camera_lock_type = lock_type;
        self.lock_distance = distance;
        // C++ W3DView::setSnapMode arms m_doingScriptedCameraLock.
        self.doing_scripted_camera_lock = true;
    }

    fn apply_camera_lock_one_frame(&mut self) {
        let Some(object_id) = self.camera_lock_id else {
            self.follow_factor = -1.0;
            return;
        };
        let Some(object) = TheGameLogic::find_object_by_id(object_id) else {
            self.camera_lock_id = None;
            self.camera_lock_drawable_id = None;
            self.follow_factor = -1.0;
            return;
        };
        let Ok(object_guard) = object.read() else {
            return;
        };

        if self.follow_factor < 0.0 {
            self.follow_factor = 0.05;
        } else {
            self.follow_factor = (self.follow_factor + 0.05).min(1.0);
        }

        let objpos = object_guard.get_position();
        let mut cur_x = self.position.x;
        let mut cur_y = self.position.y;
        let dx = objpos.x - cur_x;
        let dy = objpos.y - cur_y;
        let cell = get_global_data()
            .map(|g| g.read().partition_cell_size)
            .unwrap_or(0.0);
        let snap_thresh_sqr = cell * cell;
        let cur_dist_sqr = dx * dx + dy * dy;

        if self.snap_immediate {
            cur_x = objpos.x;
            cur_y = objpos.y;
        } else if self.camera_lock_type == CameraLockType::Tether {
            if cur_dist_sqr >= snap_thresh_sqr && cur_dist_sqr > 0.0 {
                let ratio = 1.0 - snap_thresh_sqr / cur_dist_sqr;
                cur_x += dx * ratio * 0.5;
                cur_y += dy * ratio * 0.5;
            } else {
                let ratio = 0.01 * self.lock_distance;
                cur_x += dx * ratio;
                cur_y += dy * ratio;
            }
        } else {
            cur_x += dx * self.follow_factor;
            cur_y += dy * self.follow_factor;
        }

        self.position.x = cur_x;
        self.position.y = cur_y;
        self.position.z = 0.0;

        if self.camera_lock_type == CameraLockType::Follow
            && object_guard.is_using_airborne_locomotor()
            && object_guard.is_above_terrain()
        {
            let ideal = normalize_angle(object_guard.get_orientation() - PI * 0.5);
            if self.snap_immediate {
                self.angle = ideal;
            } else {
                let diff = normalize_angle(ideal - self.angle);
                self.angle = normalize_angle(self.angle + diff * 0.1);
            }
        }

        if self.snap_immediate {
            self.snap_immediate = false;
        }
        self.ground_level = objpos.z;
        self.camera_has_moved_since_request = true;
    }

    fn settle_zoom_toward_height_above_ground(&mut self) {
        if !self.ok_to_adjust_height || self.camera_offset.z.abs() < 1.0e-4 {
            return;
        }
        // C++ writes m_zoom directly and skips while didScriptedMovement.
        if self.camera_path.is_some()
            || self.camera_move.is_some()
            || self.camera_rotate.is_some()
            || self.camera_zoom.is_some()
            || self.camera_pitch.is_some()
            || self.doing_scripted_camera_lock
        {
            return;
        }
        let desired_height = self.terrain_height_under_camera + self.height_above_ground;
        let desired_zoom = desired_height / self.camera_offset.z;
        let (adjust, scroll_cutoff, enforce_max) = get_global_data()
            .map(|g| {
                let g = g.read();
                (
                    g.camera_adjust_speed,
                    g.scroll_amount_cutoff,
                    g.enforce_max_camera_height,
                )
            })
            .unwrap_or((0.1, 10.0, true));
        // C++ W3DView.cpp:1321-1333: while scrolling, only settle when the
        // scroll is slow or the height left the [min,max] band.
        if self.scroll_amount.length() > 0.0
            && self.scroll_amount.length() >= scroll_cutoff
            && self.current_height_above_ground >= self.min_height_above_ground
            && !(enforce_max && self.current_height_above_ground > self.max_height_above_ground)
        {
            return;
        }
        let zoom_adj = (desired_zoom - self.zoom) * adjust;
        if zoom_adj.abs() >= 0.0001 {
            self.zoom += zoom_adj;
            self.rebuild_real_zoom_fov();
        }
    }

    // Mouse control
    pub fn set_mouse_lock(&mut self, locked: bool) {
        self.mouse_locked = locked;
    }
    pub fn is_mouse_locked(&self) -> bool {
        self.mouse_locked
    }

    // Height adjustment
    pub fn set_ok_to_adjust_height(&mut self, ok: bool) {
        self.ok_to_adjust_height = ok;
    }

    /// Get the actual 3D camera position in world space.
    ///
    /// C++ `W3DView::buildCameraTransform` eye point (or slave bone translation).
    pub fn get_3d_camera_position(&self) -> Point3 {
        self.build_camera_eye_and_target().0
    }

    fn look_at_pick_ray(&self, target: Point3) -> (Vec3, Vec3) {
        let eye = self.camera_position_vec3();
        let mut dir = self.camera_target_vec3() - eye;
        if dir.length_squared() < 1.0e-8 {
            dir = Vec3::Y;
        }
        let dir = dir.normalize() * DEFAULT_FAR_CLIP;
        let start = Vec3::new(target.x, target.y, target.z);
        (start, start + dir)
    }

    fn apply_slave_camera(&mut self) {
        match named_object_transform(
            &self.camera_slave_object_name,
            &self.camera_slave_object_bone_name,
        ) {
            Some((eye, target)) => {
                self.position = Point3::new(eye.x, eye.y, eye.z);
                self.slave_eye = Some(Point3::new(eye.x, eye.y, eye.z));
                self.slave_target = Some(Point3::new(target.x, target.y, target.z));
            }
            None => {
                self.is_camera_slaved = false;
                self.slave_eye = None;
                self.slave_target = None;
            }
        }
    }

    fn rebuild_real_zoom_fov(&mut self) {
        if !self.use_real_zoom_cam {
            return;
        }
        let capped = self.zoom.clamp(MIN_CAPPED_ZOOM, 1.0);
        self.fov = DEFAULT_FOV_RADIANS * capped * capped;
    }

    fn build_camera_eye_and_target(&self) -> (Point3, Point3) {
        if self.is_camera_slaved {
            if let (Some(eye), Some(target)) = (self.slave_eye, self.slave_target) {
                return (eye, target);
            }
        }

        let mut pos = self.position;
        pos.x += self.shake_offset.x;
        pos.y += self.shake_offset.y;
        if self.camera_constraint_valid {
            pos.x = pos
                .x
                .clamp(self.camera_constraint_lo.x, self.camera_constraint_hi.x);
            pos.y = pos
                .y
                .clamp(self.camera_constraint_lo.y, self.camera_constraint_hi.y);
        }

        let mut source = if self.use_real_zoom_cam {
            Vec3::new(
                self.camera_offset.x,
                self.camera_offset.y,
                self.camera_offset.z,
            )
        } else {
            Vec3::new(
                self.camera_offset.x * self.zoom,
                self.camera_offset.y * self.zoom,
                self.camera_offset.z * self.zoom,
            )
        };
        if source.z.abs() < 1.0e-4 {
            source.z = DEFAULT_CAMERA_HEIGHT.max(1.0);
        }

        let factor = 1.0 - (self.ground_level / source.z);
        source = rotate_vec_around_x(source, self.pitch_angle);
        source = rotate_vec_around_z(source, self.angle);
        source *= factor;
        source.x += pos.x;
        source.y += pos.y;
        source.z += self.ground_level;

        let mut target = Vec3::new(pos.x, pos.y, self.ground_level);
        let mut fx_pitch = self.fx_pitch;
        if self.use_real_zoom_cam {
            let mut pitch_adjust = 1.0;
            if !is_display_letter_boxed() {
                let capped = self.zoom.clamp(MIN_CAPPED_ZOOM, 1.0);
                source.z *= 0.5 + capped * 0.5;
                pitch_adjust = capped;
            }
            fx_pitch = 0.25 + pitch_adjust * 0.75;
            let denom = fx_pitch.max(1.0e-4);
            source.x = target.x + (source.x - target.x) / denom;
            source.y = target.y + (source.y - target.y) / denom;
        } else if fx_pitch <= 1.0 {
            let height = (source.z - target.z) * fx_pitch;
            target.z = source.z - height;
        } else {
            source.x = target.x + (source.x - target.x) / fx_pitch;
            source.y = target.y + (source.y - target.y) / fx_pitch;
        }

        (
            Point3::new(source.x, source.y, source.z),
            Point3::new(target.x, target.y, target.z),
        )
    }

    fn camera_position_vec3(&self) -> Vec3 {
        let camera = self.get_3d_camera_position();
        Vec3::new(camera.x, camera.y, camera.z)
    }

    fn camera_target_vec3(&self) -> Vec3 {
        let target = self.build_camera_eye_and_target().1;
        Vec3::new(target.x, target.y, target.z)
    }

    fn view_matrix(&self) -> Mat4 {
        let eye = self.camera_position_vec3();
        let mut target = self.camera_target_vec3();

        // Keep look-at stable when eye and target converge.
        if (target - eye).length_squared() < 1.0e-6 {
            target = eye + Vec3::Y;
        }

        Mat4::look_at_rh(eye, target, Vec3::Z)
    }

    fn projection_matrix(&self) -> Mat4 {
        let width = self.width.max(1) as f32;
        let height = self.height.max(1) as f32;
        let aspect = width / height;

        match self.projection_mode {
            ProjectionMode::Perspective => {
                let hfov = self.fov.clamp(0.1, PI - 0.1);
                let vfov = vertical_fov_from_horizontal(hfov, aspect);
                Mat4::perspective_rh_gl(vfov, aspect, DEFAULT_NEAR_CLIP, DEFAULT_FAR_CLIP)
            }
            ProjectionMode::Orthographic => {
                let ortho_scale = self.zoom.max(0.1);
                let half_w = width * 0.5 * ortho_scale;
                let half_h = height * 0.5 * ortho_scale;
                Mat4::orthographic_rh_gl(
                    -half_w,
                    half_w,
                    -half_h,
                    half_h,
                    DEFAULT_NEAR_CLIP,
                    DEFAULT_FAR_CLIP,
                )
            }
        }
    }

    fn view_projection_matrix(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// Basic world-to-screen transformation
    pub fn world_to_screen(&self, world: &Point3) -> Option<IPoint2> {
        match self.world_to_screen_tri_return(world) {
            (WorldToScreenReturn::InsideFrustum, Some(screen)) => Some(screen),
            _ => None,
        }
    }

    /// Transform world coordinate to screen with detailed return information
    pub fn world_to_screen_tri_return(
        &self,
        world: &Point3,
    ) -> (WorldToScreenReturn, Option<IPoint2>) {
        if self.width <= 0 || self.height <= 0 {
            return (WorldToScreenReturn::Invalid, None);
        }

        let world_vec = Vec4::new(world.x, world.y, world.z, 1.0);
        let clip = self.view_projection_matrix() * world_vec;
        if !clip.is_finite() || clip.w.abs() < 1.0e-6 {
            return (WorldToScreenReturn::Invalid, None);
        }
        if self.projection_mode == ProjectionMode::Perspective && clip.w <= 0.0 {
            return (WorldToScreenReturn::Invalid, None);
        }

        let ndc = clip.truncate() / clip.w;
        if !ndc.is_finite() {
            return (WorldToScreenReturn::Invalid, None);
        }

        let screen_x = ((ndc.x + 1.0) * 0.5 * self.width as f32 + self.origin_x as f32).round();
        let screen_y = ((1.0 - ndc.y) * 0.5 * self.height as f32 + self.origin_y as f32).round();

        let in_bounds = ndc.x >= -1.0
            && ndc.x <= 1.0
            && ndc.y >= -1.0
            && ndc.y <= 1.0
            && ndc.z >= -1.0
            && ndc.z <= 1.0;

        let return_type = if in_bounds {
            WorldToScreenReturn::InsideFrustum
        } else {
            WorldToScreenReturn::OutsideFrustum
        };

        (
            return_type,
            Some(IPoint2::new(screen_x as i32, screen_y as i32)),
        )
    }

    /// Transform screen coordinate to world coordinate at specific Z height
    pub fn screen_to_world_at_z(&self, screen: &IPoint2, z: f32) -> Result<Point3, ViewError> {
        if self.width <= 0 || self.height <= 0 {
            return Err(ViewError::NotInitialized);
        }

        let x = ((screen.x - self.origin_x) as f32 / self.width as f32) * 2.0 - 1.0;
        let y = 1.0 - ((screen.y - self.origin_y) as f32 / self.height as f32) * 2.0;

        let inverse = self.view_projection_matrix().inverse();
        if !inverse.is_finite() {
            return Err(ViewError::InvalidTransformation);
        }

        let near_clip = Vec4::new(x, y, -1.0, 1.0);
        let far_clip = Vec4::new(x, y, 1.0, 1.0);

        let near_world4 = inverse * near_clip;
        let far_world4 = inverse * far_clip;
        if near_world4.w.abs() < 1.0e-6 || far_world4.w.abs() < 1.0e-6 {
            return Err(ViewError::InvalidTransformation);
        }

        let near_world = near_world4.truncate() / near_world4.w;
        let far_world = far_world4.truncate() / far_world4.w;
        let direction = far_world - near_world;

        if direction.z.abs() < 1.0e-6 {
            return Err(ViewError::OutOfRange);
        }

        let t = (z - near_world.z) / direction.z;
        if !t.is_finite() {
            return Err(ViewError::InvalidTransformation);
        }

        let hit = near_world + direction * t;
        Ok(Point3::new(hit.x, hit.y, z))
    }

    /// Transform screen coordinate to world coordinate (on terrain)
    pub fn screen_to_world(&self, screen: &IPoint2) -> Result<Point3, ViewError> {
        self.screen_to_world_at_z(screen, 0.0)
    }

    /// Transform screen coordinate to a point on the 3D heightmap.
    ///
    /// C++ `W3DView::screenToTerrain`: pick ray through `TheTerrainRenderObject->Cast_Ray`.
    pub fn screen_to_terrain(&self, screen: &IPoint2) -> Result<Point3, ViewError> {
        if let Some((_, world)) = self
            .location_requests
            .iter()
            .rev()
            .find(|(cached, _)| cached.x == screen.x && cached.y == screen.y)
        {
            return Ok(*world);
        }

        let (ray_start, ray_end) = self.get_pick_ray(screen)?;
        let hit = if let Some(intersection) = intersect_terrain_ray(ray_start, ray_end) {
            Point3::new(intersection.x, intersection.y, intersection.z)
        } else {
            self.screen_to_world_at_z(screen, 0.0)?
        };
        Ok(hit)
    }

    /// C++ `W3DView::getPickRay`.
    pub fn get_pick_ray(&self, screen: &IPoint2) -> Result<(Vec3, Vec3), ViewError> {
        if self.width <= 0 || self.height <= 0 {
            return Err(ViewError::NotInitialized);
        }
        let x = ((screen.x - self.origin_x) as f32 / self.width as f32) * 2.0 - 1.0;
        let y = 1.0 - ((screen.y - self.origin_y) as f32 / self.height as f32) * 2.0;
        let inverse = self.view_projection_matrix().inverse();
        if !inverse.is_finite() {
            return Err(ViewError::InvalidTransformation);
        }
        let near_clip = Vec4::new(x, y, -1.0, 1.0);
        let far_clip = Vec4::new(x, y, 1.0, 1.0);
        let near_world4 = inverse * near_clip;
        let far_world4 = inverse * far_clip;
        if near_world4.w.abs() < 1.0e-6 || far_world4.w.abs() < 1.0e-6 {
            return Err(ViewError::InvalidTransformation);
        }
        let near_world = near_world4.truncate() / near_world4.w;
        let far_world = far_world4.truncate() / far_world4.w;
        let mut dir = far_world - near_world;
        if dir.length_squared() < 1.0e-12 {
            return Err(ViewError::InvalidTransformation);
        }
        dir = dir.normalize() * DEFAULT_FAR_CLIP;
        let start = self.camera_position_vec3();
        Ok((start, start + dir))
    }

    /// C++ `W3DView::pickDrawable`: skip opaque GUI, cast a 3D ray through drawables.
    pub fn pick_drawable(
        &self,
        screen: &IPoint2,
        _force_attack: bool,
        _pick_type: PickType,
        window_query: &mut dyn FnMut(i32, i32) -> bool,
    ) -> Option<u32> {
        with_drawable_manager_ref(|manager| {
            self.pick_drawable_with_manager(
                screen,
                _force_attack,
                _pick_type,
                window_query,
                manager,
            )
        })
    }

    pub fn pick_drawable_with_manager(
        &self,
        screen: &IPoint2,
        _force_attack: bool,
        _pick_type: PickType,
        window_query: &mut dyn FnMut(i32, i32) -> bool,
        manager: &crate::drawable::DrawableManager,
    ) -> Option<u32> {
        if window_query(screen.x, screen.y) {
            return None;
        }
        let (ray_start, ray_end) = self.get_pick_ray(screen).ok()?;
        let dir = ray_end - ray_start;
        let dir_len_sq = dir.length_squared();
        if dir_len_sq < 1.0e-8 {
            return None;
        }

        let mut best_id = None;
        let mut best_t = f32::MAX;
        for id in manager.get_all_drawable_ids() {
            let Some(drawable) = manager.get_drawable(id) else {
                continue;
            };
            if !drawable.is_visible() {
                continue;
            }
            let (center, radius) = drawable.get_bounding_sphere();
            let sphere_center = Vec3::new(center.x, center.y, center.z);
            let radius = radius.max(1.0);
            if let Some(t) = ray_sphere_t(ray_start, dir, sphere_center, radius) {
                if t >= 0.0 && t <= 1.0 && t < best_t {
                    best_t = t;
                    best_id = Some(drawable.get_object_id().unwrap_or(drawable.get_id().0));
                }
            }
        }
        best_id
    }

    /// C++ `W3DView::iterateDrawablesInRegion`: project drawable centers into the screen box.
    pub fn iterate_drawables_in_region(
        &self,
        region: Option<(IPoint2, IPoint2)>,
        window_query: &mut dyn FnMut(i32, i32) -> bool,
    ) -> Vec<u32> {
        with_drawable_manager_ref(|manager| {
            self.iterate_drawables_in_region_with_manager(region, window_query, manager)
        })
    }

    pub fn iterate_drawables_in_region_with_manager(
        &self,
        region: Option<(IPoint2, IPoint2)>,
        window_query: &mut dyn FnMut(i32, i32) -> bool,
        manager: &crate::drawable::DrawableManager,
    ) -> Vec<u32> {
        if let Some((lo, hi)) = region {
            if lo.x == hi.x && lo.y == hi.y {
                return self
                    .pick_drawable_with_manager(
                        &lo,
                        true,
                        PickType::Selectable,
                        window_query,
                        manager,
                    )
                    .into_iter()
                    .collect();
            }
        }

        let mut ids = Vec::new();
        for id in manager.get_all_drawable_ids() {
            let Some(drawable) = manager.get_drawable(id) else {
                continue;
            };
            if !drawable.is_visible() {
                continue;
            }
            let pos = drawable.get_position();
            let point = Point3::new(pos.x, pos.y, pos.z);
            let Some(screen) = self.world_to_screen(&point) else {
                continue;
            };
            let inside = match region {
                None => true,
                Some((lo, hi)) => {
                    let min_x = lo.x.min(hi.x);
                    let max_x = lo.x.max(hi.x);
                    let min_y = lo.y.min(hi.y);
                    let max_y = lo.y.max(hi.y);
                    screen.x >= min_x && screen.x <= max_x && screen.y >= min_y && screen.y <= max_y
                }
            };
            if inside {
                ids.push(drawable.get_object_id().unwrap_or(drawable.get_id().0));
            }
        }
        ids
    }

    /// CPU fade + type used by Display to composite viewport filters.
    pub fn filter_composite(&self) -> ViewFilterComposite {
        self.filter_state.composite(self.scroll_amount)
    }

    pub fn filter_post_render(&mut self, logic_frame: u32) -> super::view_filter::FilterPostRender {
        self.filter_state
            .post_render(logic_frame, self.scroll_amount)
    }

    /// Get the four corner points of the view projected into world space at given Z
    pub fn get_screen_corner_world_points_at_z(&self, z: f32) -> Result<[Point3; 4], ViewError> {
        let (origin_x, origin_y) = self.origin();

        let top_left = IPoint2::new(origin_x, origin_y);
        let top_right = IPoint2::new(origin_x + self.width, origin_y);
        let bottom_left = IPoint2::new(origin_x, origin_y + self.height);
        let bottom_right = IPoint2::new(origin_x + self.width, origin_y + self.height);

        Ok([
            self.screen_to_world_at_z(&top_left, z)?,
            self.screen_to_world_at_z(&top_right, z)?,
            self.screen_to_world_at_z(&bottom_left, z)?,
            self.screen_to_world_at_z(&bottom_right, z)?,
        ])
    }

    pub fn guard_band_bias(&self) -> Vector2 {
        self.guard_band_bias
    }

    /// Save current view location
    pub fn get_location(&self) -> ViewLocation {
        let mut location = ViewLocation::new();
        location.init(
            self.position.x,
            self.position.y,
            self.position.z,
            self.angle,
            self.pitch_angle,
            self.zoom,
        );
        location
    }

    /// Restore view from saved location
    pub fn set_location(&mut self, location: &ViewLocation) {
        if location.is_valid() {
            self.set_position(location.position());
            self.set_angle(location.angle());
            self.set_pitch(location.pitch());
            self.set_zoom(location.zoom());
            self.force_redraw();
        }
    }

    /// Set the guard band bias for rendering margins
    pub fn set_guard_band_bias(&mut self, bias: Vector2) {
        self.guard_band_bias = bias;
    }

    /// Get current projection mode
    pub fn projection_mode(&self) -> ProjectionMode {
        self.projection_mode
    }

    /// Set projection mode
    pub fn set_projection_mode(&mut self, mode: ProjectionMode) {
        self.projection_mode = mode;
    }

    pub fn get_view_filter_type(&self) -> FilterType {
        self.filter_state.filter()
    }
    pub fn get_view_filter_mode(&self) -> FilterMode {
        self.filter_state.mode()
    }
    pub fn set_view_filter_mode(&mut self, mode: FilterMode) -> bool {
        self.filter_state.set_mode(mode)
    }
    pub fn set_view_filter_pos(&mut self, pos: &Point3) {
        self.filter_state.set_pos(*pos);
    }
    pub fn set_view_filter(&mut self, filter: FilterType) -> bool {
        self.filter_state.set_filter(filter)
    }
    pub fn set_fade_parameters(&mut self, frames: i32, direction: i32) {
        self.filter_state.set_fade_parameters(frames, direction);
    }
    pub fn tick_filter_fade(&mut self) {
        self.filter_state.tick_fade();
    }

    /// Mirrors `W3DView::set3DWireFrameMode`.
    pub fn set_3d_wireframe_mode(&mut self, enable: bool) {
        self.wireframe_next_enabled = enable;
        self.wireframe_pending_frames = 2;
    }

    /// Clears any pending wireframe transition and disables wireframe immediately.
    pub fn reset_3d_wireframe_mode(&mut self) {
        self.wireframe_enabled = false;
        self.wireframe_next_enabled = false;
        self.wireframe_pending_frames = 0;
    }

    /// Returns the currently active 3D wireframe state.
    pub fn is_3d_wireframe_mode(&self) -> bool {
        self.wireframe_enabled
    }

    /// Returns the wireframe state that will be applied once the pending update expires.
    pub fn pending_3d_wireframe_mode(&self) -> bool {
        if self.wireframe_pending_frames > 0 {
            self.wireframe_next_enabled
        } else {
            self.wireframe_enabled
        }
    }

    pub fn set_motion_blur_follow_mode(&mut self, amount: i32) {
        self.filter_state.set_motion_blur_follow(amount);
    }

    pub fn is_time_frozen(&self) -> bool {
        self.freeze_time_for_camera_movement
    }

    pub fn camera_mod_freeze_time(&mut self) {
        self.freeze_time_for_camera_movement = true;
        if !self.is_camera_movement_finished() {
            self.freeze_time_for_camera_movement_active = true;
        }
    }

    pub fn camera_mod_freeze_angle(&mut self) {
        if let Some(rotate) = &mut self.camera_rotate {
            rotate.freeze_current_angle();
        }
        if let Some(path) = &mut self.camera_path {
            path.freeze_angles_to_start();
        }
    }

    /// Check if point is within view frustum (simplified)
    pub fn is_point_in_frustum(&self, point: &Point3) -> bool {
        matches!(
            self.world_to_screen_tri_return(point).0,
            WorldToScreenReturn::InsideFrustum
        )
    }

    /// Calculate distance from camera to point
    pub fn distance_to_point(&self, point: &Point3) -> f32 {
        let camera_pos = self.get_3d_camera_position();
        point.distance(&camera_pos)
    }

    // Debug accessors
    pub fn terrain_height_under_camera(&self) -> f32 {
        self.terrain_height_under_camera
    }
    pub fn set_terrain_height_under_camera(&mut self, height: f32) {
        self.terrain_height_under_camera = height;
    }
    pub fn current_height_above_ground(&self) -> f32 {
        self.current_height_above_ground
    }
    pub fn set_current_height_above_ground(&mut self, height: f32) {
        self.current_height_above_ground = height;
    }

    /// Update the view state (called once per frame)
    pub fn update_view(&mut self) {
        self.terrain_height_under_camera = height_around_pos(self.position.x, self.position.y);
        self.current_height_above_ground =
            self.camera_offset.z * self.zoom - self.terrain_height_under_camera;

        let mut camera_path_active = false;
        if let Some(mut path) = self.camera_path.take() {
            let finished = path.update(FRAME_LENGTH_MS as i32);
            let pos = path.get_current_position();
            self.position = Point3::new(pos.x, pos.y, pos.z);
            // C++ W3DView.cpp:3097-3212 widens m_cameraConstraint so scripted pans
            // can leave the map ("assuming the scripter knows what he is doing").
            self.widen_camera_constraint_for_scripted(pos.x, pos.y);
            if path.is_oriented() {
                self.angle = path.get_current_angle();
            }
            if !finished {
                camera_path_active = true;
                self.camera_path = Some(path);
            }
        }

        if !camera_path_active {
            // Apply position transition
            if let Some(transition) = &mut self.camera_move {
                let finished = transition.update();
                let pos = transition.get_current_position();
                self.position = Point3::new(pos.x, pos.y, pos.z);
                self.widen_camera_constraint_for_scripted(pos.x, pos.y);
                if finished {
                    self.camera_move = None;
                }
            }

            // Apply rotation transition
            if let Some(transition) = &mut self.camera_rotate {
                let finished = transition.update();
                self.angle = transition.get_current_angle();
                if finished {
                    self.camera_rotate = None;
                }
            }
        }

        // Apply zoom transition
        if let Some(mut transition) = self.camera_zoom.take() {
            let finished = transition.update();
            let zoom = transition.get_current_zoom();
            // C++ zoomCameraOneFrame writes m_zoom; setZoom would cancel this transition.
            if self.zoom_limited {
                self.zoom = zoom.clamp(self.min_zoom, self.max_zoom);
            } else {
                self.zoom = zoom;
            }
            self.rebuild_real_zoom_fov();
            if !finished {
                self.camera_zoom = Some(transition);
            }
        }

        // Apply pitch transition — C++ pitchCameraOneFrame writes m_FXPitch.
        if let Some(mut transition) = self.camera_pitch.take() {
            let finished = transition.update();
            let pitch = transition.get_current_pitch();
            self.fx_pitch = pitch;
            if !finished {
                self.camera_pitch = Some(transition);
            }
        }

        self.rotate_camera_toward_one_frame();

        // C++ W3DView::update LOCK_FOLLOW / LOCK_TETHER (1101-1254).
        self.apply_camera_lock_one_frame();
        self.settle_zoom_toward_height_above_ground();

        // Process camera shake (position offsets)
        self.tick_impulse_shake();

        // Screen filter fade advances at successful postRender, not view update.

        if self.wireframe_pending_frames > 0 {
            self.wireframe_pending_frames -= 1;
            if self.wireframe_pending_frames == 0 {
                self.wireframe_enabled = self.wireframe_next_enabled;
            }
        }

        if self.freeze_time_for_camera_movement_active && self.is_camera_movement_finished() {
            self.freeze_time_for_camera_movement = false;
            self.freeze_time_for_camera_movement_active = false;
        }

        if self.is_camera_slaved {
            self.apply_slave_camera();
        }
        if self.use_real_zoom_cam {
            self.rebuild_real_zoom_fov();
        }
    }

    /// Force a redraw of the view.
    pub fn force_redraw(&self) {
        // Keep the explicit redraw hook for legacy callers that expect immediate refresh.
        log::trace!("View {} requested redraw", self.id);
    }
}

fn with_hint_model_cache<R>(f: impl FnOnce(&mut HintModelCache) -> R) -> R {
    #[cfg(test)]
    {
        return TEST_HINT_MODELS.with(|cache| f(&mut cache.borrow_mut()));
    }
    #[cfg(not(test))]
    {
        if crate::core::game_client::live_game_client_frame().is_some() {
            return crate::core::game_client::with_live_game_client_mut(|client| {
                f(&mut client.hint_model_cache)
            })
            .expect("live client frame implies registered client");
        }
        let mut ephemeral = HintModelCache::default();
        f(&mut ephemeral)
    }
}

#[cfg(test)]
thread_local! {
    static TEST_HINT_MODELS: RefCell<HintModelCache> = RefCell::new(HintModelCache::default());
}

#[derive(Default)]
pub struct HintModelCache {
    move_hints: Vec<Option<crate::drawable::DrawableId>>,
    locater_anchor: Option<crate::drawable::DrawableId>,
    locater_arrow: Option<crate::drawable::DrawableId>,
}

impl HintModelCache {
    pub(super) fn clear(&mut self) {
        self.move_hints.clear();
        self.locater_anchor = None;
        self.locater_arrow = None;
    }
}

/// Access the receiving client's tactical view (legacy `TheTacticalView`).
pub fn with_tactical_view<R>(f: impl FnOnce(&mut View) -> R) -> R {
    // Unit tests keep the historical free-API TLS so ambient live-slot fixtures
    // do not steal TheTacticalView. Production routes through the live client.
    #[cfg(test)]
    {
        return TEST_TACTICAL_VIEW.with(|view| f(&mut view.borrow_mut()));
    }
    #[cfg(not(test))]
    {
        if crate::core::game_client::live_game_client_frame().is_some() {
            return crate::core::game_client::with_live_game_client_mut(|client| {
                f(&mut client.tactical_view)
            })
            .expect("live client frame implies registered client");
        }
        let mut ephemeral = View::new();
        ephemeral.init();
        f(&mut ephemeral)
    }
}

/// Access the receiving client's tactical view immutably.
pub fn with_tactical_view_ref<R>(f: impl FnOnce(&View) -> R) -> R {
    with_tactical_view(|view| f(view))
}

#[cfg(test)]
thread_local! {
    static TEST_TACTICAL_VIEW: RefCell<View> = {
        let mut view = View::new();
        view.init();
        RefCell::new(view)
    };
}

impl Default for View {
    fn default() -> Self {
        Self::new()
    }
}




/// Trait for objects that can be rendered by the Display system
/// This allows the concrete View struct to work with the Display's generic view management
pub trait ViewTrait: Send + Sync {
    /// Get the unique ID of this view
    fn id(&self) -> ViewId;

    /// Get the dimensions of this view
    fn dimensions(&self) -> (i32, i32);

    /// Get the origin position on the display
    fn origin(&self) -> (i32, i32);

    /// Draw this view (called by the Display system)
    fn draw_view(
        &self,
        ui: Option<&crate::core::subsystems::InGameUISubsystem>,
        client_frame: u32,
    ) -> Result<(), ViewError>;

    fn draw_view_owned(
        &self,
        ui: Option<&crate::core::subsystems::InGameUISubsystem>,
        client_frame: u32,
        manager: &mut crate::drawable::DrawableManager,
        hint_models: &mut HintModelCache,
    ) -> Result<(), ViewError> {
        let _ = (manager, hint_models);
        self.draw_view(ui, client_frame)
    }

    /// Update view state (called once per frame)
    fn update_view(&mut self) -> Result<(), ViewError>;

    /// Reset view state to defaults
    fn reset_view(&mut self);

    /// Force a redraw of this view
    fn force_redraw(&self);

    /// Get the world position this view is looking at
    fn position(&self) -> Point3;

    /// Set the world position this view should look at
    fn set_position(&mut self, pos: Point3);
}

/// Implementation of ViewTrait for the concrete View struct
impl ViewTrait for View {
    fn id(&self) -> ViewId {
        self.id()
    }

    fn dimensions(&self) -> (i32, i32) {
        (self.width(), self.height())
    }

    fn origin(&self) -> (i32, i32) {
        self.origin()
    }

    fn draw_view(
        &self,
        ui: Option<&crate::core::subsystems::InGameUISubsystem>,
        client_frame: u32,
    ) -> Result<(), ViewError> {
        if let Some(ui) = ui {
            draw_move_hint_and_locater_models(self, ui, client_frame);
        }
        View::force_redraw(self);
        Ok(())
    }

    fn draw_view_owned(
        &self,
        ui: Option<&crate::core::subsystems::InGameUISubsystem>,
        client_frame: u32,
        manager: &mut crate::drawable::DrawableManager,
        hint_models: &mut HintModelCache,
    ) -> Result<(), ViewError> {
        if let Some(ui) = ui {
            draw_move_hint_and_locater_models_with_owner(
                self,
                ui,
                client_frame,
                manager,
                hint_models,
            );
        }
        View::force_redraw(self);
        Ok(())
    }

    fn update_view(&mut self) -> Result<(), ViewError> {
        View::update_view(self);
        Ok(())
    }

    fn reset_view(&mut self) {
        self.reset();
    }

    fn force_redraw(&self) {
        View::force_redraw(self)
    }

    fn position(&self) -> Point3 {
        *self.position()
    }

    fn set_position(&mut self, pos: Point3) {
        self.set_position(&pos);
    }

}

#[cfg(test)]
mod tactical_view_ownership_tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[test]
    fn tactical_view_two_threads_do_not_share_state() {
        let barrier = Arc::new(Barrier::new(2));
        let mut joins = Vec::new();
        for thread_idx in 0..2 {
            let barrier = Arc::clone(&barrier);
            joins.push(std::thread::spawn(move || {
                crate::test_support::ensure_ambient_host();
                barrier.wait();
                let x = 10.0 + thread_idx as f32;
                with_tactical_view(|view| {
                    view.set_position(&Point3::new(x, 0.0, 0.0));
                });
                let got = with_tactical_view_ref(|view| view.position().x);
                assert!(
                    (got - x).abs() < 0.01,
                    "thread {thread_idx} expected {x} got {got}"
                );
            }));
        }
        for j in joins {
            j.join().unwrap();
        }
    }
}
