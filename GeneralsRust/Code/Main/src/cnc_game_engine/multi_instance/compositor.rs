//! Grid compositor: samples every instance's tile texture into one labelled
//! grid, presented to a window and/or rendered off-screen for PNG dumps.
//! This is host chrome around the instances, not game presentation: each
//! tile is the engine's own finished frame, copied unchanged.

use super::instance::{InstancePhase, InstanceStatus};
use font8x8::UnicodeFonts;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use winit::window::Window;

/// Instance frames are rendered into the WW3D headless default format.
pub const TILE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
const LABEL_WIDTH: u32 = 480;
const LABEL_LINES: u32 = 2;
const LABEL_HEIGHT: u32 = LABEL_LINES * 10 + 2;

const SHADER: &str = r#"
struct Rect { r: vec4<f32> };
@group(0) @binding(0) var<uniform> rect: Rect;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> VOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0));
    let c = corners[i];
    var o: VOut;
    o.pos = vec4<f32>(mix(rect.r.x, rect.r.z, c.x), mix(rect.r.y, rect.r.w, c.y), 0.0, 1.0);
    o.uv = vec2<f32>(c.x, 1.0 - c.y);
    return o;
}
@fragment fn fs_opaque(v: VOut) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(tex, samp, v.uv).rgb, 1.0);
}
@fragment fn fs_blend(v: VOut) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, v.uv);
}
"#;

struct Quad {
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

struct Tile {
    texture: wgpu::Texture,
    quad: Quad,
    label_texture: wgpu::Texture,
    label_quad: Quad,
    label_text: Vec<String>,
}

struct Pipelines {
    opaque: wgpu::RenderPipeline,
    blend: wgpu::RenderPipeline,
}

pub struct GridCompositor {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: Option<(wgpu::Surface<'static>, wgpu::SurfaceConfiguration)>,
    grid_size: (u32, u32),
    columns: u32,
    rows: u32,
    tiles: Vec<Tile>,
    module: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    pipelines: HashMap<wgpu::TextureFormat, Pipelines>,
}

impl GridCompositor {
    pub fn new(
        gpu: &ww3d_engine::HostGpu,
        window: Option<Arc<Window>>,
        grid_size: (u32, u32),
        columns: u32,
        rows: u32,
        tiles: usize,
        instance_size: (u32, u32),
    ) -> Result<Self, String> {
        let device = Arc::clone(&gpu.device);
        let queue = Arc::clone(&gpu.queue);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("multi-instance grid"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("multi-instance quad"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
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
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
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
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("multi-instance grid"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("multi-instance tile"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("multi-instance label"),
            ..Default::default()
        });
        let quad = |texture: &wgpu::Texture, sampler: &wgpu::Sampler| -> Quad {
            let uniform = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("multi-instance rect"),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("multi-instance quad"),
                layout: &bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            });
            Quad {
                uniform,
                bind_group,
            }
        };
        let tiles = (0..tiles)
            .map(|index| {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(&format!("multi-instance tile {index}")),
                    size: wgpu::Extent3d {
                        width: instance_size.0.max(1),
                        height: instance_size.1.max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: TILE_FORMAT,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_DST
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                let label_texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(&format!("multi-instance label {index}")),
                    size: wgpu::Extent3d {
                        width: LABEL_WIDTH,
                        height: LABEL_HEIGHT,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                let tile_quad = quad(&texture, &linear);
                let label_quad = quad(&label_texture, &nearest);
                Tile {
                    texture,
                    quad: tile_quad,
                    label_texture,
                    label_quad,
                    label_text: Vec::new(),
                }
            })
            .collect();

        let surface = match window {
            Some(window) => {
                let size = window.inner_size();
                let surface = gpu
                    .instance
                    .create_surface(window)
                    .map_err(|e| format!("grid surface: {e}"))?;
                let caps = surface.get_capabilities(&gpu.adapter);
                // Tiles hold the engines' encoded output; a non-sRGB swapchain
                // passes it through unchanged.
                let format = caps
                    .formats
                    .iter()
                    .copied()
                    .find(|f| *f == TILE_FORMAT)
                    .or_else(|| caps.formats.iter().copied().find(|f| !f.is_srgb()))
                    .or_else(|| caps.formats.first().copied())
                    .ok_or("grid surface has no formats")?;
                let config = wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format,
                    color_space: wgpu::SurfaceColorSpace::Auto,
                    width: size.width.max(1),
                    height: size.height.max(1),
                    present_mode: wgpu::PresentMode::Fifo,
                    alpha_mode: caps
                        .alpha_modes
                        .first()
                        .copied()
                        .unwrap_or(wgpu::CompositeAlphaMode::Auto),
                    view_formats: vec![],
                    desired_maximum_frame_latency: 2,
                };
                surface.configure(&device, &config);
                Some((surface, config))
            }
            None => None,
        };

        Ok(Self {
            device,
            queue,
            surface,
            grid_size,
            columns,
            rows,
            tiles,
            module,
            layout,
            pipelines: HashMap::new(),
        })
    }

    /// Texture instance `index` copies its finished frames into.
    pub fn tile_texture(&self, index: usize) -> &wgpu::Texture {
        &self.tiles[index].texture
    }

    pub fn resize_surface(&mut self, width: u32, height: u32) {
        if let Some((surface, config)) = self.surface.as_mut() {
            config.width = width.max(1);
            config.height = height.max(1);
            surface.configure(&self.device, config);
        }
    }

    fn pipelines(&mut self, format: wgpu::TextureFormat) -> &Pipelines {
        let device = &self.device;
        let module = &self.module;
        let layout = &self.layout;
        self.pipelines.entry(format).or_insert_with(|| {
            let make = |entry: &str, blend: Option<wgpu::BlendState>| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("multi-instance grid"),
                    layout: Some(layout),
                    vertex: wgpu::VertexState {
                        module,
                        entry_point: Some("vs"),
                        buffers: &[],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module,
                        entry_point: Some(entry),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                })
            };
            Pipelines {
                opaque: make("fs_opaque", None),
                blend: make("fs_blend", Some(wgpu::BlendState::ALPHA_BLENDING)),
            }
        })
    }

    /// Refresh labels and per-cell rectangles for a target of `size` pixels.
    fn prepare(&mut self, statuses: &[InstanceStatus], size: (u32, u32)) {
        let cols = self.columns as f32;
        let rows = self.rows as f32;
        let cell_h_px = size.1 as f32 / rows;
        let label_frac = (LABEL_HEIGHT as f32 * 1.5 / cell_h_px).min(0.5);
        let cell_w_px = size.0 as f32 / cols;
        let label_w_frac = (LABEL_WIDTH as f32 * 1.5 / cell_w_px).min(1.0);
        for (index, tile) in self.tiles.iter_mut().enumerate() {
            let col = (index as u32 % self.columns) as f32;
            let row = (index as u32 / self.columns) as f32;
            let x0 = col / cols * 2.0 - 1.0;
            let x1 = (col + 1.0) / cols * 2.0 - 1.0;
            let y1 = 1.0 - row / rows * 2.0;
            let y0 = 1.0 - (row + 1.0) / rows * 2.0;
            // One-pixel gutter between cells.
            let gx = 1.0 / size.0 as f32;
            let gy = 1.0 / size.1 as f32;
            let rect = [x0 + gx, y0 + gy, x1 - gx, y1 - gy];
            self.queue
                .write_buffer(&tile.quad.uniform, 0, bytemuck::cast_slice(&rect));
            let label_y0 = y1 - (y1 - y0) * label_frac;
            let label_x1 = x0 + (x1 - x0) * label_w_frac;
            let label = [x0 + gx, label_y0, label_x1 - gx, y1 - gy];
            self.queue
                .write_buffer(&tile.label_quad.uniform, 0, bytemuck::cast_slice(&label));

            let Some(status) = statuses.get(index) else {
                continue;
            };
            let text = label_lines(status);
            if text != tile.label_text {
                let rgba = rasterize_label(&text, label_color(status));
                self.queue.write_texture(
                    tile.label_texture.as_image_copy(),
                    &rgba,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(LABEL_WIDTH * 4),
                        rows_per_image: Some(LABEL_HEIGHT),
                    },
                    tile.label_texture.size(),
                );
                tile.label_text = text;
            }
        }
    }

    fn encode(
        &mut self,
        view: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> wgpu::CommandBuffer {
        self.pipelines(format);
        let pipelines = &self.pipelines[&format];
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("multi-instance grid"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("multi-instance grid"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.08,
                            g: 0.08,
                            b: 0.08,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipelines.opaque);
            for tile in &self.tiles {
                pass.set_bind_group(0, &tile.quad.bind_group, &[]);
                pass.draw(0..6, 0..1);
            }
            pass.set_pipeline(&pipelines.blend);
            for tile in &self.tiles {
                pass.set_bind_group(0, &tile.label_quad.bind_group, &[]);
                pass.draw(0..6, 0..1);
            }
        }
        encoder.finish()
    }

    /// Composite into the window's next swapchain image.
    pub fn present(&mut self, statuses: &[InstanceStatus]) -> Result<(), String> {
        let Some((surface, config)) = self.surface.as_ref() else {
            return Ok(());
        };
        let size = (config.width, config.height);
        let format = config.format;
        let frame = match ww3d_gpu::acquire_surface_texture(surface) {
            Ok(frame) => frame,
            Err(ww3d_gpu::GpuError::SurfaceLost | ww3d_gpu::GpuError::SurfaceOutdated) => {
                surface.configure(&self.device, config);
                return Ok(());
            }
            Err(_) => return Ok(()),
        };
        self.prepare(statuses, size);
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let commands = self.encode(&view, format);
        self.queue.submit([commands]);
        self.queue.present(frame);
        Ok(())
    }

    /// Composite off-screen and write `grid-<tag>.png`, one
    /// `tile-<tag>-NN.png` per instance and `grid-<tag>.txt` (statuses).
    pub fn dump(
        &mut self,
        dir: &Path,
        tag: &str,
        statuses: &[InstanceStatus],
    ) -> Result<Vec<PathBuf>, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let (w, h) = self.grid_size;
        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("multi-instance grid dump"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TILE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.prepare(statuses, (w, h));
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let commands = self.encode(&view, TILE_FORMAT);
        self.queue.submit([commands]);

        let mut written = Vec::new();
        let grid_path = dir.join(format!("grid-{tag}.png"));
        self.write_png(&target, &grid_path)?;
        written.push(grid_path);
        for (index, tile) in self.tiles.iter().enumerate() {
            let path = dir.join(format!("tile-{tag}-{index:02}.png"));
            self.write_png(&tile.texture, &path)?;
            written.push(path);
        }
        let text_path = dir.join(format!("grid-{tag}.txt"));
        let text: String = statuses
            .iter()
            .map(|s| format!("#{:02} {} {}\n", s.index, s.scenario, s.one_line()))
            .collect();
        std::fs::write(&text_path, text).map_err(|e| format!("{}: {e}", text_path.display()))?;
        written.push(text_path);
        Ok(written)
    }

    fn write_png(&self, texture: &wgpu::Texture, path: &Path) -> Result<(), String> {
        let (w, h) = (texture.width(), texture.height());
        let row = w * 4;
        let padded =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("multi-instance readback"),
            size: u64::from(padded) * u64::from(h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("multi-instance readback"),
            });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            texture.size(),
        );
        self.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("readback poll: {e}"))?;
        rx.recv()
            .map_err(|_| "readback channel closed".to_string())?
            .map_err(|e| format!("readback map: {e}"))?;
        let data = slice
            .get_mapped_range()
            .map_err(|e| format!("readback range: {e}"))?;
        let bgra = texture.format() == wgpu::TextureFormat::Bgra8Unorm
            || texture.format() == wgpu::TextureFormat::Bgra8UnormSrgb;
        let mut rgba = Vec::with_capacity((row * h) as usize);
        for y in 0..h {
            let start = (y * padded) as usize;
            for px in data[start..start + row as usize].chunks_exact(4) {
                if bgra {
                    rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
                } else {
                    rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
                }
            }
        }
        drop(data);
        buffer.unmap();
        image::RgbaImage::from_raw(w, h, rgba)
            .ok_or("readback size mismatch")?
            .save(path)
            .map_err(|e| format!("{}: {e}", path.display()))
    }
}

fn label_lines(s: &InstanceStatus) -> Vec<String> {
    vec![
        format!("#{:02} {}  [{}]", s.index, s.scenario, s.phase_name()),
        match &s.phase {
            InstancePhase::Failed(reason) => reason.chars().take(60).collect(),
            _ => format!(
                "{} f{} {:.0}fps sim {:.0}/s {:.0}ms",
                s.state, s.logic_frame, s.render_fps, s.sim_rate, s.frame_ms
            ),
        },
    ]
}

fn label_color(s: &InstanceStatus) -> [u8; 4] {
    match s.phase {
        InstancePhase::Failed(_) => [255, 90, 90, 255],
        InstancePhase::Booting => [255, 220, 90, 255],
        InstancePhase::Running if s.state == "InGame" => [140, 255, 140, 255],
        _ => [235, 235, 235, 255],
    }
}

/// 8x8 bitmap text on a translucent strip, `LABEL_LINES` lines.
fn rasterize_label(lines: &[String], color: [u8; 4]) -> Vec<u8> {
    let mut rgba = vec![0u8; (LABEL_WIDTH * LABEL_HEIGHT * 4) as usize];
    for px in rgba.chunks_exact_mut(4) {
        px.copy_from_slice(&[0, 0, 0, 170]);
    }
    for (line_index, line) in lines.iter().take(LABEL_LINES as usize).enumerate() {
        let top = 2 + line_index as u32 * 10;
        for (char_index, ch) in line.chars().enumerate() {
            let left = 2 + char_index as u32 * 8;
            if left + 8 > LABEL_WIDTH {
                break;
            }
            let Some(glyph) = font8x8::BASIC_FONTS.get(ch) else {
                continue;
            };
            for (gy, bits) in glyph.iter().enumerate() {
                for gx in 0..8u32 {
                    if bits & (1 << gx) != 0 {
                        let i = (((top + gy as u32) * LABEL_WIDTH + left + gx) * 4) as usize;
                        rgba[i..i + 4].copy_from_slice(&color);
                    }
                }
            }
        }
    }
    rgba
}
