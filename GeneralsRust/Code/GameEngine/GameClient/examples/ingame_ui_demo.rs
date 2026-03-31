//! # In-Game UI Demo
//!
//! Demonstrates the complete in-game UI system including:
//! - Unit selection with drag boxes
//! - Command panel with buttons and hotkeys
//! - Building placement preview
//! - Minimap with unit icons and camera indicator
//! - Resource display (credits, power)
//!
//! Run with: cargo run --example ingame_ui_demo

#[cfg(not(feature = "internal"))]
fn main() {}

#[cfg(feature = "internal")]
mod internal {
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};

use glam::Vec2;
use winit::{
    event::{Event, WindowEvent, ElementState, KeyEvent, MouseButton},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
    keyboard::{Key, NamedKey},
};

use game_client::gui::{
    IntegratedUISystem, IntegratedUISystemBuilder, UICommand,
};
use game_client::input::mouse::MouseState;
use game_client::input::keyboard::KeyboardState;

const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 720;

pub fn run() {
    env_logger::init();

    log::info!("Starting In-Game UI Demo");

    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("C&C Generals - In-Game UI Demo")
        .with_inner_size(winit::dpi::PhysicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT))
        .build(&event_loop)
        .unwrap();

    // Initialize wgpu
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let surface = instance.create_surface(&window).unwrap();

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .unwrap();

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("GPU Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: Default::default(),
        },
        None,
    ))
    .unwrap();

    let device = Arc::new(device);
    let queue = Arc::new(queue);

    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(surface_caps.formats[0]);

    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: WINDOW_WIDTH,
        height: WINDOW_HEIGHT,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };

    surface.configure(&device, &config);

    // Create UI system
    let mut ui_system = IntegratedUISystemBuilder::new()
        .with_device(device.clone())
        .with_queue(queue.clone())
        .with_format(surface_format)
        .with_screen_size(WINDOW_WIDTH, WINDOW_HEIGHT)
        .build()
        .unwrap();

    ui_system.init().unwrap();

    // Set up demo state
    setup_demo_state(&mut ui_system);

    // Input state
    let mut mouse_state = MouseState::new();
    let mut keyboard_state = KeyboardState::new();

    // Timing
    let mut last_update = Instant::now();
    let mut frame_count = 0u64;
    let mut fps_timer = Instant::now();

    log::info!("UI System initialized, entering main loop");

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    log::info!("Close requested, shutting down");
                    elwt.exit();
                }
                WindowEvent::Resized(new_size) => {
                    config.width = new_size.width;
                    config.height = new_size.height;
                    surface.configure(&device, &config);
                    ui_system.resize(new_size.width, new_size.height);
                    log::info!("Resized to {}x{}", new_size.width, new_size.height);
                }
                WindowEvent::KeyboardInput { event: key_event, .. } => {
                    keyboard_state.process_event(&key_event);

                    // Handle demo hotkeys
                    if key_event.state == ElementState::Pressed {
                        match key_event.logical_key {
                            Key::Named(NamedKey::Escape) => {
                                log::info!("ESC pressed, exiting");
                                elwt.exit();
                            }
                            Key::Character(ref c) => {
                                let ch = c.chars().next().unwrap().to_ascii_uppercase();
                                // Handle number keys for groups
                                if ch.is_ascii_digit() && ch != '0' {
                                    let group = ch.to_digit(10).unwrap() as usize - 1;
                                    if keyboard_state.is_ctrl_pressed() {
                                        ui_system.set_selection_group(group);
                                        log::info!("Created selection group {}", group);
                                    } else {
                                        ui_system.recall_selection_group(group);
                                        log::info!("Recalled selection group {}", group);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    mouse_state.process_button_event(button, state);
                }
                WindowEvent::CursorMoved { position, .. } => {
                    mouse_state.process_move_event(position.x as f32, position.y as f32);
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    mouse_state.process_scroll_event(delta);
                }
                WindowEvent::RedrawRequested => {
                    // Update timing
                    let now = Instant::now();
                    let delta_time = now.duration_since(last_update);
                    last_update = now;

                    // Update FPS counter
                    frame_count += 1;
                    if fps_timer.elapsed() >= Duration::from_secs(1) {
                        log::info!("FPS: {}", frame_count);
                        frame_count = 0;
                        fps_timer = Instant::now();
                    }

                    // Handle input
                    if let Err(e) = ui_system.handle_input(&mouse_state, &keyboard_state) {
                        log::error!("Input handling error: {}", e);
                    }

                    // Process UI commands
                    let commands = ui_system.get_commands();
                    for command in commands {
                        match command {
                            UICommand::Build(name) => {
                                log::info!("Build command: {}", name);
                                // Start building placement
                                ui_system.start_building_placement(name, 3.0, 3.0);
                            }
                            UICommand::UnitCommand(name) => {
                                log::info!("Unit command: {}", name);
                            }
                            UICommand::SpecialPower(name) => {
                                log::info!("Special power: {}", name);
                            }
                            UICommand::Cancel => {
                                log::info!("Cancel command");
                                ui_system.cancel_building_placement();
                            }
                            _ => {}
                        }
                    }

                    // Update UI
                    if let Err(e) = ui_system.update(delta_time) {
                        log::error!("UI update error: {}", e);
                    }

                    // Update demo state (simulate game state changes)
                    update_demo_state(&mut ui_system, delta_time);

                    // Render
                    let frame = surface.get_current_texture().unwrap();
                    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

                    if let Err(e) = ui_system.render(&view) {
                        log::error!("Rendering error: {}", e);
                    }

                    frame.present();

                    // Update input state for next frame
                    mouse_state.end_frame();
                    keyboard_state.end_frame();
                }
                _ => {}
            },
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        }
    }).unwrap();
}
} // mod internal

#[cfg(feature = "internal")]
fn main() {
    internal::run();
}

/// Set up initial demo state
fn setup_demo_state(ui_system: &mut IntegratedUISystem) {
    // Set minimap bounds (1000x1000 world)
    ui_system.set_minimap_bounds(0.0, 0.0, 1000.0, 1000.0);

    // Set initial resources
    ui_system.update_resources(10000, 100, 50);

    // Add some demo units to minimap
    ui_system.update_minimap_unit(1, 100.0, 100.0, [0.0, 0.5, 1.0, 1.0]); // Blue
    ui_system.update_minimap_unit(2, 200.0, 150.0, [0.0, 0.5, 1.0, 1.0]); // Blue
    ui_system.update_minimap_unit(3, 300.0, 200.0, [0.0, 0.5, 1.0, 1.0]); // Blue

    ui_system.update_minimap_unit(10, 800.0, 800.0, [1.0, 0.0, 0.0, 1.0]); // Red enemy
    ui_system.update_minimap_unit(11, 850.0, 850.0, [1.0, 0.0, 0.0, 1.0]); // Red enemy

    // Set camera position
    ui_system.update_camera(500.0, 0.0, 500.0, 800.0, 600.0);

    log::info!("Demo state initialized");
}

/// Update demo state to simulate game changes
fn update_demo_state(ui_system: &mut IntegratedUISystem, delta_time: Duration) {
    // Simulate resource accumulation
    static RESOURCES_TIMER: std::sync::Mutex<f32> = std::sync::Mutex::new(0.0);
    static CURRENT_CREDITS: AtomicI32 = AtomicI32::new(10000);

    {
        let mut timer = RESOURCES_TIMER.lock().unwrap();
        *timer += delta_time.as_secs_f32();

        if *timer >= 1.0 {
            *timer = 0.0;

            // Add some credits periodically
            let credits = CURRENT_CREDITS.fetch_add(100, Ordering::Relaxed) + 100;
            ui_system.update_resources(credits, 100, 50);
        }
    }
}
