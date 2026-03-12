// FILE: view.rs
// Author: Ported from C++ View.h/View.cpp
// Desc: A "view", or window, into the World - Camera system
// Original Author: Michael S. Booth, February 2001

use super::types::*;
use std::sync::atomic::{AtomicU32, Ordering};

// Constants matching C++ View.h
pub const DEFAULT_VIEW_WIDTH: i32 = 640;
pub const DEFAULT_VIEW_HEIGHT: i32 = 480;
pub const DEFAULT_VIEW_ORIGIN_X: i32 = 0;
pub const DEFAULT_VIEW_ORIGIN_Y: i32 = 0;

/// Camera shake intensity levels
/// Matches C++ View.h CameraShakeType enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraShakeType {
    Subtle = 0,
    Normal,
    Strong,
    Severe,
    CineExtreme,    // Added for cinematics ONLY
    CineInsane,     // Added for cinematics ONLY
}

/// World to screen transform return status
/// Matches C++ View.h WorldToScreenReturn enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldToScreenReturn {
    InsideFrustum = 0,  // On the screen (inside frustum of camera)
    OutsideFrustum,     // Return is valid but off the screen (outside frustum of camera)
    Invalid,            // No transform possible
}

/// Camera lock type
/// Matches C++ View.h CameraLockType enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraLockType {
    Follow,     // Camera follows object
    Tether,     // Camera is tethered to object with max distance
}

/// Used to save and restore view position
/// Matches C++ ViewLocation class from View.h
#[derive(Debug, Clone, Copy)]
pub struct ViewLocation {
    valid: bool,
    pos: Coord3D,
    angle: f32,
    pitch: f32,
    zoom: f32,
}

impl ViewLocation {
    pub fn new() -> Self {
        Self {
            valid: false,
            pos: Coord3D::zero(),
            angle: 0.0,
            pitch: 0.0,
            zoom: 0.0,
        }
    }

    pub fn init(&mut self, x: f32, y: f32, z: f32, angle: f32, pitch: f32, zoom: f32) {
        self.pos.x = x;
        self.pos.y = y;
        self.pos.z = z;
        self.angle = angle;
        self.pitch = pitch;
        self.zoom = zoom;
        self.valid = true;
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn get_position(&self) -> &Coord3D {
        &self.pos
    }

    pub fn get_angle(&self) -> f32 {
        self.angle
    }

    pub fn get_pitch(&self) -> f32 {
        self.pitch
    }

    pub fn get_zoom(&self) -> f32 {
        self.zoom
    }
}

impl Default for ViewLocation {
    fn default() -> Self {
        Self::new()
    }
}

/// The implementation of common view functionality
/// Matches C++ View class from View.h/View.cpp
pub struct View {
    // List links used by the Display class
    next: Option<Box<View>>,

    // The ID of this view
    id: u32,

    // Position of this view, in world coordinates
    pos: Coord3D,

    // Dimensions of the view
    width: i32,
    height: i32,

    // Location of top/left view corner
    origin_x: i32,
    origin_y: i32,

    // Angle at which view has been rotated about the Z axis
    angle: f32,

    // Rotation of view direction around horizontal (X) axis
    pitch_angle: f32,

    // Zoom values
    max_zoom: f32,
    min_zoom: f32,
    max_height_above_ground: f32,
    min_height_above_ground: f32,
    zoom: f32,

    // User's desired height above ground
    height_above_ground: f32,

    // Camera restricted in zoom height
    zoom_limited: bool,

    // Default angles
    default_angle: f32,
    default_pitch_angle: f32,

    // Cached values for debugging
    current_height_above_ground: f32,
    terrain_height_under_camera: f32,

    // Camera lock to object
    camera_lock: ObjectID,
    camera_lock_drawable: DrawableID,
    lock_type: CameraLockType,
    lock_dist: f32,

    // Field of view angle
    fov: f32,

    // Is the mouse input locked to the tactical view?
    mouse_locked: bool,

    // Should we attempt to adjust camera height?
    ok_to_adjust_height: bool,

    // Should we immediately snap to the object we're following?
    snap_immediate: bool,

    // Extra beefy margins so huge things can stay "on-screen"
    guard_band_bias: Coord2D,
}

/// Static counter for allocating view IDs
/// Matches C++ View::m_idNext from View.cpp
static VIEW_ID_COUNTER: AtomicU32 = AtomicU32::new(1);

impl View {
    /// Create a new View
    /// Matches C++ View::View() from View.cpp
    pub fn new() -> Self {
        let id = VIEW_ID_COUNTER.fetch_add(1, Ordering::SeqCst);

        Self {
            next: None,
            id,
            pos: Coord3D::new(0.0, 0.0, 0.0),
            width: 0,
            height: 0,
            origin_x: 0,
            origin_y: 0,
            angle: 0.0,
            pitch_angle: 0.0,
            max_zoom: 0.0,
            min_zoom: 0.0,
            max_height_above_ground: 0.0,
            min_height_above_ground: 0.0,
            zoom: 0.0,
            height_above_ground: 0.0,
            zoom_limited: true,
            default_angle: 0.0,
            default_pitch_angle: 0.0,
            current_height_above_ground: 0.0,
            terrain_height_under_camera: 0.0,
            camera_lock: ObjectID::INVALID,
            camera_lock_drawable: DrawableID::INVALID,
            lock_type: CameraLockType::Follow,
            lock_dist: 0.0,
            fov: 50.0 * PI_F32 / 180.0,  // Default field of view
            mouse_locked: false,
            ok_to_adjust_height: true,
            snap_immediate: false,
            guard_band_bias: Coord2D::zero(),
        }
    }

    /// Initialize the view
    /// Matches C++ View::init() from View.cpp
    pub fn init(&mut self, max_camera_height: f32, min_camera_height: f32) {
        self.width = DEFAULT_VIEW_WIDTH;
        self.height = DEFAULT_VIEW_HEIGHT;
        self.origin_x = DEFAULT_VIEW_ORIGIN_X;
        self.origin_y = DEFAULT_VIEW_ORIGIN_Y;
        self.pos.x = 0.0;
        self.pos.y = 0.0;
        self.angle = 0.0;
        self.camera_lock = ObjectID::INVALID;
        self.camera_lock_drawable = DrawableID::INVALID;
        self.zoom_limited = true;

        self.max_zoom = 1.3;
        self.min_zoom = 0.2;
        self.zoom = self.max_zoom;
        self.max_height_above_ground = max_camera_height;
        self.min_height_above_ground = min_camera_height;
        self.ok_to_adjust_height = false;

        self.default_angle = 0.0;
        self.default_pitch_angle = 0.0;
    }

    /// Reset the view
    /// Matches C++ View::reset() from View.cpp
    pub fn reset(&mut self) {
        // Only fixing the reported bug. Who knows what side effects resetting the rest could have.
        self.zoom_limited = true;
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    /// Limit the zoom height
    pub fn set_zoom_limited(&mut self, limit: bool) {
        self.zoom_limited = limit;
    }

    /// Get status of zoom limit
    pub fn is_zoom_limited(&self) -> bool {
        self.zoom_limited
    }

    pub fn set_width(&mut self, width: i32) {
        self.width = width;
    }

    pub fn get_width(&self) -> i32 {
        self.width
    }

    pub fn set_height(&mut self, height: i32) {
        self.height = height;
    }

    pub fn get_height(&self) -> i32 {
        self.height
    }

    /// Sets location of top-left view corner on display
    pub fn set_origin(&mut self, x: i32, y: i32) {
        self.origin_x = x;
        self.origin_y = y;
    }

    /// Return location of top-left view corner on display
    pub fn get_origin(&self) -> (i32, i32) {
        (self.origin_x, self.origin_y)
    }

    /// Center the view on the given coordinate
    /// Matches C++ View::lookAt() from View.cpp
    pub fn look_at(&mut self, o: &Coord3D) {
        // This needs to be changed to be 3D, this is still old 2D stuff
        self.pos.x = o.x - self.width as f32 * 0.5;
        self.pos.y = o.y - self.height as f32 * 0.5;
    }

    /// Shift the view by the given delta
    /// Matches C++ View::scrollBy() from View.cpp
    pub fn scroll_by(&mut self, delta: &Coord2D) {
        // update view's world position
        self.pos.x += delta.x;
        self.pos.y += delta.y;
    }

    /// Rotate the view around the up axis to the given angle
    /// Matches C++ View::setAngle() from View.cpp
    pub fn set_angle(&mut self, angle: f32) {
        self.angle = angle;
    }

    pub fn get_angle(&self) -> f32 {
        self.angle
    }

    /// Rotate the view around the horizontal (X) axis to the given angle
    /// Matches C++ View::setPitch() from View.cpp
    pub fn set_pitch(&mut self, angle: f32) {
        self.pitch_angle = angle;

        // Limit pitch to +/- PI/5 (36 degrees)
        let limit = PI_F32 / 5.0;

        if self.pitch_angle < -limit {
            self.pitch_angle = -limit;
        } else if self.pitch_angle > limit {
            self.pitch_angle = limit;
        }
    }

    /// Return current camera pitch
    pub fn get_pitch(&self) -> f32 {
        self.pitch_angle
    }

    /// Set the view angle back to default
    /// Matches C++ View::setAngleAndPitchToDefault() from View.cpp
    pub fn set_angle_and_pitch_to_default(&mut self) {
        self.angle = self.default_angle;
        self.pitch_angle = self.default_pitch_angle;
    }

    /// Returns position camera is looking at (z will be zero)
    pub fn get_position(&self) -> Coord3D {
        self.pos
    }

    /// Set position (internal use, but public for testing)
    pub fn set_position(&mut self, pos: &Coord3D) {
        self.pos = *pos;
    }

    pub fn get_zoom(&self) -> f32 {
        self.zoom
    }

    pub fn set_zoom(&mut self, z: f32) {
        self.zoom = z;
    }

    pub fn get_height_above_ground(&self) -> f32 {
        self.height_above_ground
    }

    pub fn set_height_above_ground(&mut self, z: f32) {
        self.height_above_ground = z;
    }

    /// Zoom in, closer to the ground, limit to min
    /// Matches C++ View::zoomIn() from View.cpp
    pub fn zoom_in(&mut self) {
        self.set_height_above_ground(self.get_height_above_ground() - 10.0);
    }

    /// Zoom out, farther away from the ground, limit to max
    /// Matches C++ View::zoomOut() from View.cpp
    pub fn zoom_out(&mut self) {
        self.set_height_above_ground(self.get_height_above_ground() + 10.0);
    }

    /// Set zoom to default value
    pub fn set_zoom_to_default(&mut self) {
        self.zoom = self.max_zoom;
    }

    /// Return max zoom value
    pub fn get_max_zoom(&self) -> f32 {
        self.max_zoom
    }

    /// Set this to adjust camera height
    pub fn set_ok_to_adjust_height(&mut self, val: bool) {
        self.ok_to_adjust_height = val;
    }

    // For debugging
    pub fn get_terrain_height_under_camera(&self) -> f32 {
        self.terrain_height_under_camera
    }

    pub fn set_terrain_height_under_camera(&mut self, z: f32) {
        self.terrain_height_under_camera = z;
    }

    pub fn get_current_height_above_ground(&self) -> f32 {
        self.current_height_above_ground
    }

    pub fn set_current_height_above_ground(&mut self, z: f32) {
        self.current_height_above_ground = z;
    }

    /// Set the horizontal field of view angle
    pub fn set_field_of_view(&mut self, angle: f32) {
        self.fov = angle;
    }

    /// Get the horizontal field of view angle
    pub fn get_field_of_view(&self) -> f32 {
        self.fov
    }

    /// Write the view's current location into the view location object
    /// Matches C++ View::getLocation() from View.cpp
    pub fn get_location(&self, location: &mut ViewLocation) {
        let pos = self.get_position();
        location.init(pos.x, pos.y, pos.z, self.get_angle(), self.get_pitch(), self.get_zoom());
    }

    /// Set the view's current location from the view location object
    /// Matches C++ View::setLocation() from View.cpp
    pub fn set_location(&mut self, location: &ViewLocation) {
        if location.is_valid() {
            self.set_position(location.get_position());
            self.set_angle(location.get_angle());
            self.set_pitch(location.get_pitch());
            self.set_zoom(location.get_zoom());
            // Note: would call forceRedraw() here in full implementation
        }
    }

    /// Get camera lock object ID
    pub fn get_camera_lock(&self) -> ObjectID {
        self.camera_lock
    }

    /// Set camera lock to follow an object
    pub fn set_camera_lock(&mut self, id: ObjectID) {
        self.camera_lock = id;
        self.lock_dist = 0.0;
        self.lock_type = CameraLockType::Follow;
    }

    /// Should we immediately snap to the object we're following?
    pub fn snap_to_camera_lock(&mut self) {
        self.snap_immediate = true;
    }

    /// Set snap mode for camera lock
    pub fn set_snap_mode(&mut self, lock_type: CameraLockType, lock_dist: f32) {
        self.lock_type = lock_type;
        self.lock_dist = lock_dist;
    }

    /// Get camera lock drawable ID
    pub fn get_camera_lock_drawable(&self) -> DrawableID {
        self.camera_lock_drawable
    }

    /// Set camera lock to follow a drawable
    pub fn set_camera_lock_drawable(&mut self, drawable: DrawableID) {
        self.camera_lock_drawable = drawable;
        self.lock_dist = 0.0;
    }

    /// Lock/unlock the mouse input to the tactical view
    pub fn set_mouse_lock(&mut self, mouse_locked: bool) {
        self.mouse_locked = mouse_locked;
    }

    /// Is the mouse input locked to the tactical view?
    pub fn is_mouse_locked(&self) -> bool {
        self.mouse_locked
    }

    pub fn set_guard_band_bias(&mut self, gb: &Coord2D) {
        self.guard_band_bias = *gb;
    }

    pub fn get_default_angle(&self) -> f32 {
        self.default_angle
    }

    pub fn set_default_angle(&mut self, angle: f32) {
        self.default_angle = angle;
    }

    pub fn get_default_pitch_angle(&self) -> f32 {
        self.default_pitch_angle
    }

    pub fn set_default_pitch_angle(&mut self, angle: f32) {
        self.default_pitch_angle = angle;
    }
}

impl Default for View {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_creation() {
        let view = View::new();
        assert_eq!(view.get_width(), 0);
        assert_eq!(view.get_height(), 0);
        assert_eq!(view.get_angle(), 0.0);
        assert_eq!(view.get_pitch(), 0.0);
    }

    #[test]
    fn test_view_init() {
        let mut view = View::new();
        view.init(1000.0, 100.0);
        assert_eq!(view.get_width(), DEFAULT_VIEW_WIDTH);
        assert_eq!(view.get_height(), DEFAULT_VIEW_HEIGHT);
        assert_eq!(view.get_max_zoom(), 1.3);
        assert!(view.is_zoom_limited());
    }

    #[test]
    fn test_view_look_at() {
        let mut view = View::new();
        view.set_width(800);
        view.set_height(600);
        let target = Coord3D::new(1000.0, 500.0, 0.0);
        view.look_at(&target);
        let pos = view.get_position();
        assert_eq!(pos.x, 1000.0 - 400.0);
        assert_eq!(pos.y, 500.0 - 300.0);
    }

    #[test]
    fn test_view_scroll() {
        let mut view = View::new();
        let delta = Coord2D::new(10.0, 20.0);
        view.scroll_by(&delta);
        let pos = view.get_position();
        assert_eq!(pos.x, 10.0);
        assert_eq!(pos.y, 20.0);
    }

    #[test]
    fn test_view_pitch_limits() {
        let mut view = View::new();
        let limit = PI_F32 / 5.0;

        // Test upper limit
        view.set_pitch(PI_F32);
        assert_eq!(view.get_pitch(), limit);

        // Test lower limit
        view.set_pitch(-PI_F32);
        assert_eq!(view.get_pitch(), -limit);

        // Test normal value
        view.set_pitch(0.5);
        assert_eq!(view.get_pitch(), 0.5);
    }

    #[test]
    fn test_view_zoom() {
        let mut view = View::new();
        view.set_height_above_ground(500.0);

        view.zoom_in();
        assert_eq!(view.get_height_above_ground(), 490.0);

        view.zoom_out();
        assert_eq!(view.get_height_above_ground(), 500.0);
    }

    #[test]
    fn test_view_location() {
        let mut view = View::new();
        view.set_position(&Coord3D::new(100.0, 200.0, 0.0));
        view.set_angle(1.5);
        view.set_pitch(0.3);
        view.set_zoom(0.8);

        let mut location = ViewLocation::new();
        view.get_location(&mut location);

        assert!(location.is_valid());
        assert_eq!(location.get_position().x, 100.0);
        assert_eq!(location.get_position().y, 200.0);
        assert_eq!(location.get_angle(), 1.5);
        assert_eq!(location.get_pitch(), 0.3);
        assert_eq!(location.get_zoom(), 0.8);
    }

    #[test]
    fn test_view_camera_lock() {
        let mut view = View::new();
        let obj_id = ObjectID(42);

        view.set_camera_lock(obj_id);
        assert_eq!(view.get_camera_lock(), obj_id);
        assert_eq!(view.lock_dist, 0.0);
        assert_eq!(view.lock_type, CameraLockType::Follow);
    }

    #[test]
    fn test_view_angle_and_pitch_default() {
        let mut view = View::new();
        view.set_default_angle(1.0);
        view.set_default_pitch_angle(0.5);

        view.set_angle(2.0);
        view.set_pitch(1.0);

        view.set_angle_and_pitch_to_default();
        assert_eq!(view.get_angle(), 1.0);
        assert_eq!(view.get_pitch(), 0.5);
    }
}
