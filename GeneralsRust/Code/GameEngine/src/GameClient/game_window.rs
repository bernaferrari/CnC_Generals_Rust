// FILE: game_window.rs
// Author: Ported to provide cross-platform window management
// Desc: Cross-platform window management using winit
//
// Provides window creation, event handling, and display mode management
// Replaces Win32-specific windowing code from C++ implementation

use winit::{
    dpi::{LogicalSize, PhysicalSize, PhysicalPosition},
    event::{Event, WindowEvent, KeyboardInput, ElementState, MouseButton, MouseScrollDelta},
    event_loop::{EventLoop, ControlFlow},
    window::{Window, WindowBuilder, Fullscreen, Icon},
    monitor::{MonitorHandle, VideoMode},
};
use std::sync::Arc;
use parking_lot::RwLock;
use std::time::{Duration, Instant};

/// Window creation parameters
#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub vsync: bool,
    pub resizable: bool,
    pub decorated: bool,
    pub maximized: bool,
    pub visible: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Command & Conquer: Generals Zero Hour".to_string(),
            width: 1024,
            height: 768,
            fullscreen: false,
            vsync: true,
            resizable: true,
            decorated: true,
            maximized: false,
            visible: true,
        }
    }
}

/// Display mode information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayModeInfo {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u32,
    pub refresh_rate: u32,
}

impl DisplayModeInfo {
    pub fn from_video_mode(mode: &VideoMode) -> Self {
        let size = mode.size();
        Self {
            width: size.width,
            height: size.height,
            bit_depth: mode.bit_depth() as u32,
            refresh_rate: mode.refresh_rate_millihertz() / 1000,
        }
    }
}

/// Input event types
#[derive(Debug, Clone)]
pub enum InputEvent {
    KeyPressed { key_code: u32, scancode: u32 },
    KeyReleased { key_code: u32, scancode: u32 },
    MouseMoved { x: f64, y: f64 },
    MouseButtonPressed { button: MouseButtonType, x: f64, y: f64 },
    MouseButtonReleased { button: MouseButtonType, x: f64, y: f64 },
    MouseWheel { delta_x: f32, delta_y: f32 },
    WindowResized { width: u32, height: u32 },
    WindowFocusChanged { focused: bool },
    WindowCloseRequested,
}

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButtonType {
    Left,
    Right,
    Middle,
    Other(u16),
}

impl From<MouseButton> for MouseButtonType {
    fn from(button: MouseButton) -> Self {
        match button {
            MouseButton::Left => MouseButtonType::Left,
            MouseButton::Right => MouseButtonType::Right,
            MouseButton::Middle => MouseButtonType::Middle,
            MouseButton::Other(id) => MouseButtonType::Other(id),
        }
    }
}

/// Window state
#[derive(Debug, Clone)]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub focused: bool,
    pub minimized: bool,
    pub cursor_position: (f64, f64),
    pub cursor_visible: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 1024,
            height: 768,
            fullscreen: false,
            focused: true,
            minimized: false,
            cursor_position: (0.0, 0.0),
            cursor_visible: true,
        }
    }
}

/// Frame timing information
#[derive(Debug, Clone, Copy)]
pub struct FrameTiming {
    pub last_frame_time: Instant,
    pub delta_time: Duration,
    pub target_frame_time: Duration,
    pub fps_limit: Option<u32>,
}

impl Default for FrameTiming {
    fn default() -> Self {
        Self {
            last_frame_time: Instant::now(),
            delta_time: Duration::from_secs(0),
            target_frame_time: Duration::from_micros(16667), // ~60 FPS
            fps_limit: Some(60),
        }
    }
}

impl FrameTiming {
    /// Create timing with a specific FPS limit
    pub fn with_fps_limit(fps: u32) -> Self {
        let target = if fps > 0 {
            Duration::from_secs_f64(1.0 / fps as f64)
        } else {
            Duration::from_micros(16667)
        };

        Self {
            last_frame_time: Instant::now(),
            delta_time: Duration::from_secs(0),
            target_frame_time: target,
            fps_limit: Some(fps),
        }
    }

    /// Update timing for a new frame
    pub fn update(&mut self) {
        let now = Instant::now();
        self.delta_time = now.duration_since(self.last_frame_time);
        self.last_frame_time = now;
    }

    /// Check if enough time has passed for the next frame
    pub fn should_render(&self) -> bool {
        self.delta_time >= self.target_frame_time
    }

    /// Get delta time in seconds
    pub fn delta_seconds(&self) -> f32 {
        self.delta_time.as_secs_f32()
    }

    /// Get current FPS
    pub fn current_fps(&self) -> f32 {
        if self.delta_time.as_secs_f32() > 0.0 {
            1.0 / self.delta_time.as_secs_f32()
        } else {
            0.0
        }
    }
}

/// Game window manager using winit for cross-platform support
pub struct GameWindow {
    window: Arc<Window>,
    state: RwLock<WindowState>,
    timing: RwLock<FrameTiming>,
    available_modes: Vec<DisplayModeInfo>,
    primary_monitor: Option<MonitorHandle>,
}

impl GameWindow {
    /// Create a new game window
    pub fn new(event_loop: &EventLoop<()>, config: WindowConfig) -> Result<Self, String> {
        // Get primary monitor
        let primary_monitor = event_loop.primary_monitor();

        // Enumerate available display modes
        let available_modes = if let Some(ref monitor) = primary_monitor {
            monitor
                .video_modes()
                .map(|mode| DisplayModeInfo::from_video_mode(&mode))
                .collect()
        } else {
            Vec::new()
        };

        // Determine fullscreen mode
        let fullscreen = if config.fullscreen {
            if let Some(ref monitor) = primary_monitor {
                // Find matching video mode or use current
                let video_mode = monitor
                    .video_modes()
                    .find(|mode| {
                        let size = mode.size();
                        size.width == config.width && size.height == config.height
                    });

                if let Some(mode) = video_mode {
                    Some(Fullscreen::Exclusive(mode))
                } else {
                    Some(Fullscreen::Borderless(Some(monitor.clone())))
                }
            } else {
                None
            }
        } else {
            None
        };

        // Build the window
        let window = WindowBuilder::new()
            .with_title(&config.title)
            .with_inner_size(LogicalSize::new(config.width, config.height))
            .with_fullscreen(fullscreen)
            .with_resizable(config.resizable)
            .with_decorations(config.decorated)
            .with_maximized(config.maximized)
            .with_visible(config.visible)
            .build(event_loop)
            .map_err(|e| format!("Failed to create window: {}", e))?;

        let window = Arc::new(window);

        let state = WindowState {
            width: config.width,
            height: config.height,
            fullscreen: config.fullscreen,
            ..Default::default()
        };

        let timing = if config.vsync {
            FrameTiming::with_fps_limit(60)
        } else {
            FrameTiming::default()
        };

        Ok(Self {
            window,
            state: RwLock::new(state),
            timing: RwLock::new(timing),
            available_modes,
            primary_monitor,
        })
    }

    /// Get window reference
    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    /// Get window state (read-only)
    pub fn state(&self) -> parking_lot::RwLockReadGuard<WindowState> {
        self.state.read()
    }

    /// Get window state (mutable)
    pub fn state_mut(&self) -> parking_lot::RwLockWriteGuard<WindowState> {
        self.state.write()
    }

    /// Get frame timing (read-only)
    pub fn timing(&self) -> parking_lot::RwLockReadGuard<FrameTiming> {
        self.timing.read()
    }

    /// Get frame timing (mutable)
    pub fn timing_mut(&self) -> parking_lot::RwLockWriteGuard<FrameTiming> {
        self.timing.write()
    }

    /// Get window dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        let state = self.state.read();
        (state.width, state.height)
    }

    /// Set window size
    pub fn set_size(&self, width: u32, height: u32) {
        self.window.set_inner_size(LogicalSize::new(width, height));
        let mut state = self.state.write();
        state.width = width;
        state.height = height;
    }

    /// Toggle fullscreen mode
    pub fn toggle_fullscreen(&self) {
        let mut state = self.state.write();
        state.fullscreen = !state.fullscreen;

        if state.fullscreen {
            if let Some(ref monitor) = self.primary_monitor {
                let fullscreen = Fullscreen::Borderless(Some(monitor.clone()));
                self.window.set_fullscreen(Some(fullscreen));
            }
        } else {
            self.window.set_fullscreen(None);
        }
    }

    /// Set fullscreen mode
    pub fn set_fullscreen(&self, fullscreen: bool) {
        let mut state = self.state.write();
        if state.fullscreen != fullscreen {
            state.fullscreen = fullscreen;

            if fullscreen {
                if let Some(ref monitor) = self.primary_monitor {
                    let fullscreen = Fullscreen::Borderless(Some(monitor.clone()));
                    self.window.set_fullscreen(Some(fullscreen));
                }
            } else {
                self.window.set_fullscreen(None);
            }
        }
    }

    /// Set display mode (resolution and fullscreen)
    pub fn set_display_mode(&self, width: u32, height: u32, fullscreen: bool) -> bool {
        // Try to find matching video mode for exclusive fullscreen
        if fullscreen {
            if let Some(ref monitor) = self.primary_monitor {
                let video_mode = monitor
                    .video_modes()
                    .find(|mode| {
                        let size = mode.size();
                        size.width == width && size.height == height
                    });

                if let Some(mode) = video_mode {
                    self.window.set_fullscreen(Some(Fullscreen::Exclusive(mode)));
                } else {
                    // Fall back to borderless fullscreen
                    self.window.set_fullscreen(Some(Fullscreen::Borderless(Some(monitor.clone()))));
                    self.window.set_inner_size(LogicalSize::new(width, height));
                }
            } else {
                return false;
            }
        } else {
            self.window.set_fullscreen(None);
            self.window.set_inner_size(LogicalSize::new(width, height));
        }

        let mut state = self.state.write();
        state.width = width;
        state.height = height;
        state.fullscreen = fullscreen;

        true
    }

    /// Get list of available display modes
    pub fn available_display_modes(&self) -> &[DisplayModeInfo] {
        &self.available_modes
    }

    /// Get current display mode
    pub fn current_display_mode(&self) -> DisplayModeInfo {
        let state = self.state.read();
        DisplayModeInfo {
            width: state.width,
            height: state.height,
            bit_depth: 32, // Modern displays are typically 32-bit
            refresh_rate: 60, // Default assumption
        }
    }

    /// Show/hide cursor
    pub fn set_cursor_visible(&self, visible: bool) {
        self.window.set_cursor_visible(visible);
        let mut state = self.state.write();
        state.cursor_visible = visible;
    }

    /// Check if cursor is visible
    pub fn is_cursor_visible(&self) -> bool {
        self.state.read().cursor_visible
    }

    /// Set window title
    pub fn set_title(&self, title: &str) {
        self.window.set_title(title);
    }

    /// Request redraw
    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    /// Process window event
    pub fn process_event(&self, event: &WindowEvent) -> Option<InputEvent> {
        match event {
            WindowEvent::Resized(physical_size) => {
                let mut state = self.state.write();
                state.width = physical_size.width;
                state.height = physical_size.height;
                Some(InputEvent::WindowResized {
                    width: physical_size.width,
                    height: physical_size.height,
                })
            }

            WindowEvent::CloseRequested => {
                Some(InputEvent::WindowCloseRequested)
            }

            WindowEvent::Focused(focused) => {
                let mut state = self.state.write();
                state.focused = *focused;
                Some(InputEvent::WindowFocusChanged { focused: *focused })
            }

            WindowEvent::CursorMoved { position, .. } => {
                let mut state = self.state.write();
                state.cursor_position = (position.x, position.y);
                Some(InputEvent::MouseMoved {
                    x: position.x,
                    y: position.y,
                })
            }

            WindowEvent::MouseInput { state: button_state, button, .. } => {
                let cursor_pos = self.state.read().cursor_position;
                let button_type = MouseButtonType::from(*button);

                match button_state {
                    ElementState::Pressed => Some(InputEvent::MouseButtonPressed {
                        button: button_type,
                        x: cursor_pos.0,
                        y: cursor_pos.1,
                    }),
                    ElementState::Released => Some(InputEvent::MouseButtonReleased {
                        button: button_type,
                        x: cursor_pos.0,
                        y: cursor_pos.1,
                    }),
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let (delta_x, delta_y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    MouseScrollDelta::PixelDelta(pos) => (pos.x as f32, pos.y as f32),
                };

                Some(InputEvent::MouseWheel { delta_x, delta_y })
            }

            WindowEvent::KeyboardInput {
                input: KeyboardInput {
                    state: key_state,
                    virtual_keycode,
                    scancode,
                    ..
                },
                ..
            } => {
                let key_code = virtual_keycode.map(|k| k as u32).unwrap_or(0);

                match key_state {
                    ElementState::Pressed => Some(InputEvent::KeyPressed {
                        key_code,
                        scancode: *scancode,
                    }),
                    ElementState::Released => Some(InputEvent::KeyReleased {
                        key_code,
                        scancode: *scancode,
                    }),
                }
            }

            _ => None,
        }
    }

    /// Update frame timing
    pub fn update_timing(&self) {
        self.timing.write().update();
    }

    /// Set FPS limit
    pub fn set_fps_limit(&self, fps: u32) {
        let mut timing = self.timing.write();
        timing.fps_limit = Some(fps);
        timing.target_frame_time = if fps > 0 {
            Duration::from_secs_f64(1.0 / fps as f64)
        } else {
            Duration::from_micros(16667)
        };
    }

    /// Get current FPS
    pub fn current_fps(&self) -> f32 {
        self.timing.read().current_fps()
    }

    /// Get delta time
    pub fn delta_time(&self) -> f32 {
        self.timing.read().delta_seconds()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_config_default() {
        let config = WindowConfig::default();
        assert_eq!(config.width, 1024);
        assert_eq!(config.height, 768);
        assert!(!config.fullscreen);
        assert!(config.vsync);
        assert!(config.resizable);
    }

    #[test]
    fn test_display_mode_info() {
        let mode = DisplayModeInfo {
            width: 1920,
            height: 1080,
            bit_depth: 32,
            refresh_rate: 60,
        };

        assert_eq!(mode.width, 1920);
        assert_eq!(mode.height, 1080);
        assert_eq!(mode.bit_depth, 32);
        assert_eq!(mode.refresh_rate, 60);
    }

    #[test]
    fn test_frame_timing() {
        let mut timing = FrameTiming::with_fps_limit(60);
        assert_eq!(timing.fps_limit, Some(60));

        std::thread::sleep(Duration::from_millis(20));
        timing.update();

        assert!(timing.delta_time.as_millis() >= 20);
        assert!(timing.current_fps() > 0.0);
    }

    #[test]
    fn test_mouse_button_conversion() {
        assert_eq!(
            MouseButtonType::from(MouseButton::Left),
            MouseButtonType::Left
        );
        assert_eq!(
            MouseButtonType::from(MouseButton::Right),
            MouseButtonType::Right
        );
        assert_eq!(
            MouseButtonType::from(MouseButton::Middle),
            MouseButtonType::Middle
        );
    }

    #[test]
    fn test_window_state_default() {
        let state = WindowState::default();
        assert_eq!(state.width, 1024);
        assert_eq!(state.height, 768);
        assert!(!state.fullscreen);
        assert!(state.focused);
        assert!(!state.minimized);
        assert!(state.cursor_visible);
    }
}
