#![allow(dead_code, unused_variables)]

/*
** Command & Conquer Generals Zero Hour(tm) - Win32GameEngine
** Copyright 2025 Electronic Arts Inc.
**
** Win32 implementation of the GameEngine (equivalent to Win32GameEngine.cpp)
*/

use crate::assets::{get_asset_manager, init_asset_manager, load_cnc_unit_model, W3DModel};
use crate::engine_factory::GameEngine;
use crate::game_logic::*;
use anyhow::Result;
use async_trait::async_trait;
use glam::{Mat4, Vec3};
use log::{info, warn};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use winit::{keyboard::Key, window::Window};
use ww3d_gpu::present_surface_texture;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

/// Win32GameEngine - Platform-specific implementation of GameEngine
/// Matches C++ Win32GameEngine class architecture
pub struct Win32GameEngine {
    // Window and graphics
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,

    // Uniform data
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,

    // Camera system (matching C++ RTS camera)
    view_matrix: Mat4,
    projection_matrix: Mat4,
    camera_position: Vec3,
    camera_target: Vec3,

    // Game state (matching C++ subsystems)
    game_logic: GameLogic,

    // Model cache for W3D models (matching C++ prototype system)
    loaded_models: HashMap<String, Arc<W3DModel>>,

    // Input and selection
    selected_objects: HashSet<ObjectId>,
    keys_pressed: HashSet<Key>,
    mouse_pos: (f32, f32),

    // Engine state (matching C++ GameEngine members)
    max_fps: u32,
    is_quitting: bool,
    is_active: bool,
    frame_time: Duration,

    // Add subsystem manager for factory pattern
    subsystem_manager: Option<SubsystemManager>,
}

use crate::subsystem_interfaces::SubsystemManager;

impl Win32GameEngine {
    /// CreateGameEngine() factory method (matching C++ pattern)
    /// Takes a window created by the main event loop
    pub async fn create_game_engine(window: Arc<Window>) -> Result<Self> {
        info!("Creating Win32GameEngine with provided window...");

        // Initialize graphics (equivalent to C++ W3DDisplay creation)
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
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
                ..Default::default()
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let size = window.inner_size();
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

        // Initialize asset manager for W3D models (matching C++ W3DAssetManager)
        info!("Initializing Asset Manager for W3D models...");
        if let Err(err) = init_asset_manager(&device, &queue).await {
            warn!("Asset Manager init failed: {err}. Continuing without assets.");
        }
        info!("Asset Manager initialized - ready to load W3D models!");

        // Create shaders (matching C++ shader system)
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("W3D Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::get_w3d_shader().into()),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("uniform_bind_group_layout"),
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        });

        // Create render pipeline
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Self::vertex_buffer_layout()],
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
            cache: None,
            multiview: None,
        });

        // Setup RTS camera (matching C++ camera system)
        let camera_position = Vec3::new(0.0, 50.0, 50.0); // Elevated RTS view
        let camera_target = Vec3::new(0.0, 0.0, 0.0);
        let view_matrix = Mat4::look_at_rh(camera_position, camera_target, Vec3::Y);
        let projection_matrix = Mat4::perspective_rh(
            45.0_f32.to_radians(),
            size.width as f32 / size.height as f32,
            0.1,
            1000.0,
        );

        // Create GameLogic (matching C++ W3DGameLogic creation)
        let game_logic = Self::create_game_logic();

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            render_pipeline,
            uniform_buffer,
            bind_group,
            view_matrix,
            projection_matrix,
            camera_position,
            camera_target,
            game_logic,
            loaded_models: HashMap::new(),
            selected_objects: HashSet::new(),
            keys_pressed: HashSet::new(),
            mouse_pos: (0.0, 0.0),
            max_fps: 60,
            is_quitting: false,
            is_active: true,
            frame_time: Duration::from_millis(16),
            subsystem_manager: None,
        })
    }

    /// Pre-load all W3D unit models (matching C++ asset loading)
    #[allow(dead_code)] // C++ parity: asset preloading, will be called during loading screens
    async fn preload_w3d_models(&mut self) -> Result<()> {
        info!("Pre-loading W3D unit models...");
        let Some(asset_manager_arc) = get_asset_manager() else {
            warn!("Asset manager unavailable; skipping W3D model preloading");
            return Ok(());
        };

        let unit_types: Vec<String> = {
            let manager = asset_manager_arc
                .lock()
                .expect("asset manager mutex poisoned");
            manager
                .get_common_cnc_units()
                .into_iter()
                .map(str::to_string)
                .collect()
        };

        for unit_type in unit_types {
            if let Err(err) = load_cnc_unit_model(&unit_type).await {
                // Keep preload non-fatal: template/model aliases can differ per mod/map pack.
                warn!("Failed to preload model for '{}': {}", unit_type, err);
                continue;
            }

            let Some(manager_arc) = get_asset_manager() else {
                continue;
            };
            let manager = manager_arc.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(model) = manager.get_cached_model(&unit_type) {
                self.loaded_models
                    .insert(unit_type.clone(), Arc::new(model));
            } else {
                warn!(
                    "Model '{}' loaded but not found in cache after preload",
                    unit_type
                );
            }
        }

        info!(
            "W3D model preloading complete (cached: {})",
            self.loaded_models.len()
        );
        Ok(())
    }

    fn get_w3d_shader() -> &'static str {
        r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.color = model.color;
    out.clip_position = uniforms.view_proj * vec4<f32>(model.position, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
"#
    }

    fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
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

    /// Render W3D models (no fallbacks - faithful to C++ version)
    #[allow(dead_code)] // Legacy stub: superseded by CncGameEngine render pipeline
    fn render_w3d_objects(&mut self) -> Result<()> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Update uniforms
        let view_proj = self.projection_matrix * self.view_matrix;
        let uniforms = Uniforms {
            view_proj: view_proj.to_cols_array_2d(),
        };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.8, // Desert sand color like C&C maps
                            g: 0.7,
                            b: 0.4,
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
            render_pass.set_bind_group(0, &self.bind_group, &[]);

            // Render game objects (faithful to C++ version - no fallbacks)
            self.render_game_objects(&mut render_pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        present_surface_texture(output);

        Ok(())
    }

    #[allow(dead_code)] // Legacy stub: superseded by CncGameEngine render pipeline
    fn render_game_objects<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        for obj in self.game_logic.get_objects().values() {
            if obj.is_alive() {
                self.render_object(obj, render_pass);
            }
        }
    }

    #[allow(dead_code)] // Legacy stub: superseded by CncGameEngine render pipeline
    fn render_object<'a>(&'a self, _obj: &Object, _render_pass: &mut wgpu::RenderPass<'a>) {
        // This Win32GameEngine render method is now deprecated
        // Since we fixed the architecture to use only CncGameEngine which has proper rendering
        log::trace!(
            "Win32GameEngine render_object called (deprecated - using CncGameEngine instead)"
        );
    }
}

#[async_trait]
impl GameEngine for Win32GameEngine {
    /// init() - Initialize all game systems (matching C++ GameEngine::init())
    async fn init(&mut self, args: &[String]) -> Result<()> {
        info!("Initializing Win32GameEngine systems...");

        // Parse command line arguments (matching C++ pattern)
        let mut _windowed = false;
        for arg in args {
            if arg == "-win" || arg == "-windowed" {
                _windowed = true;
            }
        }

        // Pre-load W3D models (matching C++ asset initialization)
        self.preload_w3d_models().await?;

        // Initialize game logic (matching C++ subsystem initialization)
        self.game_logic = Self::create_game_logic();

        info!("Win32GameEngine initialization complete");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down Win32GameEngine...");
        self.is_quitting = true;

        if let Some(ref mut manager) = self.subsystem_manager {
            manager.shutdown_all().await?;
        }

        Ok(())
    }

    fn get_name(&self) -> &str {
        "Win32GameEngine"
    }

    fn is_initialized(&self) -> bool {
        self.is_active
    }

    fn get_subsystem_manager(&self) -> Option<&SubsystemManager> {
        self.subsystem_manager.as_ref()
    }

    fn get_subsystem_manager_mut(&mut self) -> Option<&mut SubsystemManager> {
        self.subsystem_manager.as_mut()
    }

    /// execute() - Main game loop (matching C++ GameEngine::execute())
    async fn execute(&mut self) -> Result<()> {
        info!("Starting main game loop (Win32GameEngine::execute)...");

        // For the factory pattern foundation, just indicate success
        // The actual event loop is handled in main() to avoid Send issues
        info!("✅ Win32 game engine is ready to execute");

        Ok(())
    }
}

/// Static factory methods for Win32GameEngine
impl Win32GameEngine {
    /// Factory method to create a Win32 game engine instance (matches C++ CreateGameEngine)
    pub async fn create_boxed_game_engine(window: Arc<Window>) -> Result<Box<dyn GameEngine>> {
        info!("Creating Win32GameEngine via factory method...");
        let engine = Win32GameEngine::create_game_engine(window).await?;
        Ok(Box::new(engine) as Box<dyn GameEngine>)
    }

    /// create_game_logic() - Factory method (matching C++ createGameLogic())
    pub fn create_game_logic() -> GameLogic {
        GameLogic::initialize()
    }

    /// Update method (matching C++ GameEngine::update())
    pub fn update(&mut self, dt: f32) -> Result<()> {
        self.game_logic.update_with_dt(dt);
        Ok(())
    }
}
