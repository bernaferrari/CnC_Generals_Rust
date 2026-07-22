//! Unit Control Demo - Complete demonstration of the RTS unit control system
//!
//! This demo shows:
//! - Mouse raycasting for 3D unit selection
//! - Left click to select units
//! - Right click to command movement/attack
//! - Drag selection (box selection)
//! - Control groups (Ctrl+1-9 to assign, 1-9 to select)
//! - Unit highlighting and visual feedback
//! - Integration with game logic and rendering

use game_engine::common::frame_clock::{FrameClock, FrameTiming as ClockFrameTiming};
use generals_main::{
    game_logic::{GameLogic, GameMode, Team},
    RtsInputSystem, SelectionRenderer, UIRenderCommand, UnitInputHandler,
};
use glam::{Mat4, Vec2, Vec3};
use std::sync::Arc;
use std::sync::Mutex as AsyncMutex;
use std::time::Instant;
use winit::{
    event::{ElementState, Event, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowBuilder},
};
use ww3d_engine::FrameTiming;

/// Main demo application
struct UnitControlDemo {
    /// Window and event loop
    window: Arc<Window>,

    /// Core game systems
    game_logic: Arc<AsyncMutex<GameLogic>>,
    input_system: RtsInputSystem,
    unit_input_handler: UnitInputHandler,
    selection_renderer: SelectionRenderer,

    /// Rendering state
    camera_view_matrix: Mat4,
    camera_proj_matrix: Mat4,
    window_size: (f32, f32),

    /// Demo state
    running: bool,
    frame_count: u64,
    frame_clock: FrameClock,
}

impl UnitControlDemo {
    fn to_engine_timing(timing: ClockFrameTiming) -> FrameTiming {
        let sync_time = timing.total_time.as_millis().min(u32::MAX as u128) as u32;
        let delta_ms = timing.delta_time.as_millis().min(u32::MAX as u128) as u32;
        let fps = if timing.delta_time.is_zero() {
            0.0
        } else {
            1.0 / timing.delta_time.as_secs_f32()
        };

        FrameTiming {
            frame_number: timing.frame_number,
            delta_time: timing.delta_time,
            total_time: timing.total_time,
            fps,
            frame_start: Instant::now(),
            sync_time,
            previous_sync_time: sync_time.saturating_sub(delta_ms),
        }
    }

    /// Create new demo application
    pub async fn new(event_loop: &EventLoop<()>) -> anyhow::Result<Self> {
        // Create window
        let window = Arc::new(
            WindowBuilder::new()
                .with_title("C&C Generals Zero Hour - Unit Control Demo")
                .with_inner_size(winit::dpi::LogicalSize::new(1024.0, 768.0))
                .build(event_loop)?,
        );

        let window_size = {
            let size = window.inner_size();
            (size.width as f32, size.height as f32)
        };

        // Initialize game logic
        let game_logic = Arc::new(AsyncMutex::new(GameLogic::new()));

        // Set up the game world
        {
            let mut logic = game_logic.lock().unwrap_or_else(|e| e.into_inner());
            logic.start_new_game(GameMode::Skirmish);
            logic.load_map("demo_map");

            // Create additional test units for demo
            logic.create_object("USA_Ranger", Team::USA, Vec3::new(-50.0, 0.0, -50.0));
            logic.create_object("USA_Ranger", Team::USA, Vec3::new(-45.0, 0.0, -50.0));
            logic.create_object("USA_Ranger", Team::USA, Vec3::new(-40.0, 0.0, -45.0));
            logic.create_object("USA_Humvee", Team::USA, Vec3::new(-30.0, 0.0, -30.0));
            logic.create_object("USA_Humvee", Team::USA, Vec3::new(-20.0, 0.0, -25.0));

            // Create enemy units for attack testing
            logic.create_object("GLA_Soldier", Team::GLA, Vec3::new(50.0, 0.0, 50.0));
            logic.create_object("GLA_Soldier", Team::GLA, Vec3::new(55.0, 0.0, 50.0));
            logic.create_object("GLA_Technical", Team::GLA, Vec3::new(40.0, 0.0, 40.0));

            println!(
                "Demo world created with {} objects",
                logic.get_objects().len()
            );
        }

        // Initialize systems
        let input_system = RtsInputSystem::new();
        let unit_input_handler = UnitInputHandler::new(window_size, Team::USA, 0);
        let selection_renderer = SelectionRenderer::new();

        // Set up camera matrices
        let camera_view_matrix = Mat4::look_at_rh(
            Vec3::new(0.0, 50.0, 50.0), // Eye position
            Vec3::new(0.0, 0.0, 0.0),   // Look at
            Vec3::Y,                    // Up vector
        );

        let camera_proj_matrix = Mat4::perspective_rh(
            60.0_f32.to_radians(),         // FOV
            window_size.0 / window_size.1, // Aspect ratio
            1.0,                           // Near plane
            1000.0,                        // Far plane
        );

        Ok(Self {
            window,
            game_logic,
            input_system,
            unit_input_handler,
            selection_renderer,
            camera_view_matrix,
            camera_proj_matrix,
            window_size,
            running: true,
            frame_count: 0,
            frame_clock: FrameClock::new(),
        })
    }

    /// Main update loop
    pub async fn update_with_timing(&mut self, timing: &FrameTiming) -> anyhow::Result<()> {
        self.frame_count = timing.frame_number;
        let dt = timing.delta_seconds();

        // Update game logic
        {
            let mut logic = self.game_logic.lock().unwrap_or_else(|e| e.into_inner());
            logic.update_with_timing(timing);
        }

        // Process input
        self.unit_input_handler
            .process_input(&mut self.input_system, &self.game_logic)
            .await;

        // Update input system
        self.input_system.update_with_timing(timing);

        // Update selection renderer
        self.selection_renderer.update(dt);

        // Print debug info every 60 frames
        if self.frame_count % 60 == 0 {
            let selected_count = self.unit_input_handler.get_selected_objects().len();
            let hovered = self.unit_input_handler.get_hovered_object();

            println!(
                "Frame {}: {} units selected, hover: {:?}",
                self.frame_count, selected_count, hovered
            );
        }

        Ok(())
    }

    /// Advance the simulation using the internal frame clock.
    pub async fn tick(&mut self) -> anyhow::Result<()> {
        let timing = Self::to_engine_timing(self.frame_clock.next_frame());
        self.update_with_timing(&timing).await
    }

    /// Handle window events
    pub fn handle_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CloseRequested => {
                self.running = false;
                true
            }
            WindowEvent::Resized(new_size) => {
                self.window_size = (new_size.width as f32, new_size.height as f32);
                self.unit_input_handler
                    .set_window_size(self.window_size.0, self.window_size.1);

                // Update projection matrix
                self.camera_proj_matrix = Mat4::perspective_rh(
                    60.0_f32.to_radians(),
                    self.window_size.0 / self.window_size.1,
                    1.0,
                    1000.0,
                );

                true
            }
            WindowEvent::KeyboardInput { event, .. } => self.handle_keyboard_event(event),
            _ => {
                // Pass other events to input system
                self.input_system.handle_window_event(event, &self.window)
            }
        }
    }

    /// Handle keyboard events
    fn handle_keyboard_event(&mut self, event: &KeyEvent) -> bool {
        match event.state {
            ElementState::Pressed => match &event.logical_key {
                Key::Named(NamedKey::Escape) => {
                    self.running = false;
                    true
                }
                Key::Character(c) => match c.as_str() {
                    "q" => {
                        self.running = false;
                        true
                    }
                    "h" => {
                        self.print_help();
                        true
                    }
                    _ => false,
                },
                _ => false,
            },
            _ => false,
        }
    }

    /// Print help information
    fn print_help(&self) {
        println!("\n=== Unit Control Demo Help ===");
        println!("Mouse Controls:");
        println!("  Left Click          - Select unit");
        println!("  Shift+Left Click    - Add unit to selection");
        println!("  Right Click         - Move/Attack command");
        println!("  Drag Left Mouse     - Box selection");
        println!("  Mouse Wheel         - Camera zoom");
        println!();
        println!("Keyboard Controls:");
        println!("  WASD / Arrow Keys   - Move camera");
        println!("  Ctrl+A              - Select all units");
        println!("  Tab                 - Cycle through units");
        println!("  Delete              - Destroy selected units");
        println!("  Ctrl+1-9            - Assign control group");
        println!("  1-9                 - Select control group");
        println!("  F1                  - Toggle debug mode");
        println!("  H                   - Show this help");
        println!("  Q / ESC             - Quit demo");
        println!("================================\n");
    }

    /// Check if demo should continue running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get render commands for UI (would integrate with actual renderer)
    pub async fn get_render_commands(&self) -> anyhow::Result<Vec<UIRenderCommand>> {
        let unit_control = self.unit_input_handler.get_unit_control();

        // Presentation-only selection path: snapshot then draw (no live dual-read).
        let frame = {
            let logic = self.game_logic.lock().unwrap_or_else(|e| e.into_inner());
            generals_main::presentation_frame::PresentationFrame::build_from_logic(&logic, 0)
        };

        let commands = self.selection_renderer.render_selection(
            unit_control,
            &self.camera_view_matrix,
            &self.camera_proj_matrix,
            self.window_size,
            Some(&frame),
        );

        // Add selection box if dragging
        let mut all_commands = commands;
        if let Some((start, end)) = self.input_system.get_selection_box() {
            all_commands.push(self.selection_renderer.render_selection_box(start, end));
        }

        Ok(all_commands)
    }
}

/// Main demo entry point
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    env_logger::init();

    println!("Starting C&C Generals Unit Control Demo...");
    println!("Press H for help, Q or ESC to quit");

    // Create event loop
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    // Create demo application
    let mut demo = UnitControlDemo::new(&event_loop).await?;
    demo.print_help();

    // Run main loop
    event_loop.run(move |event, target| {
        match event {
            Event::WindowEvent { event, .. } => {
                if !demo.handle_event(&event) {
                    // Event not handled
                }
            }
            Event::AboutToWait => {
                // Update and render
                // Run update
                let runtime = tokio::runtime::Runtime::new().unwrap();
                if let Err(e) = runtime.block_on(demo.tick()) {
                    eprintln!("Update error: {}", e);
                    target.exit();
                }

                // Request redraw
                demo.window.request_redraw();
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                // In a real implementation, this would render to the screen
                // For this demo, we just show that we can generate render commands
                let runtime = tokio::runtime::Runtime::new().unwrap();
                if let Ok(commands) = runtime.block_on(demo.get_render_commands()) {
                    // In a real renderer, these commands would be executed
                    if !commands.is_empty() && demo.frame_count % 300 == 0 {
                        // Every 5 seconds
                        println!(
                            "Generated {} render commands for selection visualization",
                            commands.len()
                        );
                    }
                }
            }
            _ => {}
        }

        // Check if we should quit
        if !demo.is_running() {
            println!("Demo finished. Goodbye!");
            target.exit();
        }
    })?;

    Ok(())
}
