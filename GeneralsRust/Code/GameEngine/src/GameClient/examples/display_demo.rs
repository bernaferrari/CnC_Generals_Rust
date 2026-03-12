// Example: Display System Demo
// Demonstrates cross-platform window management with winit

use game_client_rust::{
    DisplaySystem, DisplayEventLoop, WindowConfig, InputEventHandler, MouseButtonType,
};
use std::sync::Arc;
use parking_lot::RwLock;
use winit::event_loop::EventLoop;

/// Example input handler that logs events
struct DemoInputHandler {
    name: String,
}

impl DemoInputHandler {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl InputEventHandler for DemoInputHandler {
    fn handle_key_pressed(&mut self, key_code: u32, scancode: u32) {
        println!(
            "[{}] Key pressed: code={}, scan={}",
            self.name, key_code, scancode
        );
    }

    fn handle_key_released(&mut self, key_code: u32, scancode: u32) {
        println!(
            "[{}] Key released: code={}, scan={}",
            self.name, key_code, scancode
        );
    }

    fn handle_mouse_moved(&mut self, x: f64, y: f64) {
        // Too noisy, skip logging
    }

    fn handle_mouse_button_pressed(&mut self, button: MouseButtonType, x: f64, y: f64) {
        println!(
            "[{}] Mouse button pressed: {:?} at ({}, {})",
            self.name, button, x, y
        );
    }

    fn handle_mouse_button_released(&mut self, button: MouseButtonType, x: f64, y: f64) {
        println!(
            "[{}] Mouse button released: {:?} at ({}, {})",
            self.name, button, x, y
        );
    }

    fn handle_mouse_wheel(&mut self, delta_x: f32, delta_y: f32) {
        println!(
            "[{}] Mouse wheel: dx={}, dy={}",
            self.name, delta_x, delta_y
        );
    }

    fn handle_window_resized(&mut self, width: u32, height: u32) {
        println!("[{}] Window resized: {}x{}", self.name, width, height);
    }

    fn handle_window_focus_changed(&mut self, focused: bool) {
        println!("[{}] Window focus: {}", self.name, focused);
    }
}

fn main() {
    println!("=== Display System Demo ===");
    println!("Controls:");
    println!("  F - Toggle fullscreen");
    println!("  ESC - Exit");
    println!();

    // Create event loop
    let event_loop = EventLoop::new();

    // Configure window
    let config = WindowConfig {
        title: "C&C Generals Display Demo".to_string(),
        width: 1024,
        height: 768,
        fullscreen: false,
        vsync: true,
        resizable: true,
        decorated: true,
        maximized: false,
        visible: true,
    };

    // Create display system
    let mut display_system = DisplaySystem::new();

    // Initialize with window
    if let Err(e) = display_system.init(&event_loop, config) {
        eprintln!("Failed to initialize display system: {}", e);
        return;
    }

    // Add input event handler
    let handler = Arc::new(RwLock::new(DemoInputHandler::new("Demo")));
    display_system.add_event_handler(handler);

    // Set FPS limit
    display_system.set_fps_limit(60);

    // Print available display modes
    println!("Available display modes:");
    for (i, mode) in display_system.get_available_modes().iter().enumerate() {
        println!(
            "  [{}] {}x{} @ {}Hz ({}bit)",
            i, mode.width, mode.height, mode.refresh_rate, mode.bit_depth
        );
    }
    println!();

    // Get current mode
    if let Some(current) = display_system.get_current_mode() {
        println!(
            "Current mode: {}x{} @ {}Hz ({}bit)",
            current.width, current.height, current.refresh_rate, current.bit_depth
        );
    }
    println!();

    // Wrap display system in Arc for event loop
    let display_system = Arc::new(RwLock::new(display_system));

    // Create event loop runner
    let event_loop_runner = DisplayEventLoop::new(display_system.clone());

    // Frame counter for demo
    let mut frame_count = 0u64;
    let mut last_print = std::time::Instant::now();

    // Run the event loop
    event_loop_runner.run(event_loop, move |system| {
        frame_count += 1;

        // Print FPS every second
        let now = std::time::Instant::now();
        if now.duration_since(last_print).as_secs() >= 1 {
            let fps = system.get_current_fps();
            let delta = system.get_delta_time();

            println!(
                "Frame: {}, FPS: {:.1}, Delta: {:.3}ms",
                frame_count,
                fps,
                delta * 1000.0
            );

            last_print = now;
        }

        // Demo: Toggle fullscreen every 5 seconds
        // (commented out to avoid annoyance, but shows how it works)
        /*
        if frame_count % (60 * 5) == 0 {
            println!("Toggling fullscreen...");
            system.toggle_fullscreen();
        }
        */
    });
}
