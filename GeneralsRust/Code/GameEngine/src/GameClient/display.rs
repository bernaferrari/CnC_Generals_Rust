// FILE: display.rs
// Author: Ported from C++ Display.h/Display.cpp
// Desc: The graphics display system with cross-platform windowing
// Original Author: Michael S. Booth, March 2001
//
// Matches C++ from:
// /GeneralsMD/Code/GameEngine/Include/GameClient/Display.h
// /GeneralsMD/Code/GameEngine/Source/GameClient/Display.cpp

use super::view::View;
use super::video_buffer::VideoBuffer;
use super::video_stream::VideoStreamInterface;
use super::types::*;
use std::sync::Arc;
use parking_lot::RwLock;
use std::time::{Duration, Instant};

/// Shroud level for fog of war
/// Matches C++ Display.h ShroudLevel struct
#[derive(Debug, Clone, Copy)]
pub struct ShroudLevel {
    /// A Value of 1 means shrouded. 0 is not. Negative is the count of people looking.
    pub current_shroud: i16,
    /// A Value of 0 means passive shroud. Positive is the count of people shrouding.
    pub active_shroud_level: i16,
}

impl Default for ShroudLevel {
    fn default() -> Self {
        Self {
            current_shroud: 0,
            active_shroud_level: 0,
        }
    }
}

/// Cell shroud status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellShroudStatus {
    Clear,
    Fogged,
    Shrouded,
}

/// Draw image modes
/// Matches C++ Display.h DrawImageMode enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawImageMode {
    Solid,      // draw image without blending and ignoring alpha
    Grayscale,  // draw image in grayscale
    Alpha,      // alpha blend the image into frame buffer
    Additive,   // additive blend the image into frame buffer
}

/// Time of day for lighting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeOfDay {
    Morning,
    Afternoon,
    Evening,
    Night,
}

/// Display settings for resolution changes
/// Matches C++ Display.h DisplaySettings struct
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplaySettings {
    pub x_res: u32,           // Resolution width
    pub y_res: u32,           // Resolution height
    pub bit_depth: u32,       // Color depth
    pub windowed: bool,       // Window mode: true = windowed, false = fullscreen
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            x_res: 1024,
            y_res: 768,
            bit_depth: 32,
            windowed: true,
        }
    }
}

/// Display mode description
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u32,
    pub refresh_rate: u32,
}

/// Debug display callback trait
pub trait DebugDisplayCallback: Send + Sync {
    fn display_debug(&self, display: &Display);
}

/// The Display class implements the display subsystem
/// Matches C++ Display class from Display.h/Display.cpp
pub struct Display {
    // Dimensions of the display
    width: u32,
    height: u32,
    bit_depth: u32,

    // TRUE when windowed, FALSE when fullscreen
    windowed: bool,

    // All of the views into the world
    view_list: Option<Box<View>>,

    // Video playback data
    video_buffer: Option<Box<VideoBuffer>>,
    video_stream: Option<Box<dyn VideoStreamInterface>>,
    currently_playing_movie: String,

    // Cinematic text data
    cinematic_text: String,
    cinematic_text_frames: i32,

    // Debug display callback
    debug_display_callback: Option<Arc<dyn DebugDisplayCallback>>,

    // Letterbox mode
    letterbox_fade_level: f32,
    letterbox_enabled: bool,
    letterbox_fade_start_time: Instant,

    // Movie timing
    movie_hold_time: i32,
    copyright_hold_time: i32,
    elapsed_movie_time: Instant,
    elapsed_copyright_time: Instant,

    // Frame statistics
    frame_count: u64,
    last_fps_update: Instant,
    current_fps: f32,
    average_fps: f32,
    last_frame_draw_calls: u32,
}

impl Display {
    /// Create a new Display
    /// Matches C++ Display::Display() from Display.cpp
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            bit_depth: 0,
            windowed: false,
            view_list: None,
            video_buffer: None,
            video_stream: None,
            currently_playing_movie: String::new(),
            cinematic_text: String::new(),
            cinematic_text_frames: 0,
            debug_display_callback: None,
            letterbox_fade_level: 0.0,
            letterbox_enabled: false,
            letterbox_fade_start_time: Instant::now(),
            movie_hold_time: -1,
            copyright_hold_time: -1,
            elapsed_movie_time: Instant::now(),
            elapsed_copyright_time: Instant::now(),
            frame_count: 0,
            last_fps_update: Instant::now(),
            current_fps: 0.0,
            average_fps: 0.0,
            last_frame_draw_calls: 0,
        }
    }

    /// Initialize the display
    pub fn init(&mut self) {
        // Subclasses will override this
    }

    /// Reset the display system
    /// Matches C++ Display::reset() from Display.cpp
    pub fn reset(&mut self) {
        // Remove letterbox border that may have been enabled by a script
        self.letterbox_fade_level = 0.0;
        self.letterbox_enabled = false;
        self.stop_movie();

        // Reset all views that need resetting
        if let Some(ref mut view) = self.view_list {
            view.reset();
        }
    }

    /// Update the display system
    /// Matches C++ Display::update() from Display.cpp
    pub fn update(&mut self) {
        if let Some(ref mut stream) = self.video_stream {
            if let Some(ref mut buffer) = self.video_buffer {
                if stream.is_frame_ready() {
                    stream.frame_decompress();
                    stream.frame_render(buffer);

                    if stream.frame_index() != stream.frame_count() - 1 {
                        stream.frame_next();
                    } else if self.copyright_hold_time >= 0 || self.movie_hold_time >= 0 {
                        // Handle movie hold time and copyright display
                        let movie_elapsed = self.elapsed_movie_time.elapsed().as_millis() as i32;
                        let copyright_elapsed = self.elapsed_copyright_time.elapsed().as_millis() as i32;

                        if self.movie_hold_time + movie_elapsed > 0 &&
                           self.copyright_hold_time + copyright_elapsed > 0 {
                            self.movie_hold_time = -1;
                            self.copyright_hold_time = -1;
                        }
                    } else {
                        self.stop_movie();
                    }
                }
            }
        }

        // Update FPS counter
        self.update_fps();
    }

    // Display attribute methods

    /// Sets the width of the display
    /// Matches C++ Display::setWidth() from Display.cpp
    pub fn set_width(&mut self, width: u32) {
        self.width = width;
    }

    /// Sets the height of the display
    /// Matches C++ Display::setHeight() from Display.cpp
    pub fn set_height(&mut self, height: u32) {
        self.height = height;
    }

    /// Returns the width of the display
    pub fn get_width(&self) -> u32 {
        self.width
    }

    /// Returns the height of the display
    pub fn get_height(&self) -> u32 {
        self.height
    }

    /// Set bit depth
    pub fn set_bit_depth(&mut self, bit_depth: u32) {
        self.bit_depth = bit_depth;
    }

    /// Get bit depth
    pub fn get_bit_depth(&self) -> u32 {
        self.bit_depth
    }

    /// Set windowed/fullscreen flag
    pub fn set_windowed(&mut self, windowed: bool) {
        self.windowed = windowed;
    }

    /// Return windowed/fullscreen flag
    pub fn get_windowed(&self) -> bool {
        self.windowed
    }

    /// Sets screen resolution/mode
    /// Matches C++ Display::setDisplayMode() from Display.cpp
    pub fn set_display_mode(&mut self, xres: u32, yres: u32, bitdepth: u32, windowed: bool) -> bool {
        // Get old values for view adjustment
        let old_display_width = self.get_width();
        let old_display_height = self.get_height();

        self.set_width(xres);
        self.set_height(yres);
        self.set_bit_depth(bitdepth);
        self.set_windowed(windowed);

        // Adjust view to match previous proportions
        if let Some(ref mut view) = self.view_list {
            let old_view_width = view.get_width();
            let old_view_height = view.get_height();
            let (old_view_origin_x, old_view_origin_y) = view.get_origin();

            if old_display_width > 0 && old_display_height > 0 {
                let new_view_width = (old_view_width as f32 / old_display_width as f32 * xres as f32) as i32;
                let new_view_height = (old_view_height as f32 / old_display_height as f32 * yres as f32) as i32;
                let new_origin_x = (old_view_origin_x as f32 / old_display_width as f32 * xres as f32) as i32;
                let new_origin_y = (old_view_origin_y as f32 / old_display_height as f32 * yres as f32) as i32;

                view.set_width(new_view_width);
                view.set_height(new_view_height);
                view.set_origin(new_origin_x, new_origin_y);
            }
        }

        true
    }

    /// Return number of display modes/resolutions supported
    pub fn get_display_mode_count(&self) -> i32 {
        // Override in platform-specific implementation
        0
    }

    /// Return description of mode
    pub fn get_display_mode_description(&self, _mode_index: i32) -> Option<DisplayMode> {
        // Override in platform-specific implementation
        None
    }

    // View management

    /// Attach the given view to the world
    /// Matches C++ Display::attachView() from Display.cpp
    pub fn attach_view(&mut self, view: Box<View>) {
        // Prepend to head of list
        let mut new_view = view;
        new_view.next = self.view_list.take();
        self.view_list = Some(new_view);
    }

    /// Return the first view of the world
    pub fn get_first_view(&self) -> Option<&View> {
        self.view_list.as_deref()
    }

    /// Return the first view of the world (mutable)
    pub fn get_first_view_mut(&mut self) -> Option<&mut View> {
        self.view_list.as_deref_mut()
    }

    /// Render all views of the world
    /// Matches C++ Display::drawViews() from Display.cpp
    pub fn draw_views(&self) {
        // In full implementation, this would iterate views and call drawView()
        // For now, this is a placeholder that subclasses will override
    }

    /// Updates state of world views
    /// Matches C++ Display::updateViews() from Display.cpp
    pub fn update_views(&mut self) {
        // In full implementation, this would iterate views and call updateView()
        // For now, this is a placeholder that subclasses will override
    }

    /// Delete all views in the Display
    /// Matches C++ Display::deleteViews() from Display.cpp
    pub fn delete_views(&mut self) {
        self.view_list = None;
    }

    // Drawing methods (to be implemented by platform-specific subclass)

    /// Redraw the entire display
    /// Matches C++ Display::draw() from Display.cpp
    pub fn draw(&self) {
        // redraw all views
        self.draw_views();
    }

    // Movie playback

    /// Play a logo movie with minimum display times
    /// Matches C++ Display::playLogoMovie() from Display.cpp
    pub fn play_logo_movie(&mut self, movie_name: &str, min_movie_length: i32, min_copyright_length: i32) {
        self.stop_movie();
        self.currently_playing_movie = movie_name.to_string();
        self.movie_hold_time = min_movie_length;
        self.copyright_hold_time = min_copyright_length;
        self.elapsed_movie_time = Instant::now();
    }

    /// Play a movie
    /// Matches C++ Display::playMovie() from Display.cpp
    pub fn play_movie(&mut self, movie_name: &str) {
        self.stop_movie();
        self.currently_playing_movie = movie_name.to_string();
    }

    /// Stop movie playback
    /// Matches C++ Display::stopMovie() from Display.cpp
    pub fn stop_movie(&mut self) {
        self.video_buffer = None;
        self.video_stream = None;
        self.currently_playing_movie.clear();
        self.copyright_hold_time = -1;
        self.movie_hold_time = -1;
    }

    /// Is a movie currently playing?
    /// Matches C++ Display::isMoviePlaying() from Display.cpp
    pub fn is_movie_playing(&self) -> bool {
        self.video_stream.is_some() && self.video_buffer.is_some()
    }

    // Cinematic text

    /// Set cinematic text
    pub fn set_cinematic_text(&mut self, text: String) {
        self.cinematic_text = text;
    }

    /// Set cinematic text display frames
    pub fn set_cinematic_text_frames(&mut self, frames: i32) {
        self.cinematic_text_frames = frames;
    }

    /// Get cinematic text
    pub fn get_cinematic_text(&self) -> &str {
        &self.cinematic_text
    }

    // Letterbox mode

    /// Enable/disable letterbox mode
    pub fn enable_letterbox(&mut self, enable: bool) {
        self.letterbox_enabled = enable;
        if enable {
            self.letterbox_fade_start_time = Instant::now();
        }
    }

    /// Is letterbox fading?
    pub fn is_letterbox_fading(&self) -> bool {
        false // Placeholder - full implementation would check fade state
    }

    /// Is letterbox enabled?
    pub fn is_letterboxed(&self) -> bool {
        self.letterbox_enabled
    }

    /// Toggle letterbox mode
    pub fn toggle_letterbox(&mut self) {
        self.letterbox_enabled = !self.letterbox_enabled;
        if self.letterbox_enabled {
            self.letterbox_fade_start_time = Instant::now();
        }
    }

    // Debug display

    /// Register debug display callback
    /// Matches C++ Display::setDebugDisplayCallback() from Display.cpp
    pub fn set_debug_display_callback(&mut self, callback: Arc<dyn DebugDisplayCallback>) {
        self.debug_display_callback = Some(callback);
    }

    /// Get debug display callback
    /// Matches C++ Display::getDebugDisplayCallback() from Display.cpp
    pub fn get_debug_display_callback(&self) -> Option<Arc<dyn DebugDisplayCallback>> {
        self.debug_display_callback.clone()
    }

    // Performance metrics

    /// Update FPS counter
    fn update_fps(&mut self) {
        self.frame_count += 1;
        let elapsed = self.last_fps_update.elapsed();

        if elapsed >= Duration::from_secs(1) {
            self.current_fps = self.frame_count as f32 / elapsed.as_secs_f32();
            self.average_fps = (self.average_fps * 0.9) + (self.current_fps * 0.1);
            self.frame_count = 0;
            self.last_fps_update = Instant::now();
        }
    }

    /// Returns the average FPS
    /// Matches C++ Display::getAverageFPS() from Display.cpp
    pub fn get_average_fps(&self) -> f32 {
        self.average_fps
    }

    /// Returns the current FPS
    pub fn get_current_fps(&self) -> f32 {
        self.current_fps
    }

    /// Returns the number of draw calls issued in the previous frame
    /// Matches C++ Display::getLastFrameDrawCalls() from Display.cpp
    pub fn get_last_frame_draw_calls(&self) -> u32 {
        self.last_frame_draw_calls
    }

    /// Set the number of draw calls for the current frame
    pub fn set_last_frame_draw_calls(&mut self, count: u32) {
        self.last_frame_draw_calls = count;
    }
}

impl Default for Display {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Display {
    /// Matches C++ Display::~Display() from Display.cpp
    fn drop(&mut self) {
        self.stop_movie();
        self.delete_views();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_creation() {
        let display = Display::new();
        assert_eq!(display.get_width(), 0);
        assert_eq!(display.get_height(), 0);
        assert_eq!(display.get_bit_depth(), 0);
        assert!(!display.get_windowed());
    }

    #[test]
    fn test_display_dimensions() {
        let mut display = Display::new();
        display.set_width(1024);
        display.set_height(768);
        display.set_bit_depth(32);

        assert_eq!(display.get_width(), 1024);
        assert_eq!(display.get_height(), 768);
        assert_eq!(display.get_bit_depth(), 32);
    }

    #[test]
    fn test_display_mode() {
        let mut display = Display::new();
        assert!(display.set_display_mode(1920, 1080, 32, true));

        assert_eq!(display.get_width(), 1920);
        assert_eq!(display.get_height(), 1080);
        assert_eq!(display.get_bit_depth(), 32);
        assert!(display.get_windowed());
    }

    #[test]
    fn test_windowed_mode() {
        let mut display = Display::new();
        assert!(!display.get_windowed());

        display.set_windowed(true);
        assert!(display.get_windowed());

        display.set_windowed(false);
        assert!(!display.get_windowed());
    }

    #[test]
    fn test_movie_playback() {
        let mut display = Display::new();
        assert!(!display.is_movie_playing());

        display.play_movie("test.bik");
        assert_eq!(display.currently_playing_movie, "test.bik");

        display.stop_movie();
        assert!(display.currently_playing_movie.is_empty());
        assert!(!display.is_movie_playing());
    }

    #[test]
    fn test_letterbox_mode() {
        let mut display = Display::new();
        assert!(!display.is_letterboxed());

        display.enable_letterbox(true);
        assert!(display.is_letterboxed());

        display.toggle_letterbox();
        assert!(!display.is_letterboxed());
    }

    #[test]
    fn test_cinematic_text() {
        let mut display = Display::new();
        display.set_cinematic_text("Test caption".to_string());
        display.set_cinematic_text_frames(60);

        assert_eq!(display.get_cinematic_text(), "Test caption");
        assert_eq!(display.cinematic_text_frames, 60);
    }

    #[test]
    fn test_display_reset() {
        let mut display = Display::new();
        display.enable_letterbox(true);
        display.play_movie("test.bik");

        display.reset();

        assert!(!display.is_letterboxed());
        assert!(!display.is_movie_playing());
    }

    #[test]
    fn test_fps_tracking() {
        let display = Display::new();
        assert_eq!(display.get_current_fps(), 0.0);
        assert_eq!(display.get_average_fps(), 0.0);
    }
}
