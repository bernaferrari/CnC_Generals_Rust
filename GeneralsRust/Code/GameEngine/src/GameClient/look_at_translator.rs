// FILE: look_at_translator.rs
// Author: Ported from C++ LookAtXlat.h/LookAtXlat.cpp
// Desc: Translate raw input events into camera movement commands
// Original Author: Michael S. Booth, April 2001

use super::types::*;
use super::view::{View, ViewLocation};

/// Maximum number of camera bookmarks
/// Matches C++ LookAtTranslator::MAX_VIEW_LOCS from LookAtXlat.h
const MAX_VIEW_LOCS: usize = 8;

/// Scroll amount constant
/// Matches C++ SCROLL_AMT from LookAtXlat.cpp
const SCROLL_AMT: i32 = 100;

/// Edge scroll size in pixels
/// Matches C++ edgeScrollSize from LookAtXlat.cpp
const EDGE_SCROLL_SIZE: i32 = 3;

/// Scroll type enumeration
/// Matches C++ anonymous enum from LookAtXlat.cpp
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrollType {
    None = 0,
    Rmb,
    Key,
    ScreenEdge,
}

/// Direction enumeration for keyboard scrolling
/// Matches C++ DIR_UP/DIR_DOWN/etc from LookAtXlat.cpp
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up = 0,
    Down,
    Left,
    Right,
}

/// Mouse cursor types (simplified for this port)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseCursor {
    Arrow,
    Scroll,
    Rotate,
}

/// Key state flags
/// Matches C++ KeyDefs.h KEY_STATE_* constants
const KEY_STATE_UP: u8 = 0x80;

/// Virtual key codes
/// Matches C++ KeyDefs.h KEY_* constants
const KEY_UP: u8 = 0x26;
const KEY_DOWN: u8 = 0x28;
const KEY_LEFT: u8 = 0x25;
const KEY_RIGHT: u8 = 0x27;

/// LookAtTranslator is responsible for camera movements
/// It handles RMB scrolling, keyboard scrolling, edge scrolling,
/// MMB rotation, pitch adjustment, and CTRL-<F key> bookmarking
/// Matches C++ LookAtTranslator class from LookAtXlat.h/LookAtXlat.cpp
pub struct LookAtTranslator {
    // Current mouse anchor point
    anchor: ICoord2D,

    // Original anchor point (for MMB rotation)
    original_anchor: ICoord2D,

    // Current mouse position
    current_pos: ICoord2D,

    // Set to true if we are in the act of RMB scrolling
    is_scrolling: bool,

    // Set to true if we are in the act of MMB rotating
    is_rotating: bool,

    // Set to true if we are in the act of ALT pitch rotation
    is_pitching: bool,

    // Set to true if we are in the act of changing the field of view
    is_changing_fov: bool,

    // Set when button goes down
    timestamp: u32,

    // Last plane ID for camera lock cycling (debug feature)
    last_plane_id: DrawableID,

    // View location bookmarks (8 slots)
    view_location: [ViewLocation; MAX_VIEW_LOCS],

    // Current scroll type
    scroll_type: ScrollType,

    // Keyboard scroll directions
    scroll_dir: [bool; 4],

    // Previous cursor for restoration
    prev_cursor: MouseCursor,

    // Last frame where mouse moved
    last_mouse_move_frame: u32,

    // Global data references (would come from TheGlobalData in C++)
    horizontal_scroll_speed_factor: f32,
    vertical_scroll_speed_factor: f32,
    keyboard_scroll_factor: f32,
    windowed: bool,
    save_camera_in_replay: bool,
}

impl LookAtTranslator {
    /// Create a new LookAtTranslator
    /// Matches C++ LookAtTranslator::LookAtTranslator() from LookAtXlat.cpp
    pub fn new() -> Self {
        Self {
            anchor: ICoord2D::zero(),
            original_anchor: ICoord2D::zero(),
            current_pos: ICoord2D::zero(),
            is_scrolling: false,
            is_rotating: false,
            is_pitching: false,
            is_changing_fov: false,
            timestamp: 0,
            last_plane_id: DrawableID::INVALID,
            view_location: [ViewLocation::new(); MAX_VIEW_LOCS],
            scroll_type: ScrollType::None,
            scroll_dir: [false; 4],
            prev_cursor: MouseCursor::Arrow,
            last_mouse_move_frame: 0,
            // Default global data values
            horizontal_scroll_speed_factor: 1.0,
            vertical_scroll_speed_factor: 1.0,
            keyboard_scroll_factor: 1.0,
            windowed: false,
            save_camera_in_replay: false,
        }
    }

    /// Set global data values
    pub fn set_global_data(
        &mut self,
        horizontal_scroll_speed_factor: f32,
        vertical_scroll_speed_factor: f32,
        keyboard_scroll_factor: f32,
        windowed: bool,
        save_camera_in_replay: bool,
    ) {
        self.horizontal_scroll_speed_factor = horizontal_scroll_speed_factor;
        self.vertical_scroll_speed_factor = vertical_scroll_speed_factor;
        self.keyboard_scroll_factor = keyboard_scroll_factor;
        self.windowed = windowed;
        self.save_camera_in_replay = save_camera_in_replay;
    }

    /// Start scrolling
    /// Matches C++ LookAtTranslator::setScrolling() from LookAtXlat.cpp
    fn set_scrolling(&mut self, scroll_type: ScrollType, current_cursor: MouseCursor) {
        self.prev_cursor = current_cursor;
        self.is_scrolling = true;
        self.scroll_type = scroll_type;
    }

    /// Stop scrolling
    /// Matches C++ LookAtTranslator::stopScrolling() from LookAtXlat.cpp
    fn stop_scrolling(&mut self) -> MouseCursor {
        self.is_scrolling = false;
        self.scroll_type = ScrollType::None;
        self.prev_cursor
    }

    /// Get RMB scroll anchor if we're RMB scrolling
    /// Matches C++ LookAtTranslator::getRMBScrollAnchor() from LookAtXlat.cpp
    pub fn get_rmb_scroll_anchor(&self) -> Option<ICoord2D> {
        if self.is_scrolling && self.scroll_type == ScrollType::Rmb {
            Some(self.anchor)
        } else {
            None
        }
    }

    /// Check if mouse has moved recently
    /// Matches C++ LookAtTranslator::hasMouseMovedRecently() from LookAtXlat.cpp
    pub fn has_mouse_moved_recently(&self, current_frame: u32) -> bool {
        const LOGICFRAMES_PER_SECOND: u32 = 30;

        if self.last_mouse_move_frame + LOGICFRAMES_PER_SECOND < current_frame {
            return false;
        }
        true
    }

    /// Set current mouse position
    /// Matches C++ LookAtTranslator::setCurrentPos() from LookAtXlat.cpp
    pub fn set_current_pos(&mut self, pos: ICoord2D) {
        self.current_pos = pos;
    }

    /// Reset modes when disabling input
    /// Matches C++ LookAtTranslator::resetModes() from LookAtXlat.cpp
    pub fn reset_modes(&mut self) {
        self.is_scrolling = false;
        self.is_rotating = false;
        self.is_pitching = false;
        self.is_changing_fov = false;
    }

    /// Handle raw key down/up event
    /// Matches key handling from C++ LookAtTranslator::translateGameMessage() from LookAtXlat.cpp
    pub fn handle_key_event(
        &mut self,
        key: u8,
        state: u8,
        is_selecting: bool,
    ) {
        let is_pressed = (state & KEY_STATE_UP) == 0;

        // Update scroll direction state
        match key {
            KEY_UP => self.scroll_dir[Direction::Up as usize] = is_pressed,
            KEY_DOWN => self.scroll_dir[Direction::Down as usize] = is_pressed,
            KEY_LEFT => self.scroll_dir[Direction::Left as usize] = is_pressed,
            KEY_RIGHT => self.scroll_dir[Direction::Right as usize] = is_pressed,
            _ => return,
        }

        // Don't start/stop scrolling if selecting or scrolling with different method
        if is_selecting || (self.is_scrolling && self.scroll_type != ScrollType::Key) {
            return;
        }

        // Count active directions
        let num_dirs = self.scroll_dir.iter().filter(|&&dir| dir).count();

        // Start or stop keyboard scrolling
        if num_dirs > 0 && !self.is_scrolling {
            self.set_scrolling(ScrollType::Key, MouseCursor::Arrow);
        } else if num_dirs == 0 && self.is_scrolling {
            self.stop_scrolling();
        }
    }

    /// Handle mouse right button down
    /// Matches C++ MSG_RAW_MOUSE_RIGHT_BUTTON_DOWN from LookAtXlat.cpp
    pub fn handle_rmb_down(
        &mut self,
        pixel: ICoord2D,
        is_selecting: bool,
        current_frame: u32,
    ) {
        self.last_mouse_move_frame = current_frame;
        self.anchor = pixel;
        self.current_pos = pixel;

        if !is_selecting && !self.is_scrolling {
            self.set_scrolling(ScrollType::Rmb, MouseCursor::Scroll);
        }
    }

    /// Handle mouse right button up
    /// Matches C++ MSG_RAW_MOUSE_RIGHT_BUTTON_UP from LookAtXlat.cpp
    pub fn handle_rmb_up(&mut self, current_frame: u32) -> Option<MouseCursor> {
        self.last_mouse_move_frame = current_frame;

        if self.scroll_type == ScrollType::Rmb {
            Some(self.stop_scrolling())
        } else {
            None
        }
    }

    /// Handle mouse middle button down
    /// Matches C++ MSG_RAW_MOUSE_MIDDLE_BUTTON_DOWN from LookAtXlat.cpp
    pub fn handle_mmb_down(&mut self, pixel: ICoord2D, current_frame: u32, client_frame: u32) {
        self.last_mouse_move_frame = current_frame;
        self.is_rotating = true;
        self.anchor = pixel;
        self.original_anchor = pixel;
        self.current_pos = pixel;
        self.timestamp = client_frame;
    }

    /// Handle mouse middle button up
    /// Returns true if camera should reset to default
    /// Matches C++ MSG_RAW_MOUSE_MIDDLE_BUTTON_UP from LookAtXlat.cpp
    pub fn handle_mmb_up(&mut self, current_frame: u32, client_frame: u32) -> bool {
        self.last_mouse_move_frame = current_frame;

        const CLICK_DURATION: u32 = 5;
        const PIXEL_OFFSET: i32 = 5;

        self.is_rotating = false;

        let dx = (self.current_pos.x - self.original_anchor.x).abs();
        let dy = (self.current_pos.y - self.original_anchor.y).abs();
        let did_move = dx > PIXEL_OFFSET || dy > PIXEL_OFFSET;

        // If middle button is "clicked" (not dragged), reset to "home" orientation
        !did_move && client_frame - self.timestamp < CLICK_DURATION
    }

    /// Handle mouse move and return rotation/pitch deltas
    /// Matches C++ MSG_RAW_MOUSE_POSITION from LookAtXlat.cpp
    pub fn handle_mouse_move(
        &mut self,
        pixel: ICoord2D,
        current_frame: u32,
        display_width: u32,
        display_height: u32,
        input_enabled: bool,
    ) -> MouseMoveResult {
        // Track mouse movement
        if self.current_pos.x != pixel.x || self.current_pos.y != pixel.y {
            self.last_mouse_move_frame = current_frame;
        }

        self.current_pos = pixel;

        let mut result = MouseMoveResult::default();

        // If input disabled, stop all scrolling
        if !input_enabled {
            if self.is_scrolling {
                result.stop_scrolling = true;
            }
            return result;
        }

        // Handle edge scrolling in fullscreen mode
        if !self.windowed {
            let at_edge = pixel.x < EDGE_SCROLL_SIZE
                || pixel.y < EDGE_SCROLL_SIZE
                || pixel.y >= display_height as i32 - EDGE_SCROLL_SIZE
                || pixel.x >= display_width as i32 - EDGE_SCROLL_SIZE;

            let inside_safe_zone = pixel.x >= EDGE_SCROLL_SIZE
                && pixel.y >= EDGE_SCROLL_SIZE
                && pixel.y < display_height as i32 - EDGE_SCROLL_SIZE
                && pixel.x < display_width as i32 - EDGE_SCROLL_SIZE;

            if self.is_scrolling && self.scroll_type == ScrollType::ScreenEdge && inside_safe_zone {
                result.stop_scrolling = true;
            } else if !self.is_scrolling && at_edge {
                result.start_edge_scrolling = true;
            }
        }

        // Handle rotation
        if self.is_rotating {
            const FACTOR: f32 = 0.01;
            result.angle_delta = FACTOR * (pixel.x - self.anchor.x) as f32;
            self.anchor = pixel;
        }

        // Handle pitch
        if self.is_pitching {
            const FACTOR: f32 = 0.01;
            result.pitch_delta = FACTOR * (pixel.y - self.anchor.y) as f32;
            self.anchor = pixel;
        }

        // Handle FOV adjustment (debug only)
        if self.is_changing_fov {
            const FACTOR: f32 = 0.01;
            result.fov_delta = FACTOR * (pixel.y - self.anchor.y) as f32;
            self.anchor = pixel;
        }

        result
    }

    /// Handle mouse wheel
    /// Matches C++ MSG_RAW_MOUSE_WHEEL from LookAtXlat.cpp
    pub fn handle_mouse_wheel(&mut self, spin: i32, current_frame: u32) -> i32 {
        self.last_mouse_move_frame = current_frame;
        spin
    }

    /// Calculate scroll offset for current frame
    /// Matches frame tick scroll calculation from C++ LookAtXlat.cpp
    pub fn calculate_scroll_offset(
        &mut self,
        display_width: u32,
        display_height: u32,
        should_move_rmb_anchor: bool,
    ) -> Coord2D {
        if !self.is_scrolling {
            return Coord2D::zero();
        }

        let mut offset = Coord2D::zero();

        match self.scroll_type {
            ScrollType::Rmb => {
                // Adjust anchor if needed
                if should_move_rmb_anchor {
                    let max_x = display_width as i32 / 2;
                    let max_y = display_height as i32 / 2;

                    if self.current_pos.x + max_x < self.anchor.x {
                        self.anchor.x = self.current_pos.x + max_x;
                    } else if self.current_pos.x - max_x > self.anchor.x {
                        self.anchor.x = self.current_pos.x - max_x;
                    }

                    if self.current_pos.y + max_y < self.anchor.y {
                        self.anchor.y = self.current_pos.y + max_y;
                    } else if self.current_pos.y - max_y > self.anchor.y {
                        self.anchor.y = self.current_pos.y - max_y;
                    }
                }

                offset.x = self.horizontal_scroll_speed_factor
                    * (self.current_pos.x - self.anchor.x) as f32;
                offset.y = self.vertical_scroll_speed_factor
                    * (self.current_pos.y - self.anchor.y) as f32;

                // Add minimum scroll based on normalized direction
                let mut vec = Coord2D::new(offset.x, offset.y);
                vec.normalize();
                offset.x += self.horizontal_scroll_speed_factor
                    * vec.x
                    * self.keyboard_scroll_factor.powi(2);
                offset.y += self.vertical_scroll_speed_factor
                    * vec.y
                    * self.keyboard_scroll_factor.powi(2);
            }
            ScrollType::Key => {
                if self.scroll_dir[Direction::Up as usize] {
                    offset.y -= self.vertical_scroll_speed_factor
                        * SCROLL_AMT as f32
                        * self.keyboard_scroll_factor;
                }
                if self.scroll_dir[Direction::Down as usize] {
                    offset.y += self.vertical_scroll_speed_factor
                        * SCROLL_AMT as f32
                        * self.keyboard_scroll_factor;
                }
                if self.scroll_dir[Direction::Left as usize] {
                    offset.x -= self.horizontal_scroll_speed_factor
                        * SCROLL_AMT as f32
                        * self.keyboard_scroll_factor;
                }
                if self.scroll_dir[Direction::Right as usize] {
                    offset.x += self.horizontal_scroll_speed_factor
                        * SCROLL_AMT as f32
                        * self.keyboard_scroll_factor;
                }
            }
            ScrollType::ScreenEdge => {
                if self.current_pos.y < EDGE_SCROLL_SIZE {
                    offset.y -= self.vertical_scroll_speed_factor
                        * SCROLL_AMT as f32
                        * self.keyboard_scroll_factor;
                }
                if self.current_pos.y >= display_height as i32 - EDGE_SCROLL_SIZE {
                    offset.y += self.vertical_scroll_speed_factor
                        * SCROLL_AMT as f32
                        * self.keyboard_scroll_factor;
                }
                if self.current_pos.x < EDGE_SCROLL_SIZE {
                    offset.x -= self.horizontal_scroll_speed_factor
                        * SCROLL_AMT as f32
                        * self.keyboard_scroll_factor;
                }
                if self.current_pos.x >= display_width as i32 - EDGE_SCROLL_SIZE {
                    offset.x += self.horizontal_scroll_speed_factor
                        * SCROLL_AMT as f32
                        * self.keyboard_scroll_factor;
                }
            }
            ScrollType::None => {}
        }

        offset
    }

    /// Save view to bookmark slot
    /// Matches C++ MSG_META_SAVE_VIEW1-8 from LookAtXlat.cpp
    pub fn save_view(&mut self, slot: usize, view: &View) -> bool {
        if slot > 0 && slot <= MAX_VIEW_LOCS {
            view.get_location(&mut self.view_location[slot - 1]);
            true
        } else {
            false
        }
    }

    /// Restore view from bookmark slot
    /// Matches C++ MSG_META_VIEW_VIEW1-8 from LookAtXlat.cpp
    pub fn restore_view(&self, slot: usize, view: &mut View) -> bool {
        if slot > 0 && slot <= MAX_VIEW_LOCS {
            view.set_location(&self.view_location[slot - 1]);
            true
        } else {
            false
        }
    }

    /// Begin pitch adjustment (debug feature)
    pub fn begin_adjust_pitch(&mut self) {
        self.is_pitching = true;
    }

    /// End pitch adjustment (debug feature)
    pub fn end_adjust_pitch(&mut self) {
        self.is_pitching = false;
    }

    /// Begin FOV adjustment (debug feature)
    pub fn begin_adjust_fov(&mut self) {
        self.is_changing_fov = true;
        self.anchor = self.current_pos;
    }

    /// End FOV adjustment (debug feature)
    pub fn end_adjust_fov(&mut self) {
        self.is_changing_fov = false;
    }

    pub fn is_scrolling(&self) -> bool {
        self.is_scrolling
    }

    pub fn is_rotating(&self) -> bool {
        self.is_rotating
    }

    pub fn is_pitching(&self) -> bool {
        self.is_pitching
    }
}

impl Default for LookAtTranslator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result from mouse move handling
#[derive(Debug, Default)]
pub struct MouseMoveResult {
    pub angle_delta: f32,
    pub pitch_delta: f32,
    pub fov_delta: f32,
    pub stop_scrolling: bool,
    pub start_edge_scrolling: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translator_creation() {
        let translator = LookAtTranslator::new();
        assert!(!translator.is_scrolling());
        assert!(!translator.is_rotating());
        assert!(!translator.is_pitching());
    }

    #[test]
    fn test_keyboard_scrolling() {
        let mut translator = LookAtTranslator::new();

        // Press up arrow
        translator.handle_key_event(KEY_UP, 0, false);
        assert!(translator.is_scrolling());

        // Release up arrow
        translator.handle_key_event(KEY_UP, KEY_STATE_UP, false);
        assert!(!translator.is_scrolling());
    }

    #[test]
    fn test_rmb_scrolling() {
        let mut translator = LookAtTranslator::new();
        let pixel = ICoord2D::new(100, 100);

        translator.handle_rmb_down(pixel, false, 0);
        assert!(translator.is_scrolling());
        assert_eq!(translator.get_rmb_scroll_anchor(), Some(pixel));

        translator.handle_rmb_up(1);
        assert!(!translator.is_scrolling());
        assert_eq!(translator.get_rmb_scroll_anchor(), None);
    }

    #[test]
    fn test_mmb_rotation() {
        let mut translator = LookAtTranslator::new();
        let pixel = ICoord2D::new(100, 100);

        translator.handle_mmb_down(pixel, 0, 0);
        assert!(translator.is_rotating());

        let reset = translator.handle_mmb_up(0, 0);
        assert!(!translator.is_rotating());
        assert!(reset); // Should reset because no movement
    }

    #[test]
    fn test_mmb_drag_no_reset() {
        let mut translator = LookAtTranslator::new();
        let start = ICoord2D::new(100, 100);
        let end = ICoord2D::new(120, 120);

        translator.handle_mmb_down(start, 0, 0);
        translator.set_current_pos(end);

        let reset = translator.handle_mmb_up(0, 10);
        assert!(!reset); // Should not reset because of movement
    }

    #[test]
    fn test_scroll_offset_calculation() {
        let mut translator = LookAtTranslator::new();
        translator.set_global_data(1.0, 1.0, 1.0, false, false);

        // Start keyboard scroll
        translator.handle_key_event(KEY_UP, 0, false);

        let offset = translator.calculate_scroll_offset(800, 600, false);
        assert!(offset.y < 0.0); // Moving up
        assert_eq!(offset.x, 0.0);
    }

    #[test]
    fn test_view_bookmarks() {
        let mut translator = LookAtTranslator::new();
        let mut view = View::new();
        view.set_position(&Coord3D::new(100.0, 200.0, 0.0));
        view.set_angle(1.5);

        // Save to slot 1
        assert!(translator.save_view(1, &view));

        // Change view
        view.set_position(&Coord3D::new(500.0, 600.0, 0.0));
        view.set_angle(2.5);

        // Restore from slot 1
        assert!(translator.restore_view(1, &mut view));
        let pos = view.get_position();
        assert_eq!(pos.x, 100.0);
        assert_eq!(pos.y, 200.0);
        assert_eq!(view.get_angle(), 1.5);
    }

    #[test]
    fn test_mouse_move_rotation() {
        let mut translator = LookAtTranslator::new();
        let start = ICoord2D::new(100, 100);
        let end = ICoord2D::new(110, 100);

        translator.handle_mmb_down(start, 0, 0);

        let result = translator.handle_mouse_move(end, 1, 800, 600, true);
        assert!(result.angle_delta > 0.0);
        assert_eq!(result.pitch_delta, 0.0);
    }

    #[test]
    fn test_edge_scrolling_detection() {
        let mut translator = LookAtTranslator::new();
        translator.set_global_data(1.0, 1.0, 1.0, false, false); // fullscreen

        // Move to edge
        let edge_pos = ICoord2D::new(1, 300);
        let result = translator.handle_mouse_move(edge_pos, 0, 800, 600, true);
        assert!(result.start_edge_scrolling);
    }

    #[test]
    fn test_reset_modes() {
        let mut translator = LookAtTranslator::new();

        translator.is_scrolling = true;
        translator.is_rotating = true;
        translator.is_pitching = true;

        translator.reset_modes();

        assert!(!translator.is_scrolling());
        assert!(!translator.is_rotating());
        assert!(!translator.is_pitching());
    }

    #[test]
    fn test_has_mouse_moved_recently() {
        let mut translator = LookAtTranslator::new();

        translator.last_mouse_move_frame = 100;
        assert!(translator.has_mouse_moved_recently(110));
        assert!(!translator.has_mouse_moved_recently(200));
    }
}
