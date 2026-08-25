//! wgpu analog of `W3DShaderManager::startRenderToTexture` / `endRenderToTexture`
//! + `filterPreRender` / `filterPostRender`.
//!
//! Live Display draws the 3D scene into an offscreen dest texture when a
//! viewport filter is active, then composites:
//! - `ScreenBWFilter` desaturate (red/green death-cam tints)
//! - `ScreenMotionBlurFilter` accumulated viewport copies (`MAX_COUNT=60`)
//! - `ScreenCrossFadeFilter` fade-pattern / `ST_MASK_TEXTURE`

use crate::display::view::{FilterMode, FilterType, ViewFilterComposite};
use std::sync::Mutex;

/// C++ `ScreenMotionBlurFilter::MAX_COUNT`.
pub const MOTION_BLUR_MAX_COUNT: i32 = 60;
const COUNT_STEP: i32 = 5;

struct ShaderFilterGpu {
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    scene_tex: wgpu::Texture,
    scene_view: wgpu::TextureView,
    prev_tex: wgpu::Texture,
    prev_view: wgpu::TextureView,
    mask_tex: wgpu::Texture,
    mask_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    params: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
    bind_layout: wgpu::BindGroupLayout,
    mb_count: i32,
    mb_decrement: bool,
    last_kind: FilterType,
}

static FILTER_GPU: Mutex<Option<ShaderFilterGpu>> = Mutex::new(None);

fn needs_rtt(composite: &ViewFilterComposite) -> bool {
    composite.filter != FilterType::Null && composite.fade > 0.0
}

fn make_color_target(
    device: &wgpu::Device,
    label: &str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn make_radial_mask(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView) {
    // Procedural analog of `exmask_g.tga` when the asset is missing.
    const N: u32 = 64;
    let mut pixels = vec![0u8; (N * N * 4) as usize];
    let mid = (N as f32 - 1.0) * 0.5;
    for y in 0..N {
        for x in 0..N {
            let dx = x as f32 - mid;
            let dy = y as f32 - mid;
            let d = (dx * dx + dy * dy).sqrt() / mid;
            let a = (1.0 - d.clamp(0.0, 1.0)).powf(1.35);
            let i = ((y * N + x) * 4) as usize;
            let v = (a * 255.0) as u8;
            pixels[i] = v;
            pixels[i + 1] = v;
            pixels[i + 2] = v;
            pixels[i + 3] = v;
        }
    }
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ST_MASK_TEXTURE fade-pattern"),
        size: wgpu::Extent3d {
            width: N,
            height: N,
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
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(N * 4),
            rows_per_image: Some(N),
        },
        wgpu::Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
    );
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn try_load_fade_mask(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView) {
    // C++ `ScreenCrossFadeFilter::init` loads `exmask_g.tga`.
    let candidates = [
        "art/textures/exmask_g.tga",
        "ART/Textures/exmask_g.tga",
        "exmask_g.tga",
        "Data/English/Art/Textures/exmask_g.tga",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(img) = image::load_from_memory(&bytes) {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let tex = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("exmask_g.tga ST_MASK_TEXTURE"),
                    size: wgpu::Extent3d {
                        width: w,
                        height: h,
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
                        texture: &tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &rgba,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * w),
                        rows_per_image: Some(h),
                    },
                    wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                );
                let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                return (tex, view);
            }
        }
    }
    make_radial_mask(device, queue)
}

impl ShaderFilterGpu {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let (scene_tex, scene_view) =
            make_color_target(device, "W3D filter scene RTT", width, height, format);
        let (prev_tex, prev_view) =
            make_color_target(device, "W3D filter previous scene", width, height, format);
        let (mask_tex, mask_view) = try_load_fade_mask(device, queue);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("W3D filter sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("W3D filter params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("W3D filter bind layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("W3D filter shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader_filter.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("W3D filter pipeline layout"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("W3D filter composite"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Self {
            width: width.max(1),
            height: height.max(1),
            format,
            scene_tex,
            scene_view,
            prev_tex,
            prev_view,
            mask_tex,
            mask_view,
            sampler,
            params,
            pipeline,
            bind_layout,
            mb_count: 0,
            mb_decrement: false,
            last_kind: FilterType::Null,
        }
    }

    fn ensure(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) {
        let width = width.max(1);
        let height = height.max(1);
        if self.width == width && self.height == height && self.format == format {
            return;
        }
        *self = Self::new(device, queue, format, width, height);
    }
}

/// C++ `W3DShaderManager::startRenderToTexture` — scene dest when a filter is live.
pub fn start_render_to_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    composite: &ViewFilterComposite,
) -> Option<wgpu::TextureView> {
    if !needs_rtt(composite) {
        return None;
    }
    let mut slot = FILTER_GPU.lock().ok()?;
    if slot.is_none() {
        *slot = Some(ShaderFilterGpu::new(device, queue, format, width, height));
    }
    let gpu = slot.as_mut()?;
    gpu.ensure(device, queue, format, width, height);
    if gpu.last_kind != composite.filter {
        gpu.mb_count = 0;
        gpu.mb_decrement = false;
        gpu.last_kind = composite.filter;
    }
    Some(gpu.scene_view.clone())
}

/// C++ `W3DShaderManager::endRenderToTexture` + `filterPostRender`.
pub fn filter_post_render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    dest: &wgpu::TextureView,
    composite: &ViewFilterComposite,
) {
    if !needs_rtt(composite) {
        return;
    }
    let Ok(mut slot) = FILTER_GPU.lock() else {
        return;
    };
    let Some(gpu) = slot.as_mut() else {
        return;
    };

    let (kind, mode, additive) = match composite.filter {
        FilterType::BlackAndWhite => {
            let mode = match composite.mode {
                FilterMode::BWRedAndWhite => 1.0,
                FilterMode::BWGreenAndWhite => 2.0,
                _ => 0.0,
            };
            (0.0f32, mode, 0.0f32)
        }
        FilterType::MotionBlur => {
            let additive = matches!(
                composite.mode,
                FilterMode::MBInAndOutSaturate
                    | FilterMode::MBInSaturate
                    | FilterMode::MBOutSaturate
            );
            let pan = matches!(
                composite.mode,
                FilterMode::MBPanAlpha
                    | FilterMode::MBPanAlpha1
                    | FilterMode::MBPanAlpha2
                    | FilterMode::MBPanAlpha3
                    | FilterMode::MBEndPanAlpha
            );
            // C++ `ScreenMotionBlurFilter::postRender`: pan uses scrollDelta
            // for m_maxCount; zoom-in/out steps COUNT_STEP and lookAts at peak.
            if !pan {
                if gpu.mb_decrement {
                    gpu.mb_count = (gpu.mb_count - COUNT_STEP).max(1);
                    if gpu.mb_count <= 1 {
                        gpu.mb_decrement = false;
                    }
                } else {
                    gpu.mb_count += COUNT_STEP;
                    if gpu.mb_count >= MOTION_BLUR_MAX_COUNT {
                        gpu.mb_count = MOTION_BLUR_MAX_COUNT;
                        gpu.mb_decrement = true;
                        let do_zoom_to = matches!(
                            composite.mode,
                            FilterMode::MBInAndOutAlpha | FilterMode::MBInAndOutSaturate
                        );
                        if do_zoom_to {
                            if let Some(pos) = composite.zoom_to {
                                crate::display::view::with_tactical_view(|view| {
                                    view.look_at(&pos);
                                });
                                crate::display::view::queue_motion_blur_zoom_look_at(pos);
                            }
                        }
                    }
                }
            }
            (
                1.0f32,
                if pan { 1.0 } else { 0.0 },
                if additive { 1.0 } else { 0.0 },
            )
        }
        FilterType::Crossfade => (2.0f32, 0.0, 0.0),
        FilterType::Null => return,
    };

    let fade = composite.fade.clamp(0.0, 1.0);
    let bytes: [f32; 8] = [
        fade,
        kind,
        mode,
        gpu.mb_count as f32,
        composite.scroll_delta.x,
        composite.scroll_delta.y,
        fade,
        additive,
    ];
    queue.write_buffer(&gpu.params, 0, bytemuck::cast_slice(&bytes));

    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("W3D filter composite bind"),
        layout: &gpu.bind_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&gpu.scene_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&gpu.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&gpu.prev_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&gpu.mask_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: gpu.params.as_entire_binding(),
            },
        ],
    });

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("W3D filterPostRender"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dest,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(&gpu.pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.draw(0..3, 0..1);
    }

    // Keep last scene for ScreenCrossFadeFilter two-capture mix.
    encoder.copy_texture_to_texture(
        gpu.scene_tex.as_image_copy(),
        gpu.prev_tex.as_image_copy(),
        wgpu::Extent3d {
            width: gpu.width,
            height: gpu.height,
            depth_or_array_layers: 1,
        },
    );
}

/// C++ `W3DShaderManager::endRenderToTexture` — dest view of the last scene capture.
pub fn end_render_to_texture() -> Option<wgpu::TextureView> {
    FILTER_GPU
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|gpu| gpu.scene_view.clone()))
}

/// Live `render_pipeline` hook: the 3D scene already sits on `dest_texture`.
/// Copy it into the leftover RTT, then `filterPostRender` onto `dest_view`.
pub fn composite_live_view_filter(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    dest_view: &wgpu::TextureView,
    dest_texture: &wgpu::Texture,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    composite: &ViewFilterComposite,
) {
    if !needs_rtt(composite) {
        return;
    }
    let _ = start_render_to_texture(device, queue, format, width, height, composite);
    {
        let Ok(slot) = FILTER_GPU.lock() else {
            return;
        };
        let Some(gpu) = slot.as_ref() else {
            return;
        };
        let width = width.max(1);
        let height = height.max(1);
        if gpu.width != width || gpu.height != height {
            return;
        }
        encoder.copy_texture_to_texture(
            dest_texture.as_image_copy(),
            gpu.scene_tex.as_image_copy(),
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }
    filter_post_render(device, queue, encoder, dest_view, composite);
}
