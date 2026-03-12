use std::time::Instant;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

#[cfg(feature = "w3d_support")]
use game_client_rust::w3d::{
    AntiAliasing, ShadowQuality, W3DConfig, W3DDevice, W3DDeviceSettings, W3DQuality,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("🚀 W3D Revolutionary Engine Demo");
    println!("═══════════════════════════════════════════════════");
    println!("🎮 Command & Conquer Generals Zero Hour");
    println!("⚡ The Most Advanced 3D Rendering System Ever Built!");
    println!("═══════════════════════════════════════════════════");

    #[cfg(not(feature = "w3d_support"))]
    {
        println!("❌ W3D support not enabled!");
        println!("   Please run with: cargo run --example w3d_demo --features w3d_support");
        return Ok(());
    }

    #[cfg(feature = "w3d_support")]
    {
        // Create event loop
        let event_loop = EventLoop::new()?;

        // Create revolutionary W3D device settings
        let settings = W3DDeviceSettings {
            width: 1920,
            height: 1080,
            windowed: true,
            vsync: true,
            config: W3DConfig {
                enable_pbr: true,
                enable_deferred_rendering: true,
                enable_gpu_culling: true,
                enable_compute_shaders: true,
                enable_tessellation: true,
                enable_geometry_shaders: true,
                enable_temporal_effects: true,
                shadow_quality: ShadowQuality::Ultra,
                anti_aliasing: AntiAliasing::TAA,
                quality_preset: W3DQuality::Extreme,
                max_lights: 2048,
                max_shadow_casters: 64,
                texture_memory_budget: 4096, // 4GB
                mesh_memory_budget: 2048,    // 2GB
                enable_multithreading: true,
                worker_threads: 8,
                enable_simd: true,
                enable_mesh_optimization: true,
                enable_texture_compression: true,
                enable_debug_overlays: true,
            },
            ..Default::default()
        };

        println!("🎯 Initializing W3D Device with EXTREME settings:");
        println!("   • PBR Rendering: ✓");
        println!("   • Deferred Pipeline: ✓");
        println!("   • GPU Culling: ✓");
        println!("   • Compute Shaders: ✓");
        println!("   • Tessellation: ✓");
        println!("   • Temporal Anti-Aliasing: ✓");
        println!("   • Ultra Shadow Quality: ✓");
        println!("   • Max Lights: 2048");
        println!("   • Texture Memory: 4GB");
        println!("   • Multi-threading: 8 cores");
        println!();

        // Create the revolutionary W3D device
        match W3DDevice::new(&event_loop, settings).await {
            Ok(mut device) => {
                println!("✅ W3D Revolutionary Device created successfully!");
                println!(
                    "🎨 Renderer: {}",
                    if device.is_feature_enabled("deferred_rendering") {
                        "Deferred + Forward+ Hybrid"
                    } else {
                        "Forward"
                    }
                );
                println!("🔮 Features enabled:");
                println!(
                    "   • PBR: {}",
                    if device.is_feature_enabled("pbr") {
                        "✓"
                    } else {
                        "✗"
                    }
                );
                println!(
                    "   • Deferred: {}",
                    if device.is_feature_enabled("deferred_rendering") {
                        "✓"
                    } else {
                        "✗"
                    }
                );
                println!(
                    "   • Compute: {}",
                    if device.is_feature_enabled("compute_shaders") {
                        "✓"
                    } else {
                        "✗"
                    }
                );
                println!(
                    "   • GPU Culling: {}",
                    if device.is_feature_enabled("gpu_culling") {
                        "✓"
                    } else {
                        "✗"
                    }
                );
                println!(
                    "   • Tessellation: {}",
                    if device.is_feature_enabled("tessellation") {
                        "✓"
                    } else {
                        "✗"
                    }
                );
                println!();

                // Enable debug mode for the demo
                device.set_debug_mode(true);

                // Load some example W3D assets (placeholder for now)
                println!("📦 Loading W3D assets...");
                match device.load_w3d_model("models/gdi_tank.w3d").await {
                    Ok(_) => println!("   ✅ GDI Tank loaded"),
                    Err(e) => println!("   ⚠️  Asset loading: {}", e),
                }
                match device.load_w3d_model("models/nod_scorpion.w3d").await {
                    Ok(_) => println!("   ✅ NOD Scorpion loaded"),
                    Err(e) => println!("   ⚠️  Asset loading: {}", e),
                }
                match device.load_w3d_model("terrain/desert_01.w3d").await {
                    Ok(_) => println!("   ✅ Desert terrain loaded"),
                    Err(e) => println!("   ⚠️  Asset loading: {}", e),
                }
                println!();

                println!("🎬 Starting rendering loop...");
                println!("Press ESC to exit");
                println!();

                // Main rendering loop
                let mut frame_count = 0u64;
                let start_time = Instant::now();
                let mut last_stats_time = start_time;

                event_loop.run(move |event, elwt| {
                    match event {
                        Event::WindowEvent { event: WindowEvent::CloseRequested, .. } |
                        Event::WindowEvent { event: WindowEvent::KeyboardInput {
                            event: winit::event::KeyEvent {
                                logical_key: winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape),
                                ..
                            },
                            ..
                        }, .. } => {
                            println!("👋 Shutting down W3D Revolutionary Engine");
                            elwt.exit();
                        }

                        Event::WindowEvent { event: WindowEvent::Resized(size), .. } => {
                            if let Err(e) = device.resize(size.width, size.height) {
                                eprintln!("⚠️  Resize error: {}", e);
                            }
                        }

                        Event::AboutToWait => {
                            // Request redraw
                            device.window().request_redraw();
                        }

                        Event::WindowEvent { event: WindowEvent::RedrawRequested, .. } => {
                            // Begin revolutionary rendering frame
                            if let Err(e) = device.begin_frame() {
                                eprintln!("⚠️  Begin frame error: {}", e);
                                return;
                            }

                            // Execute the most advanced rendering pipeline ever created
                            if let Err(e) = device.render() {
                                eprintln!("⚠️  Render error: {}", e);
                                return;
                            }

                            // End frame
                            if let Err(e) = device.end_frame() {
                                eprintln!("⚠️  End frame error: {}", e);
                                return;
                            }

                            frame_count += 1;

                            // Print performance statistics every 2 seconds
                            if last_stats_time.elapsed().as_secs_f32() >= 2.0 {
                                let stats = device.get_stats();
                                let elapsed = start_time.elapsed().as_secs_f32();

                                println!("📊 W3D Performance Stats (Frame {}):", frame_count);
                                println!("   🎯 FPS: {:.1} | Frame Time: {:.2}ms", stats.fps, stats.frame_time_ms);
                                println!("   🎨 Draw Calls: {} | Triangles: {}K | Vertices: {}K",
                                        stats.draw_calls,
                                        stats.triangles / 1000,
                                        stats.vertices / 1000);
                                println!("   🧱 Meshes: {} | Passes: {} | Vertex Colors: {}",
                                        stats.meshes,
                                        stats.material_passes,
                                        stats.vertex_color_passes);
                                println!("   🎛️ Texture Switches: {} | Shader Switches: {}",
                                        stats.texture_switches,
                                        stats.shader_switches);
                                println!("   💡 Lights: {} | Shadow Maps: {}",
                                        stats.active_lights,
                                        stats.shadow_maps_updated);
                                println!("   💾 GPU Memory: {:.1}MB | CPU Memory: {:.1}MB",
                                        stats.gpu_memory_used as f32 / 1024.0 / 1024.0,
                                        stats.cpu_memory_used as f32 / 1024.0 / 1024.0);
                                println!("   ⏱️  Pass Times - Depth: {:.2}ms | G-Buffer: {:.2}ms | Lighting: {:.2}ms | Forward: {:.2}ms | Post: {:.2}ms",
                                        stats.depth_prepass_time,
                                        stats.gbuffer_pass_time,
                                        stats.lighting_pass_time,
                                        stats.forward_pass_time,
                                        stats.post_processing_time);
                                println!("   ⚡ Total Runtime: {:.1}s | Avg FPS: {:.1}",
                                        elapsed,
                                        frame_count as f32 / elapsed);
                                println!();

                                last_stats_time = Instant::now();
                            }
                        }

                        _ => {}
                    }
                })?;
            }

            Err(e) => {
                eprintln!("❌ Failed to create W3D Device: {}", e);
                eprintln!("   This could be due to:");
                eprintln!("   • Graphics drivers not supporting required features");
                eprintln!("   • Insufficient GPU memory");
                eprintln!("   • Missing Vulkan/DirectX 12/Metal support");
                return Err(e.into());
            }
        }
    }

    Ok(())
}

#[cfg(not(feature = "w3d_support"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("❌ W3D support not compiled in!");
    println!("   Please run with: cargo run --example w3d_demo --features w3d_support");
    Ok(())
}
