/*
** Command & Conquer Generals Zero Hour(tm) - Simple Rendering Demo
** This is a minimal demo that shows the game engine can render 3D models
** Without the complex asset loading and initialization that's causing hangs
*/

use anyhow::Result;
use glam::{Mat4, Vec3};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{Key, NamedKey},
    window::WindowBuilder,
};
use ww3d_gpu::present_surface_texture;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
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

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    fn new() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
        }
    }

    fn update_view_proj(&mut self, view_proj: Mat4) {
        self.view_proj = view_proj.to_cols_array_2d();
    }
}

struct SimpleRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,

    // Camera system
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    camera_uniform: CameraUniform,

    // Models
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,

    // Camera position
    camera_pos: Vec3,
    camera_angle: f32,
}

impl SimpleRenderer {
    async fn new(window: Arc<winit::window::Window>) -> Result<Self> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window)?;

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

        // Create camera uniform buffer
        let mut camera_uniform = CameraUniform::new();
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                label: Some("camera_bind_group_layout"),
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        // Create several C&C unit models as colored cubes scattered around the map
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut index_offset = 0u32;

        let unit_positions = vec![
            (Vec3::new(-100.0, 0.0, -100.0), [1.0, 0.0, 0.0]), // Red cube - USA Ranger
            (Vec3::new(-90.0, 0.0, -100.0), [1.0, 0.0, 0.0]),  // Red cube - USA Ranger
            (Vec3::new(-110.0, 0.0, -90.0), [0.8, 0.8, 0.0]),  // Yellow cube - USA Humvee
            (Vec3::new(100.0, 0.0, 100.0), [0.0, 1.0, 0.0]),   // Green cube - GLA Soldier
            (Vec3::new(90.0, 0.0, 100.0), [0.0, 1.0, 0.0]),    // Green cube - GLA Soldier
            (Vec3::new(110.0, 0.0, 90.0), [0.0, 0.8, 0.0]),    // Dark Green - GLA Technical
            (Vec3::new(-200.0, 0.0, -200.0), [0.0, 0.0, 1.0]), // Blue cube - Command Center
            (Vec3::new(200.0, 0.0, 200.0), [1.0, 0.0, 1.0]),   // Magenta cube - Command Center
        ];

        for (position, color) in unit_positions {
            let size = 5.0;
            let cube_vertices = [
                // Front face
                Vertex {
                    position: [position.x - size, position.y - size, position.z + size],
                    color,
                },
                Vertex {
                    position: [position.x + size, position.y - size, position.z + size],
                    color,
                },
                Vertex {
                    position: [position.x + size, position.y + size, position.z + size],
                    color,
                },
                Vertex {
                    position: [position.x - size, position.y + size, position.z + size],
                    color,
                },
                // Back face
                Vertex {
                    position: [position.x + size, position.y - size, position.z - size],
                    color,
                },
                Vertex {
                    position: [position.x - size, position.y - size, position.z - size],
                    color,
                },
                Vertex {
                    position: [position.x - size, position.y + size, position.z - size],
                    color,
                },
                Vertex {
                    position: [position.x + size, position.y + size, position.z - size],
                    color,
                },
            ];

            let cube_indices = [
                0, 1, 2, 2, 3, 0, // Front
                4, 5, 6, 6, 7, 4, // Back
                5, 0, 3, 3, 6, 5, // Left
                1, 4, 7, 7, 2, 1, // Right
                3, 2, 7, 7, 6, 3, // Top
                5, 4, 1, 1, 0, 5, // Bottom
            ];

            vertices.extend_from_slice(&cube_vertices);
            for &index in &cube_indices {
                indices.push(index + index_offset);
            }
            index_offset += 8;
        }

        // Add ground plane
        let ground_size = 300.0;
        let ground_vertices = [
            Vertex {
                position: [-ground_size, -5.0, -ground_size],
                color: [0.2, 0.5, 0.2],
            },
            Vertex {
                position: [ground_size, -5.0, -ground_size],
                color: [0.2, 0.5, 0.2],
            },
            Vertex {
                position: [ground_size, -5.0, ground_size],
                color: [0.2, 0.5, 0.2],
            },
            Vertex {
                position: [-ground_size, -5.0, ground_size],
                color: [0.2, 0.5, 0.2],
            },
        ];
        let ground_indices = [0, 1, 2, 0, 2, 3];

        vertices.extend_from_slice(&ground_vertices);
        for &index in &ground_indices {
            indices.push(index + index_offset);
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let num_indices = indices.len() as u32;

        // Create shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/simple.wgsl").into()),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&camera_bind_group_layout],
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
            cache: None,
            multiview: None,
        });

        // Set initial camera position (RTS-style elevated view)
        let camera_pos = Vec3::new(0.0, 50.0, 100.0);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            render_pipeline,
            camera_buffer,
            camera_bind_group,
            camera_uniform,
            vertex_buffer,
            index_buffer,
            num_indices,
            camera_pos,
            camera_angle: 0.0,
        })
    }

    fn update(&mut self) {
        // Slowly rotate camera around the battlefield
        self.camera_angle += 0.005;
        let radius = 150.0;
        self.camera_pos.x = radius * self.camera_angle.cos();
        self.camera_pos.z = radius * self.camera_angle.sin() + 50.0;
        self.camera_pos.y = 80.0; // Keep elevated RTS view

        // Update camera uniform
        let aspect = self.config.width as f32 / self.config.height as f32;
        let projection = Mat4::perspective_rh(45.0_f32.to_radians(), aspect, 1.0, 1000.0);
        let view = Mat4::look_at_rh(
            self.camera_pos,
            Vec3::new(0.0, 0.0, 0.0), // Look at center of battlefield
            Vec3::Y,
        );
        self.camera_uniform.update_view_proj(projection * view);
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // Update camera uniform buffer
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
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
                            r: 0.5,
                            g: 0.7,
                            b: 1.0,
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
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        present_surface_texture(output);

        Ok(())
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();

    println!("🎮 C&C Generals - Simple Render Demo");
    println!("This demo shows that the graphics pipeline works!");
    println!("You should see colored cubes representing C&C units on a battlefield.");
    println!("Press ESC to exit.");

    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("C&C Generals Zero Hour - Rendering Test")
        .with_inner_size(winit::dpi::LogicalSize::new(1024.0, 768.0))
        .build(&event_loop)?;
    let window = Arc::new(window);

    // Use pollster to block on the async initialization
    let mut renderer = pollster::block_on(SimpleRenderer::new(window.clone()))?;

    event_loop.run(move |event, target| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == window.id() => match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => target.exit(),
            WindowEvent::Resized(physical_size) => {
                renderer.resize(*physical_size);
            }
            WindowEvent::RedrawRequested => {
                renderer.update();
                match renderer.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => renderer.resize(window.inner_size()),
                    Err(wgpu::SurfaceError::OutOfMemory) => target.exit(),
                    Err(e) => eprintln!("Render error: {:?}", e),
                }
            }
            _ => {}
        },
        Event::AboutToWait => {
            window.request_redraw();
        }
        _ => {}
    })?;

    Ok(())
}
