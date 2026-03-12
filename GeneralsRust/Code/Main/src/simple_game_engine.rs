use game_engine::common::frame_clock::FrameClock;
use anyhow::Result;
use glam::{Mat4, Vec3, Vec4};
use log::{info, warn, error};
use std::sync::Arc;
use std::sync::RwLock;
use wgpu::{Surface, Device, Queue, SurfaceConfiguration};
use ww3d_engine::FrameTiming;
use ww3d_gpu::present_surface_texture;
use winit::{
    application::ApplicationHandler,
    event::{Event, WindowEvent, KeyEvent, ElementState},
    event_loop::{EventLoop, ControlFlow},
    keyboard::{Key, NamedKey},
    window::{Window, WindowBuilder},
};

// Audio imports
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::fs::File;
use std::io::BufReader;

pub struct GameEngine {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    
    // Audio system
    #[allow(dead_code)]
    audio_output: OutputStream,
    audio_handle: OutputStreamHandle,
    background_music: Option<Sink>,
    sound_effects: Vec<Sink>,
    
    // Game state
    camera_pos: Vec3,
    camera_rotation: Vec3,
    models: Vec<GameModel>,
    
    // Input state
    keys_pressed: std::collections::HashSet<Key>,
}

pub struct GameModel {
    pub position: Vec3,
    pub rotation: Vec3,
    pub scale: f32,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

// Sample vertices for a colorful cube
const VERTICES: &[Vertex] = &[
    // Front face
    Vertex { position: [-1.0, -1.0,  1.0], color: [1.0, 0.0, 0.0] },
    Vertex { position: [ 1.0, -1.0,  1.0], color: [0.0, 1.0, 0.0] },
    Vertex { position: [ 1.0,  1.0,  1.0], color: [0.0, 0.0, 1.0] },
    Vertex { position: [-1.0,  1.0,  1.0], color: [1.0, 1.0, 0.0] },
    // Back face  
    Vertex { position: [-1.0, -1.0, -1.0], color: [1.0, 0.0, 1.0] },
    Vertex { position: [ 1.0, -1.0, -1.0], color: [0.0, 1.0, 1.0] },
    Vertex { position: [ 1.0,  1.0, -1.0], color: [1.0, 1.0, 1.0] },
    Vertex { position: [-1.0,  1.0, -1.0], color: [0.5, 0.5, 0.5] },
];

const INDICES: &[u16] = &[
    // Front face
    0, 1, 2,  2, 3, 0,
    // Back face
    4, 6, 5,  6, 4, 7,
    // Left face
    4, 0, 3,  3, 7, 4,
    // Right face
    1, 5, 6,  6, 2, 1,
    // Top face
    3, 2, 6,  6, 7, 3,
    // Bottom face
    4, 5, 1,  1, 0, 4,
];

impl GameEngine {
    pub async fn new(window: Arc<Window>) -> Result<Self> {
        info!("Initializing Game Engine with wgpu graphics and audio");
        
        let size = window.inner_size();

        // Initialize wgpu
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: None,
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                ..Default::default()
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        // Create shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../shaders/basic.wgsl").into()),
            
        });

        // Create render pipeline
        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
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

        // Initialize audio system
        let (audio_output, audio_handle) = OutputStream::try_default()
            .map_err(|e| anyhow::anyhow!("Failed to initialize audio output: {}", e))?;

        // Create a basic cube model
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let cube_model = GameModel {
            position: Vec3::new(0.0, 0.0, -3.0),
            rotation: Vec3::ZERO,
            scale: 1.0,
            vertex_buffer,
            index_buffer,
            num_indices: INDICES.len() as u32,
        };

        info!("Game Engine initialized successfully!");
        info!("- Graphics: wgpu with {} surface", config.format.describe().long_name);
        info!("- Audio: Rodio output stream initialized");
        info!("- Models: 1 cube loaded");

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            render_pipeline,
            audio_output,
            audio_handle,
            background_music: None,
            sound_effects: Vec::new(),
            camera_pos: Vec3::new(0.0, 0.0, 0.0),
            camera_rotation: Vec3::ZERO,
            models: vec![cube_model],
            keys_pressed: std::collections::HashSet::new(),
        })
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    logical_key: key,
                    state,
                    ..
                },
                ..
            } => {
                match state {
                    ElementState::Pressed => {
                        self.keys_pressed.insert(key.clone());
                        self.handle_key_press(key);
                    }
                    ElementState::Released => {
                        self.keys_pressed.remove(key);
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn handle_key_press(&mut self, key: &Key) {
        match key {
            Key::Named(NamedKey::Space) => {
                info!("Space key pressed - playing sound effect");
                self.play_sound_effect();
            }
            Key::Character(c) if c == "m" || c == "M" => {
                info!("M key pressed - toggling background music");
                self.toggle_background_music();
            }
            Key::Named(NamedKey::Escape) => {
                info!("Escape key pressed - should exit game");
            }
            _ => {}
        }
    }

    pub fn update(&mut self, dt: f32) {
        // Update camera based on input
        let move_speed = 5.0 * dt;
        let rot_speed = 2.0 * dt;

        if self.keys_pressed.contains(&Key::Character("w".into())) {
            self.camera_pos.z -= move_speed;
        }
        if self.keys_pressed.contains(&Key::Character("s".into())) {
            self.camera_pos.z += move_speed;
        }
        if self.keys_pressed.contains(&Key::Character("a".into())) {
            self.camera_pos.x -= move_speed;
        }
        if self.keys_pressed.contains(&Key::Character("d".into())) {
            self.camera_pos.x += move_speed;
        }

        // Rotate cube for visual effect
        for model in &mut self.models {
            model.rotation.y += rot_speed;
            model.rotation.x += rot_speed * 0.7;
        }
    }

    pub fn update_with_timing(&mut self, timing: &FrameTiming) {
        self.update(timing.delta_seconds());
    }

    pub fn render(&mut self) -> Result<()> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);

            // Render each model
            for model in &self.models {
                render_pass.set_vertex_buffer(0, model.vertex_buffer.slice(..));
                render_pass.set_index_buffer(model.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..model.num_indices, 0, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        present_surface_texture(output);

        Ok(())
    }

    pub fn play_sound_effect(&mut self) {
        // Create a simple beep sound effect
        let sink = Sink::try_new(&self.audio_handle).unwrap();
        
        // Create a simple sine wave beep
        let sample_rate = 44_100;
        let duration = 0.2; // 200ms
        let frequency = 440.0; // A note
        
        let samples: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (t * frequency * 2.0 * std::f32::consts::PI).sin() * 0.3
            })
            .collect();

        let source = rodio::buffer::SamplesBuffer::new(1, sample_rate, samples);
        sink.append(source);
        self.sound_effects.push(sink);
    }

    pub fn toggle_background_music(&mut self) {
        if let Some(music) = &self.background_music {
            if music.is_paused() {
                music.play();
                info!("Background music resumed");
            } else {
                music.pause();
                info!("Background music paused");
            }
        } else {
            // Create background music - simple ambient tone
            let sink = Sink::try_new(&self.audio_handle).unwrap();
            
            let sample_rate = 44_100;
            let duration = 10.0; // 10 seconds loop
            let samples: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
                .map(|i| {
                    let t = i as f32 / sample_rate as f32;
                    let base = (t * 220.0 * 2.0 * std::f32::consts::PI).sin() * 0.1;
                    let harmony = (t * 330.0 * 2.0 * std::f32::consts::PI).sin() * 0.05;
                    base + harmony
                })
                .collect();

            let source = rodio::buffer::SamplesBuffer::new(1, sample_rate, samples)
                .repeat_infinite();
            sink.append(source);
            
            self.background_music = Some(sink);
            info!("Background music started");
        }
    }
}

pub struct GameApplication {
    engine: Option<GameEngine>,
    frame_clock: FrameClock,
}

impl GameApplication {
    pub fn new() -> Self {
        Self {
            engine: None,
            frame_clock: FrameClock::new(),
        }
    }
}

impl ApplicationHandler for GameApplication {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.engine.is_none() {
            let window = Arc::new(
                event_loop
                    .create_window(
                        WindowBuilder::new()
                            .with_title("Command & Conquer Generals Zero Hour - Rust Edition")
                            .with_inner_size(winit::dpi::LogicalSize::new(1024, 768))
                    )
                    .unwrap()
            );

            // Initialize the engine asynchronously
            let engine = pollster::block_on(GameEngine::new(window));
            match engine {
                Ok(mut engine) => {
                    info!("Game engine initialized successfully!");
                    // Start background music
                    engine.toggle_background_music();
                    self.engine = Some(engine);
                }
                Err(e) => {
                    error!("Failed to initialize game engine: {}", e);
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if let Some(engine) = &mut self.engine {
            if engine.input(&event) {
                return;
            }

            match event {
                WindowEvent::CloseRequested
                | WindowEvent::KeyboardInput {
                    event: KeyEvent {
                        state: ElementState::Pressed,
                        logical_key: Key::Named(NamedKey::Escape),
                        ..
                    },
                    ..
                } => {
                    info!("Exit requested");
                    event_loop.exit();
                }
                WindowEvent::Resized(physical_size) => {
                    engine.resize(physical_size);
                }
                WindowEvent::RedrawRequested => {
                    let timing = self.frame_clock.next_frame();
                    engine.update_with_timing(&timing);
                    match engine.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => engine.resize(engine.window.inner_size()),
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            error!("OutOfMemory");
                            event_loop.exit();
                        }
                        Err(e) => {
                            error!("Render error: {:?}", e);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(engine) = &self.engine {
            engine.window.request_redraw();
        }
    }
}

pub async fn run_game() -> Result<()> {
    env_logger::init();
    info!("Starting Command & Conquer Generals Zero Hour - Rust Edition");

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    
    let mut app = GameApplication::new();
    event_loop.run_app(&mut app)?;

    info!("Game ended successfully");
    Ok(())
}
