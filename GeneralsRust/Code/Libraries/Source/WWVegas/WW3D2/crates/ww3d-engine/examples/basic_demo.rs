//! Basic demo showing the WW3D engine with integrated subsystems
//!
//! This example demonstrates:
//! - Engine initialization
//! - Subsystem registration
//! - Input handling
//! - Main game loop with update/render phases
//! - Frame timing and FPS display
//! - Screenshot capture

use std::time::Instant;
use ww3d_engine::*;

/// Custom game subsystem that tracks game state
struct GameSubsystem {
    time_alive: f32,
    frame_count: u64,
}

impl GameSubsystem {
    fn new() -> Self {
        Self {
            time_alive: 0.0,
            frame_count: 0,
        }
    }
}

impl Subsystem for GameSubsystem {
    fn update(&mut self, timing: &FrameTiming) {
        self.time_alive += timing.delta_seconds();
        self.frame_count = timing.frame_number;

        // Log status every second
        if self.frame_count.is_multiple_of(60) {
            println!(
                "Game alive for {:.2}s, Frame: {}, FPS: {:.1}",
                self.time_alive, self.frame_count, timing.fps
            );
        }
    }

    fn name(&self) -> &str {
        "GameSubsystem"
    }
}

/// Simple input handler that responds to keyboard events
struct DemoInputHandler;

impl InputHandler for DemoInputHandler {
    fn handle_input(&mut self, event: &InputEvent) {
        match event {
            InputEvent::KeyPressed { key } => {
                println!("Key pressed: {}", key);

                // Take screenshot on 'S' key
                if key == "S" || key == "s" {
                    let screenshot_path =
                        format!("screenshot_{}.png", Instant::now().elapsed().as_millis());
                    if let Err(e) = make_screenshot(&screenshot_path) {
                        eprintln!("Failed to queue screenshot: {:?}", e);
                    } else {
                        println!("Screenshot queued: {}", screenshot_path);
                    }
                }
            }
            InputEvent::KeyReleased { key } => {
                println!("Key released: {}", key);
            }
            InputEvent::MousePressed { button, x, y } => {
                println!("Mouse button {} pressed at ({}, {})", button, x, y);
            }
            InputEvent::MouseReleased { button, x, y } => {
                println!("Mouse button {} released at ({}, {})", button, x, y);
            }
            InputEvent::MouseMoved { x, y } => {
                // Too verbose to print every movement
                let _ = (x, y);
            }
            InputEvent::MouseScrolled { delta } => {
                println!("Mouse scrolled: {}", delta);
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== WW3D Engine Demo ===");
    println!("Initializing headless engine...");

    // Configure engine
    let config = EngineConfig {
        width: 1280,
        height: 720,
        enable_depth: true,
        ..Default::default()
    };

    // Initialize headless engine (can also use init_with_window for windowed mode)
    init_headless_blocking(config)?;

    println!("Engine initialized successfully!");

    // Print adapter info
    let adapter = adapter_info()?;
    println!("GPU Adapter: {} ({:?})", adapter.name, adapter.backend);

    // Register subsystems
    println!("\nRegistering subsystems...");

    let game_subsystem = Box::new(GameSubsystem::new());
    register_subsystem(game_subsystem)?;
    println!("- Game subsystem registered");

    // Set input handler
    let input_handler = Box::new(DemoInputHandler);
    set_input_handler(input_handler)?;
    println!("- Input handler registered");

    println!("\nStarting main loop (will run 300 frames)...");
    println!("Press 'S' to take a screenshot (simulated)");

    // Main game loop
    const MAX_FRAMES: usize = 300;
    for frame_num in 0..MAX_FRAMES {
        // Simulate some input events periodically
        if frame_num == 60 {
            queue_input(InputEvent::KeyPressed {
                key: "A".to_string(),
            })?;
        }

        if frame_num == 120 {
            queue_input(InputEvent::MousePressed {
                button: 0,
                x: 640.0,
                y: 360.0,
            })?;
        }

        if frame_num == 180 {
            queue_input(InputEvent::KeyPressed {
                key: "S".to_string(), // Trigger screenshot
            })?;
        }

        // Update all subsystems
        update()?;

        // Begin rendering
        let mut frame = begin_render()?;

        // Clear the screen to a blue color
        {
            let color_view = frame.color_view_arc();
            let depth_view = frame.depth_view_arc();
            let encoder = frame.encoder();
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view.as_ref(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.4,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: depth_view.as_ref().map(|view| {
                    wgpu::RenderPassDepthStencilAttachment {
                        view: view.as_ref(),
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // End rendering (presents the frame)
        end_render(frame)?;

        // Display progress
        if frame_num % 60 == 0 {
            let timing = timing()?;
            println!(
                "Frame {}/{}, FPS: {:.1}, Delta: {:.2}ms",
                frame_num,
                MAX_FRAMES,
                timing.fps,
                timing.delta_time.as_secs_f32() * 1000.0
            );
        }
    }

    // Get final stats
    println!("\n=== Final Statistics ===");
    let final_timing = timing()?;
    println!("Total frames: {}", final_timing.frame_number);
    println!("Total time: {:.2}s", final_timing.total_seconds());
    println!("Final FPS: {:.1}", final_timing.fps);
    println!("Surface size: {:?}", surface_size()?);

    // Shutdown
    println!("\nShutting down...");
    shutdown()?;

    println!("Demo completed successfully!");
    Ok(())
}
