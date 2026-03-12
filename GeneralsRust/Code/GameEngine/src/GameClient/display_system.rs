// FILE: display_system.rs
// Author: Integration layer for Display and Window management
// Desc: Integrates Display abstraction with GameWindow implementation
//
// This module provides the complete display system that combines:
// - Abstract Display interface (from C++ Display.h/Display.cpp)
// - Concrete GameWindow implementation (using winit)
// - Input event routing and handling
// - Frame timing and rendering loop

use super::display::{Display, DisplaySettings, DrawImageMode, TimeOfDay};
use super::game_window::{
    GameWindow, WindowConfig, InputEvent, FrameTiming, DisplayModeInfo, MouseButtonType,
};
use super::view::View;
use winit::event_loop::{EventLoop, ControlFlow};
use winit::event::{Event, WindowEvent};
use std::sync::Arc;
use parking_lot::RwLock;
use std::time::{Duration, Instant};

/// Input event handler trait
pub trait InputEventHandler: Send + Sync {
    fn handle_key_pressed(&mut self, key_code: u32, scancode: u32);
    fn handle_key_released(&mut self, key_code: u32, scancode: u32);
    fn handle_mouse_moved(&mut self, x: f64, y: f64);
    fn handle_mouse_button_pressed(&mut self, button: MouseButtonType, x: f64, y: f64);
    fn handle_mouse_button_released(&mut self, button: MouseButtonType, x: f64, y: f64);
    fn handle_mouse_wheel(&mut self, delta_x: f32, delta_y: f32);
    fn handle_window_resized(&mut self, width: u32, height: u32);
    fn handle_window_focus_changed(&mut self, focused: bool);
}

/// Complete display system integrating Display and GameWindow
pub struct DisplaySystem {
    display: Display,
    window: Option<Arc<GameWindow>>,
    event_handlers: Vec<Arc<RwLock<dyn InputEventHandler>>>,
    running: bool,
    target_fps: u32,
}

impl DisplaySystem {
    /// Create a new display system
    pub fn new() -> Self {
        Self {
            display: Display::new(),
            window: None,
            event_handlers: Vec::new(),
            running: false,
            target_fps: 60,
        }
    }

    /// Initialize the display system with a window
    pub fn init(&mut self, event_loop: &EventLoop<()>, config: WindowConfig) -> Result<(), String> {
        // Create the game window
        let window = GameWindow::new(event_loop, config.clone())?;

        // Set display dimensions from window
        self.display.set_width(config.width);
        self.display.set_height(config.height);
        self.display.set_windowed(!config.fullscreen);
        self.display.set_bit_depth(32);

        // Initialize display
        self.display.init();

        self.window = Some(Arc::new(window));
        self.running = true;

        Ok(())
    }

    /// Get reference to the display
    pub fn display(&self) -> &Display {
        &self.display
    }

    /// Get mutable reference to the display
    pub fn display_mut(&mut self) -> &mut Display {
        &mut self.display
    }

    /// Get reference to the game window
    pub fn window(&self) -> Option<&Arc<GameWindow>> {
        self.window.as_ref()
    }

    /// Add an input event handler
    pub fn add_event_handler(&mut self, handler: Arc<RwLock<dyn InputEventHandler>>) {
        self.event_handlers.push(handler);
    }

    /// Set display mode
    pub fn set_display_mode(&mut self, width: u32, height: u32, fullscreen: bool) -> bool {
        if let Some(ref window) = self.window {
            if window.set_display_mode(width, height, fullscreen) {
                self.display.set_display_mode(width, height, 32, !fullscreen);
                return true;
            }
        }
        false
    }

    /// Toggle fullscreen
    pub fn toggle_fullscreen(&mut self) {
        if let Some(ref window) = self.window {
            window.toggle_fullscreen();
            let windowed = !window.state().fullscreen;
            self.display.set_windowed(windowed);
        }
    }

    /// Get available display modes
    pub fn get_available_modes(&self) -> Vec<DisplayModeInfo> {
        if let Some(ref window) = self.window {
            window.available_display_modes().to_vec()
        } else {
            Vec::new()
        }
    }

    /// Get current display mode
    pub fn get_current_mode(&self) -> Option<DisplayModeInfo> {
        self.window.as_ref().map(|w| w.current_display_mode())
    }

    /// Set FPS limit
    pub fn set_fps_limit(&mut self, fps: u32) {
        self.target_fps = fps;
        if let Some(ref window) = self.window {
            window.set_fps_limit(fps);
        }
    }

    /// Get current FPS
    pub fn get_current_fps(&self) -> f32 {
        self.window
            .as_ref()
            .map(|w| w.current_fps())
            .unwrap_or(0.0)
    }

    /// Get delta time
    pub fn get_delta_time(&self) -> f32 {
        self.window
            .as_ref()
            .map(|w| w.delta_time())
            .unwrap_or(0.0)
    }

    /// Process a window event
    pub fn process_event(&mut self, event: &WindowEvent) {
        if let Some(ref window) = self.window {
            if let Some(input_event) = window.process_event(event) {
                self.handle_input_event(input_event);
            }
        }
    }

    /// Handle an input event by routing to registered handlers
    fn handle_input_event(&mut self, event: InputEvent) {
        match event {
            InputEvent::KeyPressed { key_code, scancode } => {
                for handler in &self.event_handlers {
                    handler.write().handle_key_pressed(key_code, scancode);
                }
            }

            InputEvent::KeyReleased { key_code, scancode } => {
                for handler in &self.event_handlers {
                    handler.write().handle_key_released(key_code, scancode);
                }
            }

            InputEvent::MouseMoved { x, y } => {
                for handler in &self.event_handlers {
                    handler.write().handle_mouse_moved(x, y);
                }
            }

            InputEvent::MouseButtonPressed { button, x, y } => {
                for handler in &self.event_handlers {
                    handler.write().handle_mouse_button_pressed(button, x, y);
                }
            }

            InputEvent::MouseButtonReleased { button, x, y } => {
                for handler in &self.event_handlers {
                    handler.write().handle_mouse_button_released(button, x, y);
                }
            }

            InputEvent::MouseWheel { delta_x, delta_y } => {
                for handler in &self.event_handlers {
                    handler.write().handle_mouse_wheel(delta_x, delta_y);
                }
            }

            InputEvent::WindowResized { width, height } => {
                self.display.set_width(width);
                self.display.set_height(height);

                for handler in &self.event_handlers {
                    handler.write().handle_window_resized(width, height);
                }
            }

            InputEvent::WindowFocusChanged { focused } => {
                for handler in &self.event_handlers {
                    handler.write().handle_window_focus_changed(focused);
                }
            }

            InputEvent::WindowCloseRequested => {
                self.running = false;
            }
        }
    }

    /// Update the display system (call once per frame)
    pub fn update(&mut self) {
        // Update frame timing
        if let Some(ref window) = self.window {
            window.update_timing();
        }

        // Update the display
        self.display.update();
    }

    /// Render the display (call once per frame after update)
    pub fn render(&self) {
        self.display.draw();
    }

    /// Request a redraw
    pub fn request_redraw(&self) {
        if let Some(ref window) = self.window {
            window.request_redraw();
        }
    }

    /// Check if the display system is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Shutdown the display system
    pub fn shutdown(&mut self) {
        self.running = false;
        self.display.reset();
    }
}

impl Default for DisplaySystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Main event loop runner for the display system
pub struct DisplayEventLoop {
    display_system: Arc<RwLock<DisplaySystem>>,
}

impl DisplayEventLoop {
    /// Create a new event loop runner
    pub fn new(display_system: Arc<RwLock<DisplaySystem>>) -> Self {
        Self { display_system }
    }

    /// Run the event loop
    pub fn run<F>(self, event_loop: EventLoop<()>, mut render_callback: F)
    where
        F: FnMut(&mut DisplaySystem) + 'static,
    {
        event_loop.run(move |event, _, control_flow| {
            match event {
                Event::WindowEvent { event, .. } => {
                    let mut system = self.display_system.write();
                    system.process_event(&event);

                    if !system.is_running() {
                        *control_flow = ControlFlow::Exit;
                    }
                }

                Event::MainEventsCleared => {
                    let mut system = self.display_system.write();

                    // Update the display system
                    system.update();

                    // Call user render callback
                    render_callback(&mut system);

                    // Render
                    system.render();

                    // Request redraw for next frame
                    system.request_redraw();
                }

                Event::RedrawRequested(_) => {
                    // Actual rendering happens in MainEventsCleared
                }

                _ => {}
            }

            *control_flow = ControlFlow::Poll;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestEventHandler {
        key_pressed_count: usize,
        mouse_moved_count: usize,
    }

    impl TestEventHandler {
        fn new() -> Self {
            Self {
                key_pressed_count: 0,
                mouse_moved_count: 0,
            }
        }
    }

    impl InputEventHandler for TestEventHandler {
        fn handle_key_pressed(&mut self, _key_code: u32, _scancode: u32) {
            self.key_pressed_count += 1;
        }

        fn handle_key_released(&mut self, _key_code: u32, _scancode: u32) {}

        fn handle_mouse_moved(&mut self, _x: f64, _y: f64) {
            self.mouse_moved_count += 1;
        }

        fn handle_mouse_button_pressed(&mut self, _button: MouseButtonType, _x: f64, _y: f64) {}
        fn handle_mouse_button_released(&mut self, _button: MouseButtonType, _x: f64, _y: f64) {}
        fn handle_mouse_wheel(&mut self, _delta_x: f32, _delta_y: f32) {}
        fn handle_window_resized(&mut self, _width: u32, _height: u32) {}
        fn handle_window_focus_changed(&mut self, _focused: bool) {}
    }

    #[test]
    fn test_display_system_creation() {
        let system = DisplaySystem::new();
        assert!(system.window.is_none());
        assert!(!system.is_running());
    }

    #[test]
    fn test_display_system_event_handlers() {
        let mut system = DisplaySystem::new();
        let handler = Arc::new(RwLock::new(TestEventHandler::new()));

        system.add_event_handler(handler.clone());

        // Simulate key press
        system.handle_input_event(InputEvent::KeyPressed {
            key_code: 65, // 'A'
            scancode: 30,
        });

        assert_eq!(handler.read().key_pressed_count, 1);

        // Simulate mouse move
        system.handle_input_event(InputEvent::MouseMoved {
            x: 100.0,
            y: 200.0,
        });

        assert_eq!(handler.read().mouse_moved_count, 1);
    }

    #[test]
    fn test_display_system_fps_limit() {
        let mut system = DisplaySystem::new();
        system.set_fps_limit(120);
        assert_eq!(system.target_fps, 120);
    }

    #[test]
    fn test_display_system_shutdown() {
        let mut system = DisplaySystem::new();
        system.running = true;

        system.shutdown();

        assert!(!system.is_running());
    }
}
