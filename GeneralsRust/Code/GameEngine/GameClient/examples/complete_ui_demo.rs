//! Complete UI System Demo
//!
//! This example demonstrates how to use the complete UI system that matches
//! the original Command & Conquer Generals interface exactly.

use std::sync::{Arc, Mutex};
use wgpu::{Device, Queue, Surface, SurfaceConfiguration, TextureFormat};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use ww3d_gpu::present_surface_texture;

use game_client_rust::core::subsystems::{
    CommandLogEntry, InGameUISubsystem, InGameUiHandle, SelectionEvent,
};
use game_client_rust::gui::{
    ButtonStyle, CompleteUISystem, EnhancedPushButton, GadgetState, UIPerformanceStats,
    UISystemConfig,
};
use game_client_rust::helpers::register_in_game_ui_backend;

/// Main demo application
struct UIDemo {
    // WGPU resources
    device: Arc<Device>,
    queue: Arc<Queue>,
    surface: Surface,
    surface_config: SurfaceConfiguration,

    // UI system
    ui_system: CompleteUISystem,

    // Demo state
    demo_mode: DemoMode,
    show_performance_overlay: bool,
    last_performance_stats: UIPerformanceStats,
    show_beacon_overlay: bool,
    last_beacon_lines: Vec<String>,
    last_selection_events: Vec<SelectionEvent>,
    last_command_history: Vec<CommandLogEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DemoMode {
    MainMenu,
    InGame,
    Dialog,
}

impl UIDemo {
    async fn new(window: &winit::window::Window) -> Result<Self, Box<dyn std::error::Error>> {
        let size = window.inner_size();

        // Create WGPU instance
        let mut backend_options = wgpu::BackendOptions::default();
        backend_options.dx12.shader_compiler = Default::default();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: Default::default(),
            backend_options,
        });

        // Create surface
        let surface = unsafe { instance.create_surface(window) }?;

        // Create adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or("Failed to create adapter")?;

        // Create device and queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    label: None,
                },
                None,
            )
            .await?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);

        // Create UI system configuration
        let ui_config = UISystemConfig {
            screen_width: size.width,
            screen_height: size.height,
            texture_format: surface_format,
            enable_msaa: true,
            msaa_samples: 4,
            enable_vsync: true,
            ui_scale: 1.0,
            default_font: "Arial".to_string(),
            font_size: 14.0,
            asset_root: "Data".to_string(),
            font_path: "Data/Fonts".to_string(),
            image_path: "Data/Images".to_string(),
            layout_path: "Data/Layouts".to_string(),
            max_draw_calls_per_frame: 1000,
            enable_ui_batching: true,
            enable_texture_atlas: true,
        };

        // Create and initialize UI system
        let mut ui_system = CompleteUISystem::new(ui_config);
        ui_system.initialize(device.clone(), queue.clone())?;

        // Install an in-game UI backend so the control bar can react to
        // commands issued through TheInGameUI facade during the demo.
        let mut in_game_ui = InGameUISubsystem::default();
        in_game_ui.init()?;
        let in_game_ui = Arc::new(Mutex::new(in_game_ui));
        register_in_game_ui_backend(Arc::new(InGameUiHandle::new(in_game_ui.clone())));
        ui_system.set_in_game_ui(in_game_ui);

        // Show main menu
        if let Err(e) = ui_system.show_main_menu() {
            log::warn!("Failed to show main menu: {}", e);
        }

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            ui_system,
            demo_mode: DemoMode::MainMenu,
            show_performance_overlay: false,
            last_performance_stats: UIPerformanceStats {
                frame_time_ms: 0.0,
                update_time_ms: 0.0,
                render_time_ms: 0.0,
                frame_count: 0,
                draw_calls: 0,
                vertices_rendered: 0,
                triangles_rendered: 0,
                texture_switches: 0,
            },
            show_beacon_overlay: false,
            last_beacon_lines: Vec::new(),
            last_selection_events: Vec::new(),
            last_command_history: Vec::new(),
        })
    }

    fn handle_input(&mut self, event: &WindowEvent) -> bool {
        // Handle UI events first
        match self.ui_system.handle_window_event(event) {
            Ok(handled) => {
                if handled {
                    return true;
                }
            }
            Err(e) => {
                log::error!("UI event handling error: {}", e);
            }
        }

        // Handle demo-specific input
        match event {
            WindowEvent::KeyboardInput { input, .. } => {
                if input.state == winit::event::ElementState::Pressed {
                    match input.virtual_keycode {
                        Some(winit::event::VirtualKeyCode::F1) => {
                            self.show_performance_overlay = !self.show_performance_overlay;
                            return true;
                        }
                        Some(winit::event::VirtualKeyCode::F2) => {
                            self.demo_mode = match self.demo_mode {
                                DemoMode::MainMenu => DemoMode::InGame,
                                DemoMode::InGame => DemoMode::Dialog,
                                DemoMode::Dialog => DemoMode::MainMenu,
                            };
                            self.switch_demo_mode();
                            return true;
                        }
                        Some(winit::event::VirtualKeyCode::F3) => {
                            self.show_beacon_overlay = !self.show_beacon_overlay;
                            if !self.show_beacon_overlay {
                                self.last_beacon_lines.clear();
                                self.last_selection_events.clear();
                                self.last_command_history.clear();
                            }
                            log::info!(
                                "Beacon overlay {}",
                                if self.show_beacon_overlay {
                                    "enabled"
                                } else {
                                    "disabled"
                                }
                            );
                            return true;
                        }
                        Some(winit::event::VirtualKeyCode::Escape) => {
                            if self.demo_mode != DemoMode::MainMenu {
                                self.demo_mode = DemoMode::MainMenu;
                                self.switch_demo_mode();
                                return true;
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        false
    }

    fn switch_demo_mode(&mut self) {
        match self.demo_mode {
            DemoMode::MainMenu => {
                if let Err(e) = self.ui_system.show_main_menu() {
                    log::error!("Failed to show main menu: {}", e);
                }
            }
            DemoMode::InGame => {
                if let Err(e) = self.ui_system.show_ingame_hud() {
                    log::error!("Failed to show in-game HUD: {}", e);
                }
            }
            DemoMode::Dialog => {
                if let Err(e) = self.ui_system.create_dialog(
                    "Test Dialog",
                    "This is a test dialog to demonstrate the UI system.",
                    &["OK", "Cancel"],
                ) {
                    log::error!("Failed to create dialog: {}", e);
                }
            }
        }

        log::info!("Switched to demo mode: {:?}", self.demo_mode);
    }

    fn update(&mut self, delta_time: f32) {
        // Update UI system
        if let Err(e) = self.ui_system.update(delta_time) {
            log::error!("UI update error: {}", e);
        }

        // Get performance stats
        self.last_performance_stats = self.ui_system.get_performance_stats();

        if self.show_beacon_overlay {
            self.last_beacon_lines = self
                .ui_system
                .beacon_panel_lines()
                .iter()
                .cloned()
                .collect();
            self.last_selection_events = self.ui_system.selection_history();
            self.last_command_history = self.ui_system.command_history();
        }

        // Demonstrate some UI interactions
        match self.demo_mode {
            DemoMode::InGame => {
                // Simulate unit selection
                if self.last_performance_stats.frame_count % 180 == 0 {
                    // Every 3 seconds at 60fps
                    if let Ok(mut control_bar) = self.ui_system.get_control_bar().lock() {
                        control_bar.on_drawable_selected("TestUnit");
                    }
                }
            }
            _ => {}
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // Get the next frame
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Render UI
        if let Err(e) = self.ui_system.render(&view) {
            log::error!("UI render error: {}", e);
        }

        // Render performance overlay if enabled
        if self.show_performance_overlay {
            self.render_performance_overlay(&view);
        }

        if self.show_beacon_overlay {
            self.render_beacon_overlay();
        }

        // Present the frame
        present_surface_texture(output);

        Ok(())
    }

    fn render_performance_overlay(&mut self, _view: &wgpu::TextureView) {
        // In a real implementation, this would render performance stats as UI text
        // For now, just log them occasionally
        if self.last_performance_stats.frame_count % 60 == 0 {
            log::info!("Performance Stats:");
            log::info!("  FPS: {:.1}", self.last_performance_stats.fps());
            log::info!(
                "  Frame Time: {:.2}ms",
                self.last_performance_stats.frame_time_ms
            );
            log::info!(
                "  Update Time: {:.2}ms",
                self.last_performance_stats.update_time_ms
            );
            log::info!(
                "  Render Time: {:.2}ms",
                self.last_performance_stats.render_time_ms
            );
            log::info!("  Draw Calls: {}", self.last_performance_stats.draw_calls);
            log::info!(
                "  Vertices: {}",
                self.last_performance_stats.vertices_rendered
            );
            log::info!(
                "  Triangles: {}",
                self.last_performance_stats.triangles_rendered
            );
        }
    }

    fn render_beacon_overlay(&self) {
        if self.last_performance_stats.frame_count % 120 != 0 {
            return;
        }

        log::info!("Beacon Overlay:");
        for line in &self.last_beacon_lines {
            log::info!("  {}", line);
        }

        if !self.last_selection_events.is_empty() {
            log::info!("Recent Selections:");
            for event in &self.last_selection_events {
                log::info!(
                    "  UL({},{}) -> LR({},{})",
                    event.upper_left.x,
                    event.upper_left.y,
                    event.lower_right.x,
                    event.lower_right.y
                );
            }
        }

        if !self.last_command_history.is_empty() {
            log::info!("Recent Commands:");
            for entry in &self.last_command_history {
                match entry {
                    CommandLogEntry::Move { position, queued } => log::info!(
                        "  Move to ({:.1}, {:.1}, {:.1}) queued={}",
                        position.x,
                        position.y,
                        position.z,
                        queued
                    ),
                    CommandLogEntry::ForceAttackGround { position } => log::info!(
                        "  Force attack ground ({:.1}, {:.1}, {:.1})",
                        position.x,
                        position.y,
                        position.z
                    ),
                    CommandLogEntry::Attack { target_id, queued } => {
                        log::info!("  Attack target {} queued={}", target_id, queued)
                    }
                    CommandLogEntry::Stop => log::info!("  Stop"),
                }
            }
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface_config.width = new_size.width;
            self.surface_config.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_config);

            // Update UI system resolution
            // Would need to add a resize method to CompleteUISystem
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    log::info!("Starting Complete UI System Demo");
    log::info!("Controls:");
    log::info!("  F1 - Toggle performance overlay");
    log::info!("  F2 - Switch demo mode (Main Menu -> In Game -> Dialog)");
    log::info!("  F3 - Toggle beacon/debug overlay");
    log::info!("  ESC - Return to main menu");

    // Create event loop and window
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Command & Conquer Generals - Complete UI System Demo")
        .with_inner_size(LogicalSize::new(1024, 768))
        .with_resizable(true)
        .build(&event_loop)?;

    // Create demo application
    let mut demo = UIDemo::new(&window).await?;

    // Main event loop
    let mut last_frame_time = std::time::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                if demo.handle_input(event) {
                    return; // Event was handled by UI
                }

                match event {
                    WindowEvent::CloseRequested => {
                        log::info!("Shutting down UI system...");
                        if let Err(e) = demo.ui_system.shutdown() {
                            log::error!("UI shutdown error: {}", e);
                        }
                        *control_flow = ControlFlow::Exit;
                    }
                    WindowEvent::Resized(physical_size) => {
                        demo.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        demo.resize(**new_inner_size);
                    }
                    _ => {}
                }
            }
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                // Calculate delta time
                let now = std::time::Instant::now();
                let delta_time = now.duration_since(last_frame_time).as_secs_f32();
                last_frame_time = now;

                // Update
                demo.update(delta_time);

                // Render
                match demo.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => demo.resize(window.inner_size()),
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => log::error!("Render error: {:?}", e),
                }
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_config_defaults() {
        let config = UISystemConfig::default();
        assert_eq!(config.screen_width, 1024);
        assert_eq!(config.screen_height, 768);
        assert_eq!(config.ui_scale, 1.0);
        assert!(config.enable_msaa);
        assert_eq!(config.msaa_samples, 4);
    }

    #[tokio::test]
    async fn test_ui_system_creation() {
        let config = UISystemConfig::default();
        let ui_system = CompleteUISystem::new(config);

        // System should be created but not initialized
        assert!(!ui_system.initialized);
    }
}
