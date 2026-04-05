//! # View Module
//!
//! Provides camera and viewport management for 3D RTS gameplay.
//! Handles 3D perspective and orthographic projections, camera movement,
//! rotation, zooming, view frustum culling, and world-screen transformations.

use std::cell::RefCell;
use std::f32::consts::PI;
use std::sync::atomic::{AtomicU32, Ordering};

use game_engine::common::ini::get_global_data;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::display::cinematic_camera::{
    CameraPath, CameraPitchTransition, CameraPositionTransition, CameraRotateTransition,
    CameraWaypoint, CameraZoomTransition,
};
use gamelogic::helpers::TheGameLogic;
use glam::{Mat4, Vec3, Vec4};
use rand::random;

/// Unique identifier for view instances
pub type ViewId = u32;

/// Default view dimensions and settings
pub const DEFAULT_VIEW_WIDTH: i32 = 640;
pub const DEFAULT_VIEW_HEIGHT: i32 = 480;
pub const DEFAULT_VIEW_ORIGIN_X: i32 = 0;
pub const DEFAULT_VIEW_ORIGIN_Y: i32 = 0;
pub const DEFAULT_FOV_DEGREES: f32 = 50.0;
pub const DEFAULT_FOV_RADIANS: f32 = DEFAULT_FOV_DEGREES * PI / 180.0;
const LOGIC_FRAMES_PER_SECOND: f32 = 30.0;
const FRAME_LENGTH_MS: f32 = 1000.0 / LOGIC_FRAMES_PER_SECOND;
const DEFAULT_NEAR_CLIP: f32 = 1.0;
const DEFAULT_FAR_CLIP: f32 = 20000.0;

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

/// Basic 3D point
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Point3 {
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

    pub fn origin() -> Self {
        Self::zero()
    }
}

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

impl std::ops::Add<Vector3> for Point3 {
    type Output = Point3;

    fn add(self, vec: Vector3) -> Point3 {
        Point3::new(self.x + vec.x, self.y + vec.y, self.z + vec.z)
    }
}

impl std::ops::Sub for Point3 {
    type Output = Vector3;

    fn sub(self, other: Point3) -> Vector3 {
        Vector3::new(self.x - other.x, self.y - other.y, self.z - other.z)
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    Null,
    BlackAndWhite,
    Crossfade,
    MotionBlur,
}

/// Viewport post-process filter mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    camera_lock_type: CameraLockType,
    lock_distance: f32,
    snap_immediate: bool,

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
    view_filter_type: FilterType,
    view_filter_mode: FilterMode,
    view_filter_pos: Point3,
    fade_total_frames: i32,
    fade_progress_frames: i32,
    fade_direction: i32,
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
        static NEXT_ID: AtomicU32 = AtomicU32::new(1);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

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
            camera_lock_type: CameraLockType::Follow,
            lock_distance: 0.0,
            snap_immediate: false,
            mouse_locked: false,
            ok_to_adjust_height: true,
            projection_mode: ProjectionMode::Perspective,
            guard_band_bias: Vector2::new(0.0, 0.0),
            terrain_height_under_camera: 0.0,
            current_height_above_ground: 0.0,
            view_filter_type: FilterType::Null,
            view_filter_mode: FilterMode::Null,
            view_filter_pos: Point3::zero(),
            fade_total_frames: 0,
            fade_progress_frames: 0,
            fade_direction: 0,
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
        self.zoom_limited = true;

        self.zoom = self.max_zoom;
        self.ok_to_adjust_height = false;
        self.default_angle = 0.0;
        self.default_pitch_angle = 0.0;
    }

    /// Reset the view to default state
    pub fn reset(&mut self) {
        self.zoom_limited = true;
        self.camera_path = None;
        self.view_filter_type = FilterType::Null;
        self.view_filter_mode = FilterMode::Null;
        self.fade_total_frames = 0;
        self.fade_progress_frames = 0;
        self.fade_direction = 0;
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

    /// Center the view on the given world coordinate.
    ///
    /// C++ parity: `View::lookAt` stores the view's top-left world origin,
    /// keeping Z unchanged and offsetting by half the current view size.
    pub fn look_at(&mut self, target: &Point3) {
        self.position.x = target.x - self.width as f32 * 0.5;
        self.position.y = target.y - self.height as f32 * 0.5;
    }

    /// Scroll the view by a 2D delta
    pub fn scroll_by(&mut self, delta: &Vector2) {
        self.position.x += delta.x;
        self.position.y += delta.y;
    }

    // Angle and rotation
    pub fn angle(&self) -> f32 {
        self.angle
    }
    pub fn set_angle(&mut self, angle: f32) {
        self.angle = angle;
    }

    pub fn pitch(&self) -> f32 {
        self.pitch_angle
    }
    pub fn set_pitch(&mut self, pitch: f32) {
        // Limit pitch to reasonable range for RTS camera
        let limit = PI / 5.0; // 36 degrees
        self.pitch_angle = pitch.clamp(-limit, limit);
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
        if self.zoom_limited {
            self.zoom = zoom.clamp(self.min_zoom, self.max_zoom);
        } else {
            self.zoom = zoom;
        }
    }

    pub fn height_above_ground(&self) -> f32 {
        self.height_above_ground
    }
    pub fn set_height_above_ground(&mut self, height: f32) {
        self.height_above_ground =
            height.clamp(self.min_height_above_ground, self.max_height_above_ground);
    }

    pub fn zoom_in(&mut self) {
        self.set_height_above_ground(self.height_above_ground - 10.0);
    }

    pub fn zoom_out(&mut self) {
        self.set_height_above_ground(self.height_above_ground + 10.0);
    }

    pub fn set_zoom_to_default(&mut self) {
        self.set_zoom(self.max_zoom);
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

    // Field of view
    pub fn field_of_view(&self) -> f32 {
        self.fov
    }
    pub fn set_field_of_view(&mut self, fov_radians: f32) {
        self.fov = fov_radians.clamp(0.1, PI - 0.1);
    }

    // Camera locking
    pub fn camera_lock_id(&self) -> Option<u32> {
        self.camera_lock_id
    }
    pub fn set_camera_lock(&mut self, id: Option<u32>) {
        self.camera_lock_id = id;
        self.lock_distance = 0.0;
        self.camera_lock_type = CameraLockType::Follow;
    }

    pub fn snap_to_camera_lock(&mut self) {
        self.snap_immediate = true;
    }

    pub fn set_snap_mode(&mut self, lock_type: CameraLockType, distance: f32) {
        self.camera_lock_type = lock_type;
        self.lock_distance = distance;
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

    /// Get the actual 3D camera position in world space
    pub fn get_3d_camera_position(&self) -> Point3 {
        // Calculate actual camera position based on look-at point, angles, and zoom
        let distance = self.height_above_ground + self.zoom * 100.0;

        // Create camera position offset from look-at point
        let offset = Vector3::new(
            -self.angle.sin() * self.pitch_angle.cos() * distance,
            -self.angle.cos() * self.pitch_angle.cos() * distance,
            self.pitch_angle.sin() * distance + self.terrain_height_under_camera,
        );

        let mut target = self.position;
        target.x += self.shake_offset.x;
        target.y += self.shake_offset.y;
        target + offset
    }

    fn camera_position_vec3(&self) -> Vec3 {
        let camera = self.get_3d_camera_position();
        Vec3::new(camera.x, camera.y, camera.z)
    }

    fn camera_target_vec3(&self) -> Vec3 {
        Vec3::new(
            self.position.x + self.shake_offset.x,
            self.position.y + self.shake_offset.y,
            self.position.z,
        )
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
            ProjectionMode::Perspective => Mat4::perspective_rh_gl(
                self.fov.clamp(0.1, PI - 0.1),
                aspect,
                DEFAULT_NEAR_CLIP,
                DEFAULT_FAR_CLIP,
            ),
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

    /// Transform screen coordinate to point on terrain (Z=0)
    pub fn screen_to_terrain(&self, screen: &IPoint2) -> Result<Point3, ViewError> {
        self.screen_to_world_at_z(screen, 0.0)
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
        self.view_filter_type
    }

    pub fn get_view_filter_mode(&self) -> FilterMode {
        self.view_filter_mode
    }

    pub fn set_view_filter_mode(&mut self, filter_mode: FilterMode) -> bool {
        self.view_filter_mode = filter_mode;
        true
    }

    pub fn set_view_filter_pos(&mut self, pos: &Point3) {
        self.view_filter_pos = *pos;
    }

    pub fn set_view_filter(&mut self, filter: FilterType) -> bool {
        self.view_filter_type = filter;
        true
    }

    pub fn set_fade_parameters(&mut self, frames: i32, direction: i32) {
        self.fade_total_frames = frames.max(0);
        self.fade_progress_frames = 0;
        self.fade_direction = direction;
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
        self.set_view_filter_mode(FilterMode::from_pan_amount(amount));
        self.set_view_filter(FilterType::MotionBlur);
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
        (*point - camera_pos).magnitude()
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
        // Calculate current height above ground
        self.current_height_above_ground = self.height_above_ground;

        let mut camera_path_active = false;
        if let Some(mut path) = self.camera_path.take() {
            let finished = path.update(FRAME_LENGTH_MS as i32);
            let pos = path.get_current_position();
            self.position = Point3::new(pos.x, pos.y, pos.z);
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
            self.set_zoom(zoom);
            if !finished {
                self.camera_zoom = Some(transition);
            }
        }

        // Apply pitch transition
        if let Some(mut transition) = self.camera_pitch.take() {
            let finished = transition.update();
            let pitch = transition.get_current_pitch();
            self.set_pitch(pitch);
            if !finished {
                self.camera_pitch = Some(transition);
            }
        }

        self.rotate_camera_toward_one_frame();

        // Update camera following/tether logic if locked to an object.
        if let Some(object_id) = self.camera_lock_id {
            if let Some(object) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(object_guard) = object.read() {
                    let target = object_guard.get_position();
                    let target_center = Point3::new(target.x, target.y, target.z);
                    let current_center = Point3::new(
                        self.position.x + self.width as f32 * 0.5,
                        self.position.y + self.height as f32 * 0.5,
                        self.position.z,
                    );

                    match self.camera_lock_type {
                        CameraLockType::Follow => {
                            if self.snap_immediate {
                                self.look_at(&target_center);
                                self.snap_immediate = false;
                            } else {
                                let blend = 0.25_f32;
                                let next_center = Point3::new(
                                    current_center.x + (target_center.x - current_center.x) * blend,
                                    current_center.y + (target_center.y - current_center.y) * blend,
                                    target_center.z,
                                );
                                self.look_at(&next_center);
                            }
                        }
                        CameraLockType::Tether => {
                            let dx = target_center.x - current_center.x;
                            let dy = target_center.y - current_center.y;
                            let distance = (dx * dx + dy * dy).sqrt();
                            let allowed = self.lock_distance.max(0.0);

                            if distance > allowed && distance > 0.001 {
                                let step = (distance - allowed) / distance;
                                let next_center = Point3::new(
                                    current_center.x + dx * step,
                                    current_center.y + dy * step,
                                    target_center.z,
                                );
                                self.look_at(&next_center);
                            }
                            self.snap_immediate = false;
                        }
                    }
                }
            }
        }

        // Process camera shake (position offsets)
        if self.shake_intensity > 0.01 {
            self.shake_offset.x = self.shake_intensity * self.shake_angle_cos;
            self.shake_offset.y = self.shake_intensity * self.shake_angle_sin;
            self.shake_intensity *= 0.75;
            self.shake_angle_cos = -self.shake_angle_cos;
            self.shake_angle_sin = -self.shake_angle_sin;
        } else {
            self.shake_intensity = 0.0;
            self.shake_offset = Vector2::zero();
        }

        if self.fade_total_frames > 0 {
            self.fade_progress_frames += 1;
            if self.fade_progress_frames >= self.fade_total_frames {
                if self.fade_direction < 0 && self.view_filter_type == FilterType::BlackAndWhite {
                    self.view_filter_mode = FilterMode::Null;
                    self.view_filter_type = FilterType::Null;
                }
                self.fade_total_frames = 0;
                self.fade_progress_frames = 0;
            }
        }

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
    }

    /// Force a redraw of the view.
    pub fn force_redraw(&self) {
        // Keep the explicit redraw hook for legacy callers that expect immediate refresh.
        log::trace!("View {} requested redraw", self.id);
    }
}

thread_local! {
    static THE_TACTICAL_VIEW: RefCell<View> = {
        let mut view = View::new();
        view.init();
        RefCell::new(view)
    };
}

/// Access the global tactical view (legacy `TheTacticalView` equivalent).
pub fn with_tactical_view<R>(f: impl FnOnce(&mut View) -> R) -> R {
    THE_TACTICAL_VIEW.with(|view| f(&mut view.borrow_mut()))
}

/// Access the global tactical view immutably.
pub fn with_tactical_view_ref<R>(f: impl FnOnce(&View) -> R) -> R {
    THE_TACTICAL_VIEW.with(|view| f(&view.borrow()))
}

impl Default for View {
    fn default() -> Self {
        Self::new()
    }
}

/// Camera animation and movement utilities
impl View {
    /// Move camera to a position over multiple frames.
    pub fn move_camera_to(
        &mut self,
        target: &Point3,
        milliseconds: i32,
        _shutter: i32,
        _orient: bool,
        ease_in: f32,
        ease_out: f32,
    ) {
        // C++ base behavior for zero-duration calls is immediate lookAt.
        if milliseconds <= 0 {
            self.look_at(target);
            self.camera_move = None;
            return;
        }

        let frames = ms_to_frames(milliseconds);
        let (ease_in, ease_out) = ease_ratios(milliseconds, ease_in, ease_out);
        let end_position = Vec3::new(
            target.x - self.width as f32 * 0.5,
            target.y - self.height as f32 * 0.5,
            self.position.z,
        );

        self.camera_move = Some(CameraPositionTransition::new(
            end_position,
            frames,
            ease_in,
            ease_out,
            Vec3::new(self.position.x, self.position.y, self.position.z),
        ));
        self.camera_path = None;
        if self.freeze_time_for_camera_movement {
            self.freeze_time_for_camera_movement_active = true;
        }
    }

    /// Move camera along an explicit waypoint chain.
    pub fn move_camera_along_waypoint_path(
        &mut self,
        waypoints: &[Point3],
        milliseconds: i32,
        shutter: i32,
        orient: bool,
        ease_in: f32,
        ease_out: f32,
    ) {
        if waypoints.is_empty() {
            return;
        }

        if milliseconds <= 0 || waypoints.len() == 1 {
            self.look_at(waypoints.last().unwrap_or(&waypoints[0]));
            self.camera_path = None;
            return;
        }

        let half_w = self.width as f32 * 0.5;
        let half_h = self.height as f32 * 0.5;

        let mut path = Vec::with_capacity(waypoints.len());
        for index in 0..waypoints.len() {
            let point = waypoints[index];
            let angle = if index + 1 < waypoints.len() {
                let next = waypoints[index + 1];
                let dx = next.x - point.x;
                let dy = next.y - point.y;
                if dx.abs() < f32::EPSILON && dy.abs() < f32::EPSILON {
                    self.angle
                } else {
                    dx.atan2(dy)
                }
            } else {
                self.angle
            };

            path.push(CameraWaypoint {
                position: Vec3::new(point.x - half_w, point.y - half_h, self.position.z),
                angle,
                time_multiplier: 1,
            });
        }

        let (ease_in, ease_out) = ease_ratios(milliseconds, ease_in, ease_out);
        self.camera_move = None;
        self.camera_rotate = None;
        self.camera_path = Some(CameraPath::new(
            path,
            milliseconds.max(1),
            shutter.max(1),
            orient,
            ease_in,
            ease_out,
        ));
        if self.freeze_time_for_camera_movement {
            self.freeze_time_for_camera_movement_active = true;
        }
    }

    /// Check if camera movement animation is finished
    pub fn is_camera_movement_finished(&self) -> bool {
        self.camera_move.is_none()
            && self.camera_path.is_none()
            && self.camera_rotate.is_none()
            && self.camera_zoom.is_none()
            && self.camera_pitch.is_none()
            && self.rotate_camera_toward.is_none()
    }

    /// Move camera to location and restore default orientation/zoom.
    pub fn reset_camera(
        &mut self,
        location: &Point3,
        milliseconds: i32,
        ease_in: f32,
        ease_out: f32,
    ) {
        if milliseconds <= 0 {
            self.look_at(location);
            self.set_angle_and_pitch_to_default();
            self.set_zoom(self.max_zoom);
            self.camera_rotate = None;
            self.camera_zoom = None;
            self.camera_pitch = None;
            return;
        }

        self.move_camera_to(location, milliseconds, 0, false, ease_in, ease_out);
        let frames = ms_to_frames(milliseconds);
        let (ease_in, ease_out) = ease_ratios(milliseconds, ease_in, ease_out);
        let angle_delta = self.default_angle - self.angle;
        self.camera_rotate = Some(CameraRotateTransition::new(
            angle_delta / (2.0 * PI),
            frames,
            ease_in,
            ease_out,
            self.angle,
        ));
        self.camera_zoom = Some(CameraZoomTransition::new(
            self.max_zoom,
            frames,
            ease_in,
            ease_out,
            self.zoom,
        ));
        self.camera_pitch = Some(CameraPitchTransition::new(
            // C++ W3D resetCamera drives pitch endpoint to 1.0f.
            1.0,
            frames,
            ease_in,
            ease_out,
            self.pitch_angle,
        ));
    }

    /// Rotate camera by a number of full rotations.
    pub fn rotate_camera(
        &mut self,
        rotations: f32,
        milliseconds: i32,
        ease_in: f32,
        ease_out: f32,
    ) {
        if milliseconds <= 0 {
            self.set_angle(self.angle + rotations * 2.0 * PI);
            self.camera_rotate = None;
            return;
        }

        let frames = ms_to_frames(milliseconds);
        let (ease_in, ease_out) = ease_ratios(milliseconds, ease_in, ease_out);
        self.camera_rotate = Some(CameraRotateTransition::new(
            rotations, frames, ease_in, ease_out, self.angle,
        ));
        if self.freeze_time_for_camera_movement {
            self.freeze_time_for_camera_movement_active = true;
        }
    }

    /// Zoom camera to a specific level.
    pub fn zoom_camera(&mut self, final_zoom: f32, milliseconds: i32, ease_in: f32, ease_out: f32) {
        if milliseconds <= 0 {
            self.set_zoom(final_zoom);
            self.camera_zoom = None;
            return;
        }

        let frames = ms_to_frames(milliseconds);
        let (ease_in, ease_out) = ease_ratios(milliseconds, ease_in, ease_out);
        self.camera_zoom = Some(CameraZoomTransition::new(
            final_zoom, frames, ease_in, ease_out, self.zoom,
        ));
    }

    /// Pitch camera to a specific angle.
    pub fn pitch_camera(
        &mut self,
        final_pitch: f32,
        milliseconds: i32,
        ease_in: f32,
        ease_out: f32,
    ) {
        if milliseconds <= 0 {
            self.set_pitch(final_pitch);
            self.camera_pitch = None;
            return;
        }

        let frames = ms_to_frames(milliseconds);
        let (ease_in, ease_out) = ease_ratios(milliseconds, ease_in, ease_out);
        self.camera_pitch = Some(CameraPitchTransition::new(
            final_pitch,
            frames,
            ease_in,
            ease_out,
            self.pitch_angle,
        ));
    }

    fn rotate_camera_toward_one_frame(&mut self) {
        let disable_camera = get_global_data()
            .map(|g| g.read().disable_camera_movement)
            .unwrap_or(false);

        let finished = {
            let info = match &mut self.rotate_camera_toward {
                Some(info) => info,
                None => return,
            };

            info.cur_frame += 1;

            if disable_camera {
                info.cur_frame >= info.total_frames()
            } else if info.track_object {
                if info.cur_frame <= info.total_frames() {
                    if let Some(obj_id) = info.target_object_id {
                        if let Some(object) = TheGameLogic::find_object_by_id(obj_id) {
                            if let Ok(guard) = object.read() {
                                let pos = guard.get_position();
                                info.target_position = Point3::new(pos.x, pos.y, pos.z);
                            }
                        }
                    }

                    let center = Point3::new(
                        self.position.x + self.width as f32 * 0.5,
                        self.position.y + self.height as f32 * 0.5,
                        self.position.z,
                    );
                    let dir_x = info.target_position.x - center.x;
                    let dir_y = info.target_position.y - center.y;
                    let dir_length = (dir_x * dir_x + dir_y * dir_y).sqrt();

                    if dir_length >= 0.1 {
                        let mut angle = (dir_x / dir_length).acos();
                        if dir_y < 0.0 {
                            angle = -angle;
                        }
                        angle -= PI / 2.0;
                        angle = normalize_angle(angle);

                        if info.cur_frame <= info.num_frames {
                            let factor =
                                parabolic_ease(info.cur_frame as f32 / info.num_frames as f32);
                            let mut angle_diff = angle - self.angle;
                            angle_diff = normalize_angle(angle_diff);
                            angle_diff *= factor;
                            self.angle += angle_diff;
                            self.angle = normalize_angle(self.angle);
                        } else {
                            self.angle = angle;
                        }
                    }
                }
                info.cur_frame >= info.total_frames()
            } else if info.cur_frame <= info.num_frames {
                let factor = parabolic_ease(info.cur_frame as f32 / info.num_frames as f32);
                self.angle = info.start_angle + (info.end_angle - info.start_angle) * factor;
                self.angle = normalize_angle(self.angle);
                info.cur_frame >= info.total_frames()
            } else {
                true
            }
        };

        if finished {
            let track_object = self
                .rotate_camera_toward
                .as_ref()
                .map_or(false, |i| i.track_object);
            let end_angle = self
                .rotate_camera_toward
                .as_ref()
                .map_or(0.0, |i| i.end_angle);
            self.rotate_camera_toward = None;
            self.freeze_time_for_camera_movement = false;
            if !track_object {
                self.angle = end_angle;
            }
        }
    }

    /// Set final zoom for an active movement (C++ `W3DView::cameraModFinalZoom`).
    pub fn camera_mod_final_zoom(&mut self, final_zoom: f32, ease_in: f32, ease_out: f32) {
        if let Some(rotate_transition) = &self.camera_rotate {
            let time_ms = frames_to_ms(rotate_transition.remaining_frames());
            self.zoom_camera(
                final_zoom,
                time_ms,
                (time_ms as f32) * ease_in,
                (time_ms as f32) * ease_out,
            );
        }
        if let Some(move_transition) = &self.camera_move {
            let time_ms = frames_to_ms(move_transition.remaining_frames());
            self.zoom_camera(
                final_zoom,
                time_ms,
                (time_ms as f32) * ease_in,
                (time_ms as f32) * ease_out,
            );
        }
    }

    /// Set final pitch for an active movement (C++ `W3DView::cameraModFinalPitch`).
    pub fn camera_mod_final_pitch(&mut self, final_pitch: f32, ease_in: f32, ease_out: f32) {
        if let Some(rotate_transition) = &self.camera_rotate {
            let time_ms = frames_to_ms(rotate_transition.remaining_frames());
            self.pitch_camera(
                final_pitch,
                time_ms,
                (time_ms as f32) * ease_in,
                (time_ms as f32) * ease_out,
            );
        }
        if let Some(move_transition) = &self.camera_move {
            let time_ms = frames_to_ms(move_transition.remaining_frames());
            self.pitch_camera(
                final_pitch,
                time_ms,
                (time_ms as f32) * ease_in,
                (time_ms as f32) * ease_out,
            );
        }
    }

    pub fn camera_mod_final_time_multiplier(&mut self, final_multiplier: i32) {
        if let Some(path) = &mut self.camera_path {
            path.set_final_time_multiplier(final_multiplier);
        }
    }

    pub fn camera_mod_rolling_average(&mut self, frames_to_average: i32) {
        if let Some(path) = &mut self.camera_path {
            path.set_rolling_average_frames(frames_to_average);
        }
    }

    /// C++ parity for `W3DView::cameraModLookToward`.
    pub fn camera_mod_look_toward(&mut self, target: &Point3) {
        if self.camera_rotate.is_some() {
            return;
        }
        if let Some(path) = &mut self.camera_path {
            path.camera_mod_look_toward(Vec3::new(target.x, target.y, target.z));
            return;
        }

        if let Some(move_transition) = &self.camera_move {
            let center = Point3::new(
                self.position.x + self.width as f32 * 0.5,
                self.position.y + self.height as f32 * 0.5,
                self.position.z,
            );
            let dir_x = target.x - center.x;
            let dir_y = target.y - center.y;
            if (dir_x * dir_x + dir_y * dir_y).sqrt() < 0.1 {
                return;
            }

            let desired = normalize_angle(dir_y.atan2(dir_x) - PI * 0.5);
            let delta = normalize_angle(desired - self.angle);
            let remaining_ms = frames_to_ms(move_transition.remaining_frames());
            if remaining_ms <= 0 {
                self.angle = desired;
                return;
            }
            self.rotate_camera(delta / (2.0 * PI), remaining_ms, 0.0, 0.0);
        }
    }

    /// C++ parity for `W3DView::cameraModFinalLookToward`.
    pub fn camera_mod_final_look_toward(&mut self, target: &Point3) {
        if self.camera_rotate.is_some() {
            return;
        }
        if let Some(path) = &mut self.camera_path {
            path.camera_mod_final_look_toward(Vec3::new(target.x, target.y, target.z));
            return;
        }

        // `moveCameraTo` in C++ also uses the waypoint camera path code, so final-look modifiers
        // should still affect active simple move transitions.
        self.camera_mod_look_toward(target);
    }

    /// C++ parity for `W3DView::cameraModFinalMoveTo`.
    pub fn camera_mod_final_move_to(&mut self, target: &Point3) {
        if self.camera_rotate.is_some() {
            return;
        }
        if let Some(path) = &mut self.camera_path {
            path.camera_mod_final_move_to(Vec3::new(target.x, target.y, target.z));
            return;
        }

        if let Some(move_transition) = self.camera_move.take() {
            let remaining_frames = move_transition.remaining_frames().max(1);
            let current = move_transition.get_current_position();
            let end_position = Vec3::new(
                target.x - self.width as f32 * 0.5,
                target.y - self.height as f32 * 0.5,
                current.z,
            );
            self.camera_move = Some(CameraPositionTransition::new(
                end_position,
                remaining_frames,
                0.0,
                0.0,
                current,
            ));
        }
    }

    /// C++ parity for `W3DView::rotateCameraTowardObject`.
    pub fn rotate_camera_toward_object(
        &mut self,
        object_id: u32,
        milliseconds: i32,
        hold_milliseconds: i32,
        ease_in: f32,
        ease_out: f32,
    ) {
        let hold_ms = if hold_milliseconds < 1 {
            0
        } else {
            hold_milliseconds
        };
        let num_hold_frames = (hold_ms as f32 / FRAME_LENGTH_MS) as i32;
        let num_hold_frames = num_hold_frames.max(0);

        let ms = if milliseconds < 1 { 1 } else { milliseconds };
        let num_frames = (ms as f32 / FRAME_LENGTH_MS) as i32;
        let num_frames = num_frames.max(1);

        let (ease_in, ease_out) = ease_ratios(ms, ease_in, ease_out);

        self.rotate_camera_toward = Some(RotateCameraToward {
            num_frames,
            cur_frame: 0,
            num_hold_frames,
            ease_in,
            ease_out,
            track_object: true,
            target_object_id: Some(object_id),
            target_position: Point3::zero(),
            start_angle: 0.0,
            end_angle: 0.0,
        });
        self.camera_path = None;
        if self.freeze_time_for_camera_movement {
            self.freeze_time_for_camera_movement_active = true;
        }
    }

    /// C++ parity for `W3DView::rotateCameraTowardPosition`.
    pub fn rotate_camera_toward_position(
        &mut self,
        pos: &Point3,
        milliseconds: i32,
        ease_in: f32,
        ease_out: f32,
        reverse_rotation: bool,
    ) {
        let ms = if milliseconds < 1 { 1 } else { milliseconds };
        let num_frames = (ms as f32 / FRAME_LENGTH_MS) as i32;
        let num_frames = num_frames.max(1);

        let center = Point3::new(
            self.position.x + self.width as f32 * 0.5,
            self.position.y + self.height as f32 * 0.5,
            self.position.z,
        );
        let dir_x = pos.x - center.x;
        let dir_y = pos.y - center.y;
        let dir_length = (dir_x * dir_x + dir_y * dir_y).sqrt();
        if dir_length < 0.1 {
            return;
        }

        let mut angle = (dir_x / dir_length).acos();
        if dir_y < 0.0 {
            angle = -angle;
        }
        angle -= PI / 2.0;
        angle = normalize_angle(angle);

        if reverse_rotation {
            if self.angle < angle {
                angle -= 2.0 * PI;
            } else {
                angle += 2.0 * PI;
            }
        }

        let (ease_in, ease_out) = ease_ratios(ms, ease_in, ease_out);

        self.rotate_camera_toward = Some(RotateCameraToward {
            num_frames,
            cur_frame: 0,
            num_hold_frames: 0,
            ease_in,
            ease_out,
            track_object: false,
            target_object_id: None,
            target_position: *pos,
            start_angle: self.angle,
            end_angle: angle,
        });
        self.camera_path = None;
        if self.freeze_time_for_camera_movement {
            self.freeze_time_for_camera_movement_active = true;
        }
    }

    /// Apply camera shake impulse using the legacy damped-oscillation model.
    pub fn shake(&mut self, _epicenter: &Point3, _shake_type: CameraShakeType) {
        let angle = random::<f32>() * 2.0 * PI;
        self.shake_angle_cos = angle.cos();
        self.shake_angle_sin = angle.sin();

        let data = game_engine::common::global_data::read();
        let mut intensity = match _shake_type {
            CameraShakeType::Subtle => data.shake_subtle_intensity,
            CameraShakeType::Normal => data.shake_normal_intensity,
            CameraShakeType::Strong => data.shake_strong_intensity,
            CameraShakeType::Severe => data.shake_severe_intensity,
            CameraShakeType::CineExtreme => data.shake_cine_extreme_intensity,
            CameraShakeType::CineInsane => data.shake_cine_insane_intensity,
        };

        let dx = _epicenter.x - self.position.x;
        let dy = _epicenter.y - self.position.y;
        let distance = (dx * dx + dy * dy).sqrt();
        if distance > data.max_shake_range {
            return;
        }

        intensity *= 1.0 - (distance / data.max_shake_range);
        self.shake_intensity += intensity;
        if self.shake_intensity > data.max_shake_intensity {
            // C++ parity (W3DView::shake): overflow clamps to fixed 3.0, not to max_shake_intensity.
            self.shake_intensity = 3.0;
        }
    }
}

fn ms_to_frames(milliseconds: i32) -> i32 {
    let ms = milliseconds.max(1) as f32;
    let frames = (ms / FRAME_LENGTH_MS) as i32;
    frames.max(1)
}

fn frames_to_ms(frames: i32) -> i32 {
    ((frames.max(1) as f32) * FRAME_LENGTH_MS) as i32
}

fn normalize_angle(mut angle: f32) -> f32 {
    while angle < -PI {
        angle += 2.0 * PI;
    }
    while angle > PI {
        angle -= 2.0 * PI;
    }
    angle
}

fn ease_ratios(milliseconds: i32, ease_in: f32, ease_out: f32) -> (f32, f32) {
    let total = milliseconds.max(1) as f32;
    (ease_in / total, ease_out / total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_creation() {
        let view = View::new();
        assert!(view.id() > 0);
        assert_eq!(view.width(), 0);
        assert_eq!(view.height(), 0);
    }

    #[test]
    fn test_view_initialization() {
        let mut view = View::new();
        view.init();

        assert_eq!(view.width(), DEFAULT_VIEW_WIDTH);
        assert_eq!(view.height(), DEFAULT_VIEW_HEIGHT);
        assert_eq!(
            view.origin(),
            (DEFAULT_VIEW_ORIGIN_X, DEFAULT_VIEW_ORIGIN_Y)
        );
        assert!(view.is_zoom_limited());
    }

    #[test]
    fn test_angle_and_pitch_limits() {
        let mut view = View::new();

        // Test pitch limiting
        view.set_pitch(PI); // Try to set extreme pitch
        let limit = PI / 5.0;
        assert!((view.pitch() - limit).abs() < 0.001);

        view.set_pitch(-PI); // Try negative extreme
        assert!((view.pitch() - (-limit)).abs() < 0.001);

        // Angle should not be limited
        view.set_angle(2.0 * PI);
        assert!((view.angle() - 2.0 * PI).abs() < 0.001);
    }

    #[test]
    fn test_zoom_limits() {
        let mut view = View::new();
        view.init();

        // Test zoom limiting when enabled
        view.set_zoom(10.0); // Try excessive zoom
        assert!(view.zoom() <= view.max_zoom());

        view.set_zoom(-1.0); // Try negative zoom
        assert!(view.zoom() >= view.min_zoom);

        // Test no limits when disabled
        view.set_zoom_limited(false);
        view.set_zoom(10.0);
        assert!((view.zoom() - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_position_and_look_at() {
        let mut view = View::new();
        view.init();
        view.set_position(&Point3::new(0.0, 0.0, 12.0));

        let target = Point3::new(100.0, 200.0, 0.0);
        view.look_at(&target);

        // C++ parity: lookAt stores top-left, preserving existing Z.
        assert!((view.position().x - (target.x - DEFAULT_VIEW_WIDTH as f32 * 0.5)).abs() < 0.001);
        assert!((view.position().y - (target.y - DEFAULT_VIEW_HEIGHT as f32 * 0.5)).abs() < 0.001);
        assert!((view.position().z - 12.0).abs() < 0.001);

        let delta = Vector2::new(50.0, -25.0);
        view.scroll_by(&delta);

        assert!(
            (view.position().x - (target.x - DEFAULT_VIEW_WIDTH as f32 * 0.5 + 50.0)).abs() < 0.001
        );
        assert!(
            (view.position().y - (target.y - DEFAULT_VIEW_HEIGHT as f32 * 0.5 - 25.0)).abs()
                < 0.001
        );
    }

    #[test]
    fn test_view_location_save_restore() {
        let mut view = View::new();
        view.init();

        // Set specific view state
        let look_target = Point3::new(100.0, 200.0, 10.0);
        view.set_position(&Point3::new(0.0, 0.0, 7.0));
        view.look_at(&look_target);
        let saved_position = *view.position();
        view.set_angle(PI / 4.0);
        view.set_pitch(PI / 6.0);
        view.set_zoom(0.5);

        // Save location
        let location = view.get_location();
        assert!(location.is_valid());

        // Change view
        view.look_at(&Point3::origin());
        view.set_angle(0.0);
        view.set_pitch(0.0);
        view.set_zoom(1.0);

        // Restore location
        view.set_location(&location);

        assert_eq!(*view.position(), saved_position);
        assert!((view.angle() - PI / 4.0).abs() < 0.001);
        assert!((view.pitch() - PI / 6.0).abs() < 0.001);
        assert!((view.zoom() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_3d_camera_position_calculation() {
        let mut view = View::new();
        view.init();

        // Set camera looking at origin with some height and angle
        view.look_at(&Point3::origin());
        view.set_height_above_ground(100.0);
        view.set_angle(PI / 4.0); // 45 degrees
        view.set_pitch(PI / 8.0); // Ensure camera has positive height component

        let camera_pos = view.get_3d_camera_position();

        // Camera should be offset from look-at point
        assert_ne!(camera_pos, *view.position());
        // Camera should be above ground
        assert!(camera_pos.z > 0.0);
    }

    #[test]
    fn test_projection_mode_switching() {
        let mut view = View::new();
        assert_eq!(view.projection_mode(), ProjectionMode::Perspective);

        view.set_projection_mode(ProjectionMode::Orthographic);
        assert_eq!(view.projection_mode(), ProjectionMode::Orthographic);
    }

    #[test]
    fn test_move_camera_to_animated_ends_at_look_at_position() {
        let mut view = View::new();
        view.init();

        let target = Point3::new(400.0, 300.0, 999.0);
        view.move_camera_to(&target, 1000, 0, false, 0.0, 0.0);

        for _ in 0..40 {
            view.update_view();
        }

        assert!(view.is_camera_movement_finished());
        assert!((view.position().x - (target.x - DEFAULT_VIEW_WIDTH as f32 * 0.5)).abs() < 0.001);
        assert!((view.position().y - (target.y - DEFAULT_VIEW_HEIGHT as f32 * 0.5)).abs() < 0.001);
    }

    #[test]
    fn test_move_camera_along_waypoint_path_reaches_last_waypoint() {
        let mut view = View::new();
        view.init();

        let path = vec![
            Point3::new(100.0, 120.0, 0.0),
            Point3::new(260.0, 260.0, 0.0),
            Point3::new(520.0, 360.0, 0.0),
        ];
        view.move_camera_along_waypoint_path(&path, 1000, 0, true, 0.0, 0.0);

        for _ in 0..40 {
            view.update_view();
        }

        assert!(view.is_camera_movement_finished());
        let expected_x = path.last().unwrap().x - DEFAULT_VIEW_WIDTH as f32 * 0.5;
        let expected_y = path.last().unwrap().y - DEFAULT_VIEW_HEIGHT as f32 * 0.5;
        assert!((view.position().x - expected_x).abs() < 0.001);
        assert!((view.position().y - expected_y).abs() < 0.001);
    }

    #[test]
    fn test_camera_mod_final_time_multiplier_speeds_waypoint_path() {
        let path = vec![
            Point3::new(100.0, 120.0, 0.0),
            Point3::new(260.0, 260.0, 0.0),
            Point3::new(520.0, 360.0, 0.0),
        ];
        let start_x = path[0].x - DEFAULT_VIEW_WIDTH as f32 * 0.5;
        let start_y = path[0].y - DEFAULT_VIEW_HEIGHT as f32 * 0.5;

        let mut normal = View::new();
        normal.init();
        normal.move_camera_along_waypoint_path(&path, 1000, 0, true, 0.0, 0.0);
        normal.update_view();
        let normal_dx = normal.position().x - start_x;
        let normal_dy = normal.position().y - start_y;
        let normal_distance = (normal_dx * normal_dx + normal_dy * normal_dy).sqrt();

        let mut accelerated = View::new();
        accelerated.init();
        accelerated.move_camera_along_waypoint_path(&path, 1000, 0, true, 0.0, 0.0);
        accelerated.camera_mod_final_time_multiplier(3);
        accelerated.update_view();
        let fast_dx = accelerated.position().x - start_x;
        let fast_dy = accelerated.position().y - start_y;
        let fast_distance = (fast_dx * fast_dx + fast_dy * fast_dy).sqrt();

        assert!(fast_distance > normal_distance);
    }

    #[test]
    fn test_camera_mod_freeze_time_clears_after_scripted_movement_finishes() {
        let mut view = View::new();
        view.init();
        view.camera_mod_freeze_time();
        assert!(view.is_time_frozen());

        let path = vec![
            Point3::new(100.0, 120.0, 0.0),
            Point3::new(260.0, 260.0, 0.0),
            Point3::new(520.0, 360.0, 0.0),
        ];
        view.move_camera_along_waypoint_path(&path, 1000, 0, true, 0.0, 0.0);
        assert!(view.freeze_time_for_camera_movement_active);

        for _ in 0..40 {
            view.update_view();
        }

        assert!(view.is_camera_movement_finished());
        assert!(!view.is_time_frozen());
    }

    #[test]
    fn test_camera_mod_freeze_angle_stops_rotation_progress() {
        let mut view = View::new();
        view.init();
        view.rotate_camera(0.5, 1000, 0.0, 0.0);
        view.update_view();

        let frozen_angle = view.angle();
        view.camera_mod_freeze_angle();

        for _ in 0..6 {
            view.update_view();
            assert!((view.angle() - frozen_angle).abs() < 0.001);
        }
    }

    #[test]
    fn test_wireframe_mode_applies_with_frame_delay() {
        let mut view = View::new();
        view.init();

        assert!(!view.is_3d_wireframe_mode());
        assert!(!view.pending_3d_wireframe_mode());

        view.set_3d_wireframe_mode(true);
        assert!(!view.is_3d_wireframe_mode());
        assert!(view.pending_3d_wireframe_mode());

        view.update_view();
        assert!(!view.is_3d_wireframe_mode());
        assert!(view.pending_3d_wireframe_mode());

        view.update_view();
        assert!(view.is_3d_wireframe_mode());
        assert!(view.pending_3d_wireframe_mode());

        view.set_3d_wireframe_mode(false);
        assert!(view.is_3d_wireframe_mode());
        assert!(!view.pending_3d_wireframe_mode());

        view.update_view();
        assert!(view.is_3d_wireframe_mode());
        assert!(!view.pending_3d_wireframe_mode());

        view.update_view();
        assert!(!view.is_3d_wireframe_mode());
        assert!(!view.pending_3d_wireframe_mode());
    }

    #[test]
    fn test_camera_commands_zero_duration_apply_immediately() {
        let mut view = View::new();
        view.init();

        view.set_position(&Point3::new(15.0, 25.0, 3.0));
        view.set_angle(0.25);
        view.set_pitch(0.2);
        view.set_zoom(0.8);

        let target = Point3::new(320.0, 240.0, 99.0);
        view.move_camera_to(&target, 0, 0, false, 0.0, 0.0);
        assert!(view.camera_move.is_none());
        assert!((view.position().x - (target.x - DEFAULT_VIEW_WIDTH as f32 * 0.5)).abs() < 0.001);
        assert!((view.position().y - (target.y - DEFAULT_VIEW_HEIGHT as f32 * 0.5)).abs() < 0.001);
        assert!((view.position().z - 3.0).abs() < 0.001);

        let old_angle = view.angle();
        view.rotate_camera(0.5, 0, 0.0, 0.0);
        assert!(view.camera_rotate.is_none());
        assert!((view.angle() - (old_angle + PI)).abs() < 0.001);

        view.zoom_camera(0.33, 0, 0.0, 0.0);
        assert!(view.camera_zoom.is_none());
        assert!((view.zoom() - 0.33).abs() < 0.001);

        view.pitch_camera(10.0, 0, 0.0, 0.0);
        assert!(view.camera_pitch.is_none());
        assert!(view.pitch() <= PI / 5.0 + 0.001);

        view.reset_camera(&Point3::new(0.0, 0.0, 0.0), 0, 0.0, 0.0);
        assert!(view.is_camera_movement_finished());
        assert!((view.angle() - view.default_angle).abs() < 0.001);
        assert!((view.pitch() - view.default_pitch_angle).abs() < 0.001);
        assert!((view.zoom() - view.max_zoom()).abs() < 0.001);
    }

    #[test]
    fn test_camera_transition_frame_thresholds_match_cpp_division() {
        // C++ parity: frame count uses integer division by frame length with minimum 1.
        assert_eq!(ms_to_frames(1), 1);
        assert_eq!(ms_to_frames(33), 1);
        assert_eq!(ms_to_frames(34), 1);
        assert_eq!(ms_to_frames(66), 1);
        assert_eq!(ms_to_frames(67), 2);
    }

    #[test]
    fn test_rotate_camera_34ms_finishes_in_one_update() {
        let mut view = View::new();
        view.init();
        view.set_angle(0.0);

        view.rotate_camera(0.5, 34, 0.0, 0.0);
        assert!(view.camera_rotate.is_some());

        view.update_view();

        assert!(view.camera_rotate.is_none());
        assert!((view.angle() - PI).abs() < 0.001);
    }

    #[test]
    fn test_set_default_view_does_not_mutate_current_camera_immediately() {
        let mut view = View::new();
        view.init();
        view.set_angle(0.35);
        view.set_pitch(0.2);

        let current_angle = view.angle();
        let current_pitch = view.pitch();
        let global_max = get_global_data()
            .map(|global| global.read().max_camera_height)
            .unwrap_or(view.max_height_above_ground);
        view.set_default_view(0.6, 1.5, 0.75);

        assert!((view.angle() - current_angle).abs() < 0.001);
        assert!((view.pitch() - current_pitch).abs() < 0.001);
        assert!(
            (view.max_height_above_ground - (global_max * 0.75).max(view.min_height_above_ground))
                .abs()
                < 0.001
        );
    }

    #[test]
    fn test_shake_overflow_clamps_to_cpp_constant() {
        let mut view = View::new();
        view.init();
        view.set_position(&Point3::origin());
        let epicenter = Point3::origin();

        // Accumulate enough shake to overflow max_shake_intensity.
        for _ in 0..8 {
            view.shake(&epicenter, CameraShakeType::Subtle);
        }

        assert!((view.shake_intensity - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_camera_mod_final_zoom_uses_remaining_movement_time() {
        let mut view = View::new();
        view.init();

        view.move_camera_to(&Point3::new(300.0, 200.0, 0.0), 1000, 0, false, 0.0, 0.0);
        assert!(view.camera_move.is_some());
        assert!(view.camera_zoom.is_none());

        // Advance once so camera-mod computes a non-full remaining duration.
        view.update_view();
        view.camera_mod_final_zoom(0.45, 0.0, 0.0);
        assert!(view.camera_zoom.is_some());

        for _ in 0..40 {
            view.update_view();
        }
        assert!((view.zoom() - 0.45).abs() < 0.001);
    }

    #[test]
    fn test_camera_mod_final_pitch_uses_remaining_movement_time() {
        let mut view = View::new();
        view.init();

        view.move_camera_to(&Point3::new(300.0, 200.0, 0.0), 1000, 0, false, 0.0, 0.0);
        assert!(view.camera_move.is_some());
        assert!(view.camera_pitch.is_none());

        view.update_view();
        view.camera_mod_final_pitch(0.4, 0.0, 0.0);
        assert!(view.camera_pitch.is_some());

        for _ in 0..40 {
            view.update_view();
        }
        assert!((view.pitch() - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_camera_mod_look_toward_only_applies_while_path_active() {
        let mut idle = View::new();
        idle.init();
        idle.set_angle(0.33);
        idle.camera_mod_look_toward(&Point3::new(500.0, 100.0, 0.0));
        assert!((idle.angle() - 0.33).abs() < 0.001);

        let path = vec![
            Point3::new(100.0, 120.0, 0.0),
            Point3::new(260.0, 260.0, 0.0),
            Point3::new(520.0, 360.0, 0.0),
        ];

        let mut baseline = View::new();
        baseline.init();
        baseline.move_camera_along_waypoint_path(&path, 1000, 0, true, 0.0, 0.0);
        baseline.update_view();
        let baseline_angle = baseline.angle();

        let mut modified = View::new();
        modified.init();
        modified.move_camera_along_waypoint_path(&path, 1000, 0, true, 0.0, 0.0);
        modified.camera_mod_look_toward(&Point3::new(900.0, 100.0, 0.0));
        modified.update_view();
        let modified_angle = modified.angle();

        assert!((baseline_angle - modified_angle).abs() > 0.001);
    }

    #[test]
    fn test_camera_mod_final_move_to_retargets_path_endpoint() {
        let path = vec![
            Point3::new(100.0, 120.0, 0.0),
            Point3::new(260.0, 260.0, 0.0),
            Point3::new(520.0, 360.0, 0.0),
        ];

        let mut baseline = View::new();
        baseline.init();
        baseline.move_camera_along_waypoint_path(&path, 1000, 0, true, 0.0, 0.0);
        for _ in 0..40 {
            baseline.update_view();
        }

        let mut modified = View::new();
        modified.init();
        modified.move_camera_along_waypoint_path(&path, 1000, 0, true, 0.0, 0.0);
        let retarget = Point3::new(780.0, 510.0, 0.0);
        modified.camera_mod_final_move_to(&retarget);
        for _ in 0..40 {
            modified.update_view();
        }

        let expected_x = retarget.x - DEFAULT_VIEW_WIDTH as f32 * 0.5;
        let expected_y = retarget.y - DEFAULT_VIEW_HEIGHT as f32 * 0.5;
        assert!((modified.position().x - expected_x).abs() < 0.001);
        assert!((modified.position().y - expected_y).abs() < 0.001);
        assert!(
            (baseline.position().x - modified.position().x).abs() > 0.001
                || (baseline.position().y - modified.position().y).abs() > 0.001
        );
    }

    #[test]
    fn test_camera_mod_look_toward_applies_to_active_move_transition() {
        let mut view = View::new();
        view.init();
        view.set_angle(0.0);

        view.move_camera_to(&Point3::new(400.0, 320.0, 0.0), 1000, 0, false, 0.0, 0.0);
        view.camera_mod_look_toward(&Point3::new(900.0, 120.0, 0.0));
        assert!(view.camera_rotate.is_some());

        let start_angle = view.angle();
        view.update_view();
        assert!((view.angle() - start_angle).abs() > 0.0001);
    }

    #[test]
    fn test_reset_camera_animated_targets_w3d_pitch_endpoint() {
        let mut view = View::new();
        view.init();
        view.set_pitch(0.0);

        view.reset_camera(&Point3::new(300.0, 300.0, 0.0), 1000, 0.0, 0.0);
        assert!(view.camera_pitch.is_some());

        for _ in 0..40 {
            view.update_view();
        }

        // W3D resetCamera uses pitchCamera(1.0f, ...). Rust pitch is clamped at PI/5.
        assert!((view.pitch() - (PI / 5.0)).abs() < 0.001);
    }

    #[test]
    fn test_camera_locking() {
        let mut view = View::new();

        assert_eq!(view.camera_lock_id(), None);

        view.set_camera_lock(Some(42));
        assert_eq!(view.camera_lock_id(), Some(42));

        view.set_snap_mode(CameraLockType::Tether, 100.0);
        // Additional testing would require object system integration
    }

    #[test]
    fn test_field_of_view_limits() {
        let mut view = View::new();

        // Test FOV limiting
        view.set_field_of_view(0.0); // Too small
        assert!(view.field_of_view() > 0.0);

        view.set_field_of_view(PI); // Too large
        assert!(view.field_of_view() < PI);

        // Test normal FOV
        view.set_field_of_view(PI / 3.0); // 60 degrees
        assert!((view.field_of_view() - PI / 3.0).abs() < 0.001);
    }

    #[test]
    fn test_basic_vector_math() {
        let v1 = Vector3::new(1.0, 0.0, 0.0);
        let v2 = Vector3::new(0.0, 1.0, 0.0);

        let cross = v1.cross(&v2);
        assert_eq!(cross, Vector3::new(0.0, 0.0, 1.0));

        let dot = v1.dot(&v2);
        assert_eq!(dot, 0.0); // Perpendicular vectors

        assert!((v1.magnitude() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_point_vector_operations() {
        let p1 = Point3::new(1.0, 2.0, 3.0);
        let p2 = Point3::new(4.0, 5.0, 6.0);
        let v = Vector3::new(1.0, 1.0, 1.0);

        let diff = p2 - p1;
        assert_eq!(diff, Vector3::new(3.0, 3.0, 3.0));

        let moved = p1 + v;
        assert_eq!(moved, Point3::new(2.0, 3.0, 4.0));
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
    fn draw_view(&self) -> Result<(), ViewError>;

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

    fn draw_view(&self) -> Result<(), ViewError> {
        // This would integrate with the rendering system
        // For now, just signal that drawing was requested
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
