use font8x8::{UnicodeFonts, BASIC_FONTS};
use glam::Mat4;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use ww3d_gpu::present_surface_texture;

/// Color definition matching C++ UI colors
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    // Original C&C Generals UI Colors
    pub const UI_BACKGROUND: Self = Self {
        r: 0.1,
        g: 0.1,
        b: 0.1,
        a: 0.9,
    };
    pub const UI_BUTTON_NORMAL: Self = Self {
        r: 0.3,
        g: 0.3,
        b: 0.3,
        a: 1.0,
    };
    pub const UI_BUTTON_HOVER: Self = Self {
        r: 0.4,
        g: 0.4,
        b: 0.4,
        a: 1.0,
    };
    pub const UI_BUTTON_PRESSED: Self = Self {
        r: 0.2,
        g: 0.2,
        b: 0.2,
        a: 1.0,
    };
    pub const UI_TEXT: Self = Self {
        r: 0.9,
        g: 0.9,
        b: 0.9,
        a: 1.0,
    };
    pub const UI_TEXT_SELECTED: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    pub const UI_HEALTH_GREEN: Self = Self {
        r: 0.0,
        g: 0.8,
        b: 0.0,
        a: 1.0,
    };
    pub const UI_HEALTH_YELLOW: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    pub const UI_HEALTH_RED: Self = Self {
        r: 0.8,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
}

impl From<Color> for [f32; 4] {
    fn from(color: Color) -> Self {
        [color.r, color.g, color.b, color.a]
    }
}

/// UI Vertex for WGPU rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UIVertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
    color: [f32; 4],
}

impl UIVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x3, // position
        1 => Float32x2, // tex_coords
        2 => Float32x4, // color
    ];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UIVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

/// UI Draw Command representing a single render operation
pub struct UIDrawCommand {
    pub vertices: Vec<UIVertex>,
    pub indices: Vec<u16>,
    pub texture_id: Option<u32>,
    pub clip_rect: Option<(f32, f32, f32, f32)>, // x, y, width, height
}

/// Font glyph information
#[derive(Debug, Clone)]
pub struct Glyph {
    pub texture_coords: (f32, f32, f32, f32), // u1, v1, u2, v2
    pub size: (f32, f32),
    pub bearing: (f32, f32),
    pub advance: f32,
}

/// Font data for text rendering
pub struct Font {
    pub texture_id: u32,
    pub glyphs: HashMap<char, Glyph>,
    pub line_height: f32,
    pub base_size: f32,
}

/// Texture resource
pub struct UITexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub width: u32,
    pub height: u32,
}

/// Main WGPU UI Renderer
pub struct WgpuUIRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: Arc<wgpu::Surface<'static>>,
    config: wgpu::SurfaceConfiguration,

    // Rendering pipeline
    ui_pipeline: wgpu::RenderPipeline,

    // Uniform buffers
    projection_buffer: wgpu::Buffer,
    projection_bind_group: wgpu::BindGroup,

    // Resources
    textures: HashMap<u32, UITexture>,
    texture_bind_groups: HashMap<u32, wgpu::BindGroup>,
    _default_texture: UITexture,
    default_texture_bind_group: wgpu::BindGroup,
    fonts: HashMap<String, Font>,
    next_texture_id: u32,

    // Vertex/Index buffers
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    max_vertices: usize,
    max_indices: usize,

    // Screen dimensions
    screen_width: u32,
    screen_height: u32,
}

impl WgpuUIRenderer {
    pub async fn new(window: &winit::window::Window) -> Result<Self, Box<dyn std::error::Error>> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = Arc::new(unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::from_window(window)?)
        }?);

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
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

        // Create shaders
        let ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/ui.wgsl").into()),
        });

        // Create projection matrix buffer
        let projection_matrix =
            Self::create_projection_matrix(size.width as f32, size.height as f32);
        let projection_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Projection Buffer"),
            contents: bytemuck::cast_slice(&projection_matrix.to_cols_array()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create bind group layout for projection matrix
        let projection_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Projection Bind Group Layout"),
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
            });

        let projection_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Projection Bind Group"),
            layout: &projection_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: projection_buffer.as_entire_binding(),
            }],
        });

        // Create texture bind group layout
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // Create render pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("UI Pipeline Layout"),
            bind_group_layouts: &[&projection_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &ui_shader,
                entry_point: Some("vs_main"),
                buffers: &[UIVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &ui_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // No culling for UI
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

        // Create a default 1x1 white texture so untextured UI draws still satisfy
        // shader binding requirements.
        let default_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("UI Default White Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                aspect: wgpu::TextureAspect::All,
                texture: &default_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let default_view = default_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let default_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("UI Default Texture Bind Group"),
            layout: &ui_pipeline.get_bind_group_layout(1),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&default_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&default_sampler),
                },
            ],
        });

        // Create vertex and index buffers
        let max_vertices = 65536;
        let max_indices = 98304; // 1.5x vertices for typical UI geometry

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("UI Vertex Buffer"),
            size: (max_vertices * std::mem::size_of::<UIVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("UI Index Buffer"),
            size: (max_indices * std::mem::size_of::<u16>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            device,
            queue,
            surface,
            config,
            ui_pipeline,
            projection_buffer,
            projection_bind_group,
            textures: HashMap::new(),
            texture_bind_groups: HashMap::new(),
            _default_texture: UITexture {
                texture: default_texture,
                view: default_view,
                sampler: default_sampler,
                width: 1,
                height: 1,
            },
            default_texture_bind_group,
            fonts: HashMap::new(),
            next_texture_id: 1,
            vertex_buffer,
            index_buffer,
            max_vertices,
            max_indices,
            screen_width: size.width,
            screen_height: size.height,
        })
    }

    /// Create orthographic projection matrix for UI rendering
    fn create_projection_matrix(width: f32, height: f32) -> Mat4 {
        // Create orthographic projection from (0,0) to (width, height)
        // with Z from -1.0 to 1.0
        Mat4::orthographic_rh(0.0, width, height, 0.0, -1.0, 1.0)
    }

    /// Resize the renderer when window size changes
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.screen_width = new_size.width;
            self.screen_height = new_size.height;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);

            // Update projection matrix
            let projection_matrix =
                Self::create_projection_matrix(new_size.width as f32, new_size.height as f32);
            self.queue.write_buffer(
                &self.projection_buffer,
                0,
                bytemuck::cast_slice(&projection_matrix.to_cols_array()),
            );
        }
    }

    /// Load a texture from raw RGBA data
    pub fn load_texture(&mut self, width: u32, height: u32, data: &[u8]) -> u32 {
        let texture_id = self.next_texture_id;
        self.next_texture_id += 1;

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("UI Texture {}", texture_id)),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group for this texture
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Texture Bind Group"),
            layout: &self.ui_pipeline.get_bind_group_layout(1),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        self.textures.insert(
            texture_id,
            UITexture {
                texture,
                view,
                sampler,
                width,
                height,
            },
        );

        self.texture_bind_groups.insert(texture_id, bind_group);

        texture_id
    }

    /// Render UI draw commands
    pub fn render(&mut self, commands: &[UIDrawCommand]) -> Result<(), Box<dyn std::error::Error>> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("UI Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear - UI renders over game
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.ui_pipeline);
            render_pass.set_bind_group(0, &self.projection_bind_group, &[]);
            render_pass.set_bind_group(1, &self.default_texture_bind_group, &[]);

            for command in commands {
                if command.vertices.is_empty() || command.indices.is_empty() {
                    continue;
                }

                // Update vertex buffer
                if command.vertices.len() <= self.max_vertices {
                    self.queue.write_buffer(
                        &self.vertex_buffer,
                        0,
                        bytemuck::cast_slice(&command.vertices),
                    );
                }

                // Update index buffer
                if command.indices.len() <= self.max_indices {
                    self.queue.write_buffer(
                        &self.index_buffer,
                        0,
                        bytemuck::cast_slice(&command.indices),
                    );
                }

                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

                // Bind command texture when available, otherwise keep default white texture.
                let texture_bind_group = command
                    .texture_id
                    .and_then(|texture_id| self.texture_bind_groups.get(&texture_id))
                    .unwrap_or(&self.default_texture_bind_group);
                render_pass.set_bind_group(1, texture_bind_group, &[]);

                // Draw the indexed geometry
                render_pass.draw_indexed(0..command.indices.len() as u32, 0, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        present_surface_texture(output);

        Ok(())
    }

    /// Create a rectangle draw command
    pub fn create_rect(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: Color,
    ) -> UIDrawCommand {
        let vertices = vec![
            UIVertex {
                position: [x, y, 0.0],
                tex_coords: [0.0, 0.0],
                color: color.into(),
            },
            UIVertex {
                position: [x + width, y, 0.0],
                tex_coords: [1.0, 0.0],
                color: color.into(),
            },
            UIVertex {
                position: [x + width, y + height, 0.0],
                tex_coords: [1.0, 1.0],
                color: color.into(),
            },
            UIVertex {
                position: [x, y + height, 0.0],
                tex_coords: [0.0, 1.0],
                color: color.into(),
            },
        ];

        let indices = vec![0, 1, 2, 0, 2, 3];

        UIDrawCommand {
            vertices,
            indices,
            texture_id: None,
            clip_rect: None,
        }
    }

    /// Create a textured rectangle draw command (used for icons/buttons).
    pub fn create_textured_rect(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        texture_id: u32,
    ) -> UIDrawCommand {
        // Textures in the original C++ shell are authored with their own color.
        // Keep vertex tint white so we don't darken artwork.
        let mut cmd = self.create_rect(x, y, width, height, Color::WHITE);
        cmd.texture_id = Some(texture_id);
        cmd
    }

    pub fn texture_size(&self, texture_id: u32) -> Option<(u32, u32)> {
        self.textures
            .get(&texture_id)
            .map(|texture| (texture.width, texture.height))
    }

    /// Create a text draw command using an 8x8 bitmap font fallback.
    pub fn create_text(
        &self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        color: Color,
    ) -> UIDrawCommand {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut cursor_x = x;
        let mut cursor_y = y - font_size;
        let pixel = (font_size.max(8.0) / 8.0).max(1.0);
        let advance = pixel * 8.0 + pixel;

        for ch in text.chars() {
            if ch == '\n' {
                cursor_x = x;
                cursor_y += font_size + pixel;
                continue;
            }

            let glyph = BASIC_FONTS.get(ch).or_else(|| BASIC_FONTS.get('?'));
            let Some(bitmap) = glyph else {
                cursor_x += advance;
                continue;
            };

            for (row, bits) in bitmap.iter().enumerate() {
                for col in 0..8 {
                    if (bits >> col) & 1 == 0 {
                        continue;
                    }

                    let px = cursor_x + col as f32 * pixel;
                    let py = cursor_y + row as f32 * pixel;
                    let base = vertices.len() as u16;

                    vertices.push(UIVertex {
                        position: [px, py, 0.0],
                        tex_coords: [0.0, 0.0],
                        color: color.into(),
                    });
                    vertices.push(UIVertex {
                        position: [px + pixel, py, 0.0],
                        tex_coords: [1.0, 0.0],
                        color: color.into(),
                    });
                    vertices.push(UIVertex {
                        position: [px + pixel, py + pixel, 0.0],
                        tex_coords: [1.0, 1.0],
                        color: color.into(),
                    });
                    vertices.push(UIVertex {
                        position: [px, py + pixel, 0.0],
                        tex_coords: [0.0, 1.0],
                        color: color.into(),
                    });
                    indices.extend_from_slice(&[
                        base,
                        base + 1,
                        base + 2,
                        base,
                        base + 2,
                        base + 3,
                    ]);
                }
            }

            cursor_x += advance;
        }

        UIDrawCommand {
            vertices,
            indices,
            texture_id: None,
            clip_rect: None,
        }
    }

    /// Get screen dimensions
    pub fn get_screen_size(&self) -> (u32, u32) {
        (self.screen_width, self.screen_height)
    }
}
