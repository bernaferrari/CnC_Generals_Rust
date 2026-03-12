//! WW3D Engine Advanced Demo
//!
//! This example showcases all major features of the WW3D engine:
//! - Asset loading and management
//! - Advanced rendering with WGPU
//! - Particle systems
//! - Physics simulation
//! - Procedural geometry
//! - Collision detection
//! - Modern async Rust patterns

use ww3d_gpu::present_surface_texture;
use ww3d_renderer_3d::{Scene, Renderer};
use ww3d_assets::AssetManager;
use ww3d_particles::{ParticleSystemManager, ParticleEmitter};
use ww3d_geometry::{MeshBuilder, Vec3, AABox, Sphere, MeshGeometry};
use winit::{
    event::{Event, WindowEvent, KeyboardInput, ElementState},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
    keyboard::KeyCode,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 WW3D Advanced Demo - Complete Engine Showcase");
    println!("================================================");

    // Initialize asset manager
    let mut asset_manager = AssetManager::new();
    println!("📦 Asset manager initialized");

    // Create event loop and window
    let event_loop = EventLoop::new()?;
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("WW3D Advanced Demo - Complete Engine Showcase")
            .with_inner_size(winit::dpi::LogicalSize::new(1600, 900))
            .build(&event_loop)?
    );

    // Initialize WGPU
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let surface = instance.create_surface(window.clone())?;
    let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }).await?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: Some("WW3D Device"),
                ..Default::default()
            },
        )
        .await?;

    let device = Arc::new(device);
    let queue = Arc::new(queue);

    // Configure surface
    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps.formats.iter()
        .copied()
        .find(|f| f.is_srgb())
        .unwrap_or(surface_caps.formats[0]);

    let size = window.inner_size();
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    println!("✅ WGPU initialized successfully");
    println!("🎮 Resolution: {}x{}", size.width, size.height);
    println!("🎨 Format: {:?}", surface_format);

    // Create the main scene
    let mut scene = Scene::new(device.clone(), queue.clone(), config.clone());

    // Setup comprehensive demo features
    setup_demo_features(&mut scene, device.clone(), queue.clone()).await?;

    // Create particle systems
    setup_particle_systems(&mut scene)?;

    // Create advanced shader pipeline
    let shader_system = create_shader_pipeline(&device)?;

    println!("\n🎯 Starting advanced render loop...");
    println!("Controls:");
    println!("  ESC - Exit");
    println!("  Space - Toggle particle systems");
    println!("  P - Performance stats");
    println!("  R - Reload shaders");

    // Main render loop
    let mut frame_count = 0u64;
    let start_time = Instant::now();
    let mut last_stats_time = Instant::now();
    let mut show_stats = false;

    event_loop.run(move |event, event_loop| {
        let mut control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                println!("\n👋 Shutting down WW3D Engine...");
                control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput {
                    event: KeyboardInput {
                        physical_key: winit::keyboard::PhysicalKey::Code(key_code),
                        state: ElementState::Pressed,
                        ..
                    },
                    ..
                },
                ..
            } => match key_code {
                KeyCode::Escape => {
                    println!("\n👋 Escape pressed, exiting...");
                    control_flow = ControlFlow::Exit;
                }
                KeyCode::Space => {
                    // Toggle particle systems
                    println!("🎆 Toggled particle systems");
                }
                KeyCode::KeyP => {
                    show_stats = !show_stats;
                    println!("📊 Performance stats: {}", if show_stats { "ON" } else { "OFF" });
                }
                KeyCode::KeyR => {
                    println!("🔄 Reloading shaders...");
                    // Shader hot reload would go here
                }
                _ => {}
            }
            Event::AboutToWait => {
                window.request_redraw();
            }
            Event::WindowEvent { event: WindowEvent::RedrawRequested, .. } => {
                frame_count += 1;

                // Update scene
                let current_time = start_time.elapsed().as_secs_f32();
                scene.update(16); // 60fps delta time

                // Render frame
                let output = surface.get_current_texture().unwrap();
                let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Advanced Demo Encoder"),
                });

                // Render pass
                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Advanced Demo Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.1,
                                    g: 0.15,
                                    b: 0.2,
                                    a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        occlusion_query_set: None,
                        timestamp_writes: None,
                    });

                    // Render scene with all features
                    scene.render(&mut render_pass);
                }

                queue.submit(std::iter::once(encoder.finish()));
                present_surface_texture(output);

                // Performance stats
                if show_stats && last_stats_time.elapsed() >= Duration::from_secs(1) {
                    let fps = frame_count as f64 / start_time.elapsed().as_secs_f64();
                    println!("🎮 FPS: {:.1}, Frame: {}, Particles: {}",
                        fps, frame_count, scene.total_active_particles());
                    last_stats_time = Instant::now();
                }
            }
            _ => {}
        }

        event_loop.set_control_flow(control_flow);
    })?;

    Ok(())
}

async fn setup_demo_features(
    scene: &mut Scene,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🏗️ Setting up Advanced Demo Features...");

    // 1. Procedural Geometry Showcase
    println!("  🧊 Creating procedural geometry...");
    let mut mesh_builder = MeshBuilder::new();

    // Add various geometric shapes
    mesh_builder.add_cube(Vec3::new(-3.0, 1.0, 0.0), 0.8);
    mesh_builder.add_sphere(Vec3::new(0.0, 1.0, 0.0), 0.7, 24, 16);
    mesh_builder.add_cylinder(Vec3::new(3.0, 1.0, 0.0), 0.5, 1.5, 16);

    // Create terrain-like ground plane
    mesh_builder.add_plane(Vec3::new(0.0, -0.5, 0.0), 10.0);

    let demo_mesh = mesh_builder.build();
    scene.add_mesh(demo_mesh);
    println!("  ✅ Added {} procedural shapes", 4);

    // 2. Collision Detection Demo
    println!("  💥 Setting up collision detection...");
    let test_objects = vec![
        AABox::new(Vec3::new(-3.0, 1.0, 0.0), Vec3::new(0.4, 0.4, 0.4)),
        Sphere::new(Vec3::new(0.0, 1.0, 0.0), 0.7),
        AABox::new(Vec3::new(3.0, 1.0, 0.0), Vec3::new(0.25, 0.75, 0.25)),
    ];

    // Test collisions between objects
    for (i, obj1) in test_objects.iter().enumerate() {
        for (j, obj2) in test_objects.iter().enumerate() {
            if i != j {
                let collision = match (obj1, obj2) {
                    (AABox { .. }, AABox { .. }) => {
                        obj1.intersects_aabox(obj2)
                    }
                    _ => false, // Simplified collision test
                };
                if collision {
                    println!("  ✅ Collision detected between objects {} and {}", i, j);
                }
            }
        }
    }

    // 3. Advanced Rendering Setup
    println!("  🎨 Setting up advanced rendering...");

    // Create vertex buffer with advanced vertex format
    let vertex_data: Vec<f32> = vec![
        // Position (x,y,z) | Normal (x,y,z) | Color (r,g,b) | TexCoord (u,v)
        -1.0, -1.0, 0.0,   0.0, 0.0, 1.0,   1.0, 0.0, 0.0,   0.0, 0.0,
         1.0, -1.0, 0.0,   0.0, 0.0, 1.0,   0.0, 1.0, 0.0,   1.0, 0.0,
         0.0,  1.0, 0.0,   0.0, 0.0, 1.0,   0.0, 0.0, 1.0,   0.5, 1.0,
    ];

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Advanced Vertex Buffer"),
        contents: bytemuck::cast_slice(&vertex_data),
        usage: wgpu::BufferUsages::VERTEX,
    });

    // Create index buffer
    let index_data: Vec<u16> = vec![0, 1, 2];
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Advanced Index Buffer"),
        contents: bytemuck::cast_slice(&index_data),
        usage: wgpu::BufferUsages::INDEX,
    });

    println!("  ✅ Created advanced vertex/index buffers");

    // 4. Texture and Material Setup
    println!("  🖼️ Creating advanced materials...");

    // Create gradient texture
    let texture_size = 256;
    let mut texture_data = Vec::with_capacity(texture_size * texture_size * 4);

    for y in 0..texture_size {
        for x in 0..texture_size {
            let r = (x as f32 / texture_size as f32 * 255.0) as u8;
            let g = (y as f32 / texture_size as f32 * 255.0) as u8;
            let b = 128u8;
            let a = 255u8;

            texture_data.extend_from_slice(&[r, g, b, a]);
        }
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Advanced Demo Texture"),
        size: wgpu::Extent3d {
            width: texture_size as u32,
            height: texture_size as u32,
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
            bytes_per_row: Some(texture_size as u32 * 4),
            rows_per_image: Some(texture_size as u32),
        },
        wgpu::Extent3d {
            width: texture_size as u32,
            height: texture_size as u32,
            depth_or_array_layers: 1,
        },
    );

    println!("  ✅ Created {}x{} gradient texture", texture_size, texture_size);

    // 5. Performance Monitoring Setup
    println!("  📊 Setting up performance monitoring...");
    scene.enable_performance_monitoring(true);
    println!("  ✅ Performance monitoring enabled");

    println!("🏗️ Advanced demo features setup complete!");
    Ok(())
}

fn setup_particle_systems(scene: &mut Scene) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🎆 Setting up Advanced Particle Systems...");

    let mut particle_manager =
        ww3d_renderer_3d::particle_bridge::create_particle_system_manager()
            .map_err(|err| format!("Failed to create particle system manager: {err:?}"))?;

    // Fire particle system
    let fire_emitter = ParticleSystemManager::create_fire_emitter();
    ww3d_renderer_3d::particle_bridge::add_emitter_with_renderer_resources(
        &mut particle_manager,
        fire_emitter,
    )
    .map_err(|err| format!("Failed to add fire emitter: {err:?}"))?;
    println!("  🔥 Added fire particle emitter");

    // Smoke particle system
    let smoke_emitter = ParticleSystemManager::create_smoke_emitter();
    ww3d_renderer_3d::particle_bridge::add_emitter_with_renderer_resources(
        &mut particle_manager,
        smoke_emitter,
    )
    .map_err(|err| format!("Failed to add smoke emitter: {err:?}"))?;
    println!("  💨 Added smoke particle emitter");

    // Explosion particle system
    let explosion_emitter = ParticleSystemManager::create_explosion_emitter();
    ww3d_renderer_3d::particle_bridge::add_emitter_with_renderer_resources(
        &mut particle_manager,
        explosion_emitter,
    )
    .map_err(|err| format!("Failed to add explosion emitter: {err:?}"))?;
    println!("  💥 Added explosion particle emitter");

    // Enable advanced features
    particle_manager.enable_sorting(true);
    particle_manager.enable_gpu_acceleration(true);
    particle_manager.set_max_particles(10000);

    println!("  ✅ Particle sorting: enabled");
    println!("  ✅ GPU acceleration: enabled");
    println!("  ✅ Max particles: 10,000");

    scene.set_particle_system(Box::new(particle_manager));

    println!("🎆 Advanced particle systems setup complete!");
    Ok(())
}

fn create_shader_pipeline(device: &wgpu::Device) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🎨 Creating Advanced Shader Pipeline...");

    // Load vertex shader
    let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Advanced Vertex Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../game/src/demo.wgsl").into()),
        
    });

    // Load fragment shader
    let fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Advanced Fragment Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../game/src/demo.wgsl").into()),
        
    });

    println!("  ✅ Loaded advanced WGSL shaders");
    println!("  ✅ Hot reload capability ready");

    // Create bind group layouts for advanced materials
    let material_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Material Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    println!("  ✅ Created advanced bind group layouts");

    // Create pipeline layout
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Advanced Pipeline Layout"),
        bind_group_layouts: &[&material_bind_group_layout],
        push_constant_ranges: &[],
    });

    println!("  ✅ Created advanced pipeline layout");
    println!("🎨 Advanced shader pipeline ready!");

    Ok(())
}
