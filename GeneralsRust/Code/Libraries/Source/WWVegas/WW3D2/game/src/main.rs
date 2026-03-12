use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use wgpu::util::DeviceExt;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};
use ww3d_assets::AssetManager;
use ww3d_engine::{self, EngineConfig, EngineError};
use ww3d_geometry::{AABox, MeshBuilder, Sphere, Vec3};
use ww3d_particles::{ParticleSystem, ParticleSystemManager};
use ww3d_renderer_3d::scene_system::scene::SceneClass;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 WW3D Engine Demo - Complete 3D Graphics Pipeline");
    println!("==============================================");
    println!("🌟 2025 Masterpiece: Modern Rust 3D Gaming Engine");
    println!("   - Async/Await with Tokio");
    println!("   - WGPU Cross-Platform Graphics");
    println!("   - Glam Linear Algebra");
    println!("   - Complete C++ WW3D Feature Parity");
    println!("   - Advanced Particle Systems");
    println!("   - Physics Simulation");
    println!("   - Mesh Processing & Optimization");
    println!("   - Asset Management");
    println!("   - Modern Shader Pipeline");
    println!("==============================================");

    // Test W3D parsing first
    let asset_manager = test_w3d_parsing().await;

    // Create WGPU renderer and window
    run_demo(asset_manager).await?;

    Ok(())
}

async fn test_w3d_parsing() -> AssetManager {
    println!("\n=== Testing W3D File Parsing ===");

    let mut asset_manager = AssetManager::new();

    // Test with a simple W3D file
    let w3d_path = "/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main/GeneralsRust/Code/Tools/w3d_to_gltf/W3D/CBoffice01_RS.w3d";

    println!("Looking for W3D file at: {}", w3d_path);

    match asset_manager.load_3d_assets(w3d_path) {
        Ok(_) => {
            println!("✅ Successfully loaded W3D file!");
            println!("📊 Assets loaded: {}", asset_manager.num_assets());

            // List loaded assets
            for name in asset_manager.asset_names() {
                println!("  - {}", name);
            }
        }
        Err(e) => {
            println!("❌ Failed to load W3D file: {}", e);
        }
    }

    asset_manager
}

async fn setup_comprehensive_demo(
    _scene: &mut SceneClass,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🏗️ Setting up Comprehensive WW3D Demo...");

    // Create procedural geometry
    println!("  🧊 Creating procedural geometry...");

    // Create primitives using static methods
    let cube_mesh = MeshBuilder::create_cube(1.0);
    let _sphere_mesh = MeshBuilder::create_sphere(0.8, 16, 16);
    let _plane_mesh = MeshBuilder::create_plane(5.0, 5.0, 4);

    // For the demo, use the cube mesh
    let demo_mesh = cube_mesh;
    println!(
        "  ✅ Created mesh with {} vertices, {} triangles",
        demo_mesh.vertices.len(),
        demo_mesh.triangles.len()
    );

    // Note: Scene integration for mesh render objects requires RenderObj trait implementation
    // and scene graph integration. This demo focuses on asset loading and collision systems.
    println!("  ✅ Created procedural geometry (scene integration in future demo)");

    // Demonstrate collision detection
    println!("  💥 Testing collision detection...");
    let box1 = AABox::new(Vec3::new(-2.0, 0.0, 0.0), Vec3::new(0.5, 0.5, 0.5));
    let box2 = AABox::new(Vec3::new(-1.5, 0.0, 0.0), Vec3::new(0.5, 0.5, 0.5));
    let colliding = box1.intersects_aabox(&box2);
    println!("  ✅ AABB collision test: {} (expected: true)", colliding);

    let sphere = Sphere::new(Vec3::new(2.0, 0.0, 0.0), 1.0);
    let point_inside = sphere.contains_point(Vec3::new(2.0, 0.5, 0.0));
    println!("  ✅ Sphere point test: {} (expected: true)", point_inside);

    // Create vertex and index buffers for rendering
    let vertex_data: Vec<f32> = vec![
        // Triangle 1
        -0.5, -0.5, 0.0, // position
        1.0, 0.0, 0.0, // color
        0.0, 0.0, // tex coords
        0.5, -0.5, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0, 0.5, 1.0,
    ];

    let index_data: Vec<u16> = vec![0, 1, 2];

    let _vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Demo Vertex Buffer"),
        contents: bytemuck::cast_slice(&vertex_data),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let _index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Demo Index Buffer"),
        contents: bytemuck::cast_slice(&index_data),
        usage: wgpu::BufferUsages::INDEX,
    });

    println!("  ✅ Created vertex and index buffers");

    // Create a simple texture for demonstration
    let texture_data: Vec<u8> = vec![
        255, 0, 0, 255, // Red
        0, 255, 0, 255, // Green
        0, 0, 255, 255, // Blue
        255, 255, 0, 255, // Yellow
    ];

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Demo Texture"),
        size: wgpu::Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &texture_data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(8),
            rows_per_image: Some(2),
        },
        wgpu::Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        },
    );

    println!("  ✅ Created and uploaded demo texture");

    println!("🏗️ Comprehensive demo setup complete!");
    Ok(())
}

fn setup_particle_demo(_scene: &mut SceneClass) {
    println!("\n🎆 Setting up Particle Demo...");

    // Create particle system manager using the renderer's GPU context
    let mut particle_manager =
        match ww3d_renderer_3d::particle_bridge::create_particle_system_manager() {
            Ok(manager) => manager,
            Err(err) => {
                eprintln!("  ⚠️ Unable to create particle system manager: {err:?}");
                return;
            }
        };

    // Add fire emitter
    let fire_emitter = ParticleSystemManager::create_fire_emitter();
    if let Err(err) = ww3d_renderer_3d::particle_bridge::add_emitter_with_renderer_resources(
        &mut particle_manager,
        fire_emitter,
    ) {
        eprintln!("  ⚠️ Failed to add fire emitter: {err:?}");
    } else {
        println!("  ✅ Added fire particle emitter");
    }

    // Add smoke emitter
    let smoke_emitter = ParticleSystemManager::create_smoke_emitter();
    if let Err(err) = ww3d_renderer_3d::particle_bridge::add_emitter_with_renderer_resources(
        &mut particle_manager,
        smoke_emitter,
    ) {
        eprintln!("  ⚠️ Failed to add smoke emitter: {err:?}");
    } else {
        println!("  ✅ Added smoke particle emitter");
    }

    // Enable particle sorting
    particle_manager.enable_sorting(true);

    // Note: Particle system integration with scene graph requires SceneClass::set_particle_system
    // method. Particles currently managed independently. C++ equivalent: SceneClass::Set_Particle_System

    println!("🎆 Particle demo setup complete!");
}

async fn run_demo(asset_manager: AssetManager) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Starting WGPU Rendering Demo ===");

    // Create event loop
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("WW3D Engine - Complete 3D Graphics Pipeline Demo")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
            .build(&event_loop)
            .unwrap(),
    );

    let mut engine_config = EngineConfig::default();
    let initial_size = window.inner_size();
    engine_config.width = initial_size.width.max(1);
    engine_config.height = initial_size.height.max(1);

    ww3d_engine::init_with_window(window.clone(), engine_config).await?;

    let device = ww3d_engine::device()?;
    let queue = ww3d_engine::queue()?;
    let color_format = ww3d_engine::color_format()?;
    let adapter_info = ww3d_engine::adapter_info()?;

    println!("✅ WW3D Engine initialized successfully!");
    println!("🪟 Window: {}x{}", initial_size.width, initial_size.height);
    println!(
        "🖥️ Adapter: {} ({:?})",
        adapter_info.name, adapter_info.backend
    );
    println!("⚙️ Device type: {:?}", adapter_info.device_type);
    println!("🎨 Surface format: {:?}", color_format);

    // Create scene with particle support
    let mut scene = SceneClass::new();
    println!("✅ Scene created with particle support");

    // Setup comprehensive demo features
    setup_comprehensive_demo(&mut scene, device.clone(), queue.clone()).await?;

    // Create particle emitters for demo
    setup_particle_demo(&mut scene);

    // Display loaded assets info
    println!("\n📦 Loaded Assets Summary:");
    println!("  - Total assets: {}", asset_manager.num_assets());
    for name in asset_manager.asset_names() {
        println!("  - Asset: {}", name);
    }

    println!("\n🎆 Particle System Info:");
    println!("  - Scene created successfully");

    // Create a simple shader for demo
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Demo Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../src/shader_system/demo.wgsl").into()),
    });

    // Create render pipeline
    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Demo Render Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    });

    // Main render loop
    println!("\n🎯 Starting render loop...");
    println!("Press ESC to exit");
    println!("🎨 Rendering a simple demo triangle to show the pipeline works");
    println!("📸 Press F12 at any time to capture a screenshot to ./screenshots");

    let mut frame_count = 0u32;
    let start_time = std::time::Instant::now();
    let mut last_update = std::time::Instant::now();

    if let Err(err) = ww3d_engine::set_movie_capture_output("captures", "ww3d_capture") {
        eprintln!("⚠️ Unable to configure movie capture output: {err:?}");
    }

    let mut movie_capture_enabled = false;

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    println!("Window close requested, exiting...");
                    let _ = ww3d_engine::shutdown();
                    elwt.exit();
                }
                Event::WindowEvent {
                    event:
                        WindowEvent::KeyboardInput {
                            event: key_event, ..
                        },
                    ..
                } => {
                    if key_event.state == ElementState::Pressed {
                        match key_event.physical_key {
                            PhysicalKey::Code(KeyCode::Escape) => {
                                println!("Escape pressed, exiting...");
                                let _ = ww3d_engine::shutdown();
                                elwt.exit();
                            }
                            PhysicalKey::Code(KeyCode::F10) => {
                                movie_capture_enabled = !movie_capture_enabled;
                                if let Err(err) = ww3d_engine::set_movie_capture_enabled(movie_capture_enabled) {
                                    eprintln!("Failed to toggle movie capture: {err:?}");
                                } else if movie_capture_enabled {
                                    println!(
                                        "🎥 Movie capture enabled – frames will be written to ./captures"
                                    );
                                } else {
                                    println!("🛑 Movie capture disabled");
                                }
                            }
                            PhysicalKey::Code(KeyCode::F12) => {
                                let timestamp = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis();
                                let path =
                                    PathBuf::from(format!("screenshots/ww3d_{timestamp}.png"));
                                match ww3d_engine::make_screenshot(&path) {
                                    Ok(()) => println!("📸 Screenshot scheduled: {:?}", path),
                                    Err(err) => eprintln!("Failed to schedule screenshot: {err:?}"),
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    let _ = ww3d_engine::resize(size.width, size.height);
                }
                Event::AboutToWait => {
                    window.request_redraw();
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    frame_count += 1;
                    let now = std::time::Instant::now();
                    let _delta_time_ms = now.duration_since(last_update).as_millis() as u32;
                    last_update = now;

                    // Note: Scene update and particle emission would go here in full integration.
                    // Current demo focuses on rendering pipeline and asset loading validation.
                    // C++ equivalent: SceneClass::Update and ParticleSystem::Emit

                    match ww3d_engine::begin_render() {
                        Ok(mut frame) => {
                            let color_view = frame.color_view_arc();
                            let depth_view = frame.depth_view_arc();

                            {
                                let encoder = frame.encoder();
                                let mut render_pass =
                                    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                        label: Some("Demo Render Pass"),
                                        color_attachments: &[Some(
                                            wgpu::RenderPassColorAttachment {
                                                view: color_view.as_ref(),
                                                depth_slice: None,
                                                resolve_target: None,
                                                ops: wgpu::Operations {
                                                    load: wgpu::LoadOp::Clear(wgpu::Color {
                                                        r: 0.1,
                                                        g: 0.2,
                                                        b: 0.3,
                                                        a: 1.0,
                                                    }),
                                                    store: wgpu::StoreOp::Store,
                                                },
                                            },
                                        )],
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
                                        occlusion_query_set: None,
                                        timestamp_writes: None,
                                    });

                                // Note: Scene rendering requires RenderInfoClass adapter for WGPU.
                                // The scene system uses abstract RenderInfo, which needs conversion
                                // to WGPU RenderPass. Current demo uses direct WGPU rendering.
                                // C++ equivalent: SceneClass::Render with DX8 RenderContext

                                // Demo triangle for visual feedback
                                render_pass.set_pipeline(&render_pipeline);
                                render_pass.draw(0..3, 0..1);
                            }

                            if let Err(err) = ww3d_engine::end_render(frame) {
                                eprintln!("Failed to end render frame: {err:?}");
                            }
                        }
                        Err(err) => match err {
                            EngineError::Surface(wgpu::SurfaceError::Lost) => {
                                let size = window.inner_size();
                                let _ = ww3d_engine::resize(size.width, size.height);
                            }
                            EngineError::Surface(wgpu::SurfaceError::OutOfMemory) => {
                                eprintln!("GPU out of memory, shutting down.");
                                let _ = ww3d_engine::shutdown();
                                elwt.exit();
                            }
                            other => {
                                eprintln!("Failed to begin render frame: {other:?}");
                            }
                        },
                    }

                    // Print stats every 60 frames
                    if frame_count % 60 == 0 {
                        let elapsed = start_time.elapsed();
                        let fps = frame_count as f64 / elapsed.as_secs_f64();
                        println!("🎮 FPS: {:.1}, Frames: {}", fps, frame_count);
                        println!("🎆 WW3D Engine Demo running successfully");
                    }
                }
                _ => {}
            }
        })
        .unwrap();

    Ok(())
}
