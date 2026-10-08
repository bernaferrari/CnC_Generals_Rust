//! # Display System Demo
//!
//! Demonstrates the display system for Command & Conquer Generals Zero Hour.
//! Shows resolution management, display modes, and rendering setup using wgpu.
//!
//! Run with: `cargo run --example display_demo`

use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    error::OsError,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

const WINDOW_TITLE: &str = "C&C Generals Zero Hour - Display Demo";
const DEFAULT_WIDTH: u32 = 1280;
const DEFAULT_HEIGHT: u32 = 720;

/// Display mode configuration matching the original game options
#[derive(Debug, Clone)]
struct DisplayMode {
    width: u32,
    height: u32,
    refresh_rate: u32,
    bit_depth: u32,
    windowed: bool,
}

impl DisplayMode {
    fn new(width: u32, height: u32, windowed: bool) -> Self {
        Self {
            width,
            height,
            refresh_rate: 60,
            bit_depth: 32,
            windowed,
        }
    }

    fn aspect_ratio(&self) -> f32 {
        self.width as f32 / self.height as f32
    }

    fn is_widescreen(&self) -> bool {
        (self.aspect_ratio() - 16.0 / 9.0).abs() < 0.01
            || (self.aspect_ratio() - 16.0 / 10.0).abs() < 0.01
    }
}

impl std::fmt::Display for DisplayMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}x{} @ {}Hz, {}-bit, {}",
            self.width,
            self.height,
            self.refresh_rate,
            self.bit_depth,
            if self.windowed {
                "windowed"
            } else {
                "fullscreen"
            }
        )
    }
}

/// Simulated display settings manager
struct DisplaySettings {
    current_mode: DisplayMode,
    available_modes: Vec<DisplayMode>,
    vsync_enabled: bool,
    gamma: f32,
    brightness: f32,
}

impl DisplaySettings {
    fn new() -> Self {
        let available_modes = vec![
            DisplayMode::new(800, 600, true),
            DisplayMode::new(1024, 768, true),
            DisplayMode::new(1280, 720, true),
            DisplayMode::new(1280, 1024, true),
            DisplayMode::new(1600, 900, true),
            DisplayMode::new(1920, 1080, true),
            DisplayMode::new(2560, 1440, true),
        ];

        Self {
            current_mode: DisplayMode::new(DEFAULT_WIDTH, DEFAULT_HEIGHT, true),
            available_modes,
            vsync_enabled: true,
            gamma: 1.0,
            brightness: 0.5,
        }
    }

    fn enumerate_modes(&self) {
        println!("\n=== Available Display Modes ===");
        for (i, mode) in self.available_modes.iter().enumerate() {
            let marker = if mode.width == self.current_mode.width
                && mode.height == self.current_mode.height
            {
                " [CURRENT]"
            } else {
                ""
            };
            println!("  [{}] {}{}", i + 1, mode, marker);
        }
    }

    fn set_mode(&mut self, width: u32, height: u32, windowed: bool) -> Result<(), String> {
        let mode = DisplayMode::new(width, height, windowed);
        println!(
            "Setting display mode: {}x{}, {}",
            width,
            height,
            if windowed { "windowed" } else { "fullscreen" }
        );
        self.current_mode = mode;
        Ok(())
    }

    fn set_gamma(&mut self, gamma: f32) {
        self.gamma = gamma.clamp(0.5, 2.0);
        println!("Gamma set to {:.2}", self.gamma);
    }

    fn set_brightness(&mut self, brightness: f32) {
        self.brightness = brightness.clamp(0.0, 1.0);
        println!("Brightness set to {:.2}", self.brightness);
    }
}

/// Main display demo application
struct DisplayDemo {
    settings: DisplaySettings,
    frame_count: u64,
    start_time: Instant,
    running: bool,
}

impl DisplayDemo {
    fn new() -> Self {
        Self {
            settings: DisplaySettings::new(),
            frame_count: 0,
            start_time: Instant::now(),
            running: true,
        }
    }

    fn run_demo(&mut self) {
        println!("=== Command & Conquer Generals Zero Hour - Display System Demo ===\n");

        // Display current configuration
        self.show_current_config();

        // Enumerate available modes
        self.settings.enumerate_modes();

        // Demonstrate mode changes
        self.demo_mode_changes();

        // Demonstrate gamma/brightness
        self.demo_gamma_brightness();

        // Simulate render loop
        self.simulate_render_loop();

        println!("\n=== Display Demo Complete ===");
    }

    fn show_current_config(&self) {
        println!("Current Display Configuration:");
        println!(
            "  Resolution: {}x{}",
            self.settings.current_mode.width, self.settings.current_mode.height
        );
        println!("  Bit Depth: {}", self.settings.current_mode.bit_depth);
        println!(
            "  Refresh Rate: {}Hz",
            self.settings.current_mode.refresh_rate
        );
        println!("  Windowed: {}", self.settings.current_mode.windowed);
        println!("  VSync: {}", self.settings.vsync_enabled);
        println!(
            "  Aspect Ratio: {:.3}",
            self.settings.current_mode.aspect_ratio()
        );
        println!(
            "  Widescreen: {}",
            self.settings.current_mode.is_widescreen()
        );
        println!("  Gamma: {:.2}", self.settings.gamma);
        println!("  Brightness: {:.2}", self.settings.brightness);
    }

    fn demo_mode_changes(&mut self) {
        println!("\n=== Display Mode Changes ===");

        // Try different resolutions
        let test_modes = [(1920, 1080, false), (1280, 720, true), (800, 600, true)];

        for (w, h, windowed) in test_modes {
            match self.settings.set_mode(w, h, windowed) {
                Ok(_) => println!("  Mode change successful"),
                Err(e) => println!("  Mode change failed: {}", e),
            }
        }
    }

    fn demo_gamma_brightness(&mut self) {
        println!("\n=== Gamma and Brightness ===");
        self.settings.set_gamma(1.2);
        self.settings.set_brightness(0.7);
        self.settings.set_gamma(1.0);
        self.settings.set_brightness(0.5);
    }

    fn simulate_render_loop(&mut self) {
        println!("\n=== Simulated Render Loop ===");
        println!("Running 5 simulated frames...\n");

        for i in 0..5 {
            self.frame_count += 1;
            let elapsed = self.start_time.elapsed();

            // Simulate frame work
            self.begin_frame();
            self.update_display();
            self.end_frame();

            println!(
                "  Frame {}: {:.3}s elapsed, FPS: {:.1}",
                i + 1,
                elapsed.as_secs_f32(),
                self.frame_count as f32 / elapsed.as_secs_f32()
            );

            // Simulate frame time
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }

    fn begin_frame(&self) {
        // Frame begin - would clear buffers, set render targets
    }

    fn update_display(&self) {
        // Update display state - would render scene, UI, etc.
    }

    fn end_frame(&self) {
        // Frame end - would present back buffer, swap chains
    }
}

/// Owns the demo window for Winit's application lifecycle.
struct WindowDemo {
    window: Option<Window>,
    creation_error: Option<OsError>,
    start: Instant,
    frame: u64,
}

impl WindowDemo {
    fn new() -> Self {
        Self {
            window: None,
            creation_error: None,
            start: Instant::now(),
            frame: 0,
        }
    }
}

impl ApplicationHandler for WindowDemo {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() || self.creation_error.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(WINDOW_TITLE)
            .with_inner_size(winit::dpi::PhysicalSize::new(DEFAULT_WIDTH, DEFAULT_HEIGHT));
        match event_loop.create_window(attributes) {
            Ok(window) => {
                println!("Window created: {}x{}", DEFAULT_WIDTH, DEFAULT_HEIGHT);
                println!("Press Escape or close window to exit.");
                self.start = Instant::now();
                self.window = Some(window);
            }
            Err(error) => {
                self.creation_error = Some(error);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self
            .window
            .as_ref()
            .filter(|window| window.id() == window_id)
        else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => {
                println!("Window closed after {} frames", self.frame);
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == winit::event::ElementState::Pressed
                    && event.logical_key
                        == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape)
                {
                    println!("Escape pressed, exiting.");
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                self.frame += 1;
                if self.frame.is_multiple_of(60) {
                    let fps = self.frame as f32 / self.start.elapsed().as_secs_f32();
                    println!("FPS: {:.1}, Frame: {}", fps, self.frame);
                }
                window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("C&C Generals Zero Hour - Display System Demo");
    println!("=============================================\n");

    // Run the headless demo first
    let mut demo = DisplayDemo::new();
    demo.run_demo();

    // Now attempt to create an actual window for visual demo
    println!("\n=== Creating Window for Visual Demo ===");

    let event_loop = EventLoop::new()?;
    let mut app = WindowDemo::new();
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.creation_error {
        return Err(error.into());
    }
    Ok(())
}
