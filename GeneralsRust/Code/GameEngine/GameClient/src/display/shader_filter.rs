//! wgpu analog of `W3DShaderManager::startRenderToTexture` / `endRenderToTexture`
//! + `filterPreRender` / `filterPostRender`.
//!
//! Live Display draws the 3D scene into an offscreen dest texture when a
//! viewport filter is active, then composites:
//! - `ScreenBWFilter` desaturate (red/green death-cam tints)
//! - `ScreenMotionBlurFilter` accumulated viewport copies (`MAX_COUNT=60`)
//! - `ScreenCrossFadeFilter` fade-pattern / `ST_MASK_TEXTURE`

use crate::display::view::{FilterMode, FilterType, ViewFilterComposite};
use crate::display::view_filter::FilterPostRender;
/// C++ `ScreenMotionBlurFilter::MAX_COUNT`.
pub use crate::display::view_filter::MOTION_BLUR_MAX_COUNT;

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
}

fn needs_rtt(composite: &ViewFilterComposite) -> bool {
    composite.filter != FilterType::Null
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
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
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
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
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
            multiview_mask: None,
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

fn draw_filter_gpu(
    gpu: &mut ShaderFilterGpu,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    dest: &wgpu::TextureView,
    composite: &ViewFilterComposite,
    post: &FilterPostRender,
) {
    let (kind, mode, additive) = match composite.filter {
        FilterType::BlackAndWhite => {
            let mode = match composite.mode {
                FilterMode::BWRedAndWhite => 1.0,
                FilterMode::BWGreenAndWhite => 2.0,
                _ => 0.0,
            };
            (0.0f32, mode, 0.0f32)
        }
        FilterType::MotionBlur => (
            1.0f32,
            if post.pan { 1.0 } else { 0.0 },
            if post.additive { 1.0 } else { 0.0 },
        ),
        FilterType::Crossfade => (2.0f32, 0.0, 0.0),
        FilterType::Null => return,
    };

    let fade = post.fade.clamp(0.0, 1.0);
    let bytes: [f32; 8] = [
        fade,
        kind,
        mode,
        post.motion_blur_count as f32,
        post.scroll_delta.x,
        post.scroll_delta.y,
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
            multiview_mask: None,
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

/// GPU resources belong to one renderer. Resizing never resets CPU filter phase.
#[derive(Default)]
pub struct ShaderFilterResources {
    gpu: Option<ShaderFilterGpu>,
    has_scene_capture: bool,
}
impl ShaderFilterResources {
    pub fn has_scene_capture_for(
        &self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> bool {
        self.has_scene_capture
            && self.gpu.as_ref().is_some_and(|gpu| {
                gpu.width == width.max(1) && gpu.height == height.max(1) && gpu.format == format
            })
    }

    /// The receiving Display renders its complete scene into this target before
    /// finishing the filter phase. The returned view is a clone of that owner's
    /// texture view; it never selects another Display's allocations.
    pub(crate) fn scene_view(&self) -> Option<wgpu::TextureView> {
        self.gpu.as_ref().map(|gpu| gpu.scene_view.clone())
    }

    /// Finish the direct RTT path used by `Display::draw_with_snow_state`.
    pub(crate) fn finish_render_to_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dest: &wgpu::TextureView,
        composite: &ViewFilterComposite,
        post: &FilterPostRender,
    ) -> bool {
        if !needs_rtt(composite) {
            return false;
        }
        let Some(gpu) = self.gpu.as_mut() else {
            return false;
        };
        draw_filter_gpu(gpu, device, queue, encoder, dest, composite, post);
        self.has_scene_capture = true;
        true
    }

    /// Native caller invokes this before advancing its CPU postRender owner.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        composite: &ViewFilterComposite,
    ) -> bool {
        if !needs_rtt(composite) {
            return false;
        }
        if !self.gpu.as_ref().is_some_and(|gpu| {
            gpu.width == width.max(1) && gpu.height == height.max(1) && gpu.format == format
        }) {
            self.has_scene_capture = false;
        }
        if self.gpu.is_none() {
            self.gpu = Some(ShaderFilterGpu::new(device, queue, format, width, height));
        }
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.ensure(device, queue, format, width, height);
        }
        self.gpu.is_some()
    }
    /// Synchronous explicit native render: no TLS, tactical View or CPU advancement.
    pub fn composite_live_view_filter(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dest: &wgpu::TextureView,
        texture: &wgpu::Texture,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        composite: &ViewFilterComposite,
        post: &FilterPostRender,
    ) -> bool {
        self.composite_retained_view_filter(
            device, queue, encoder, dest, texture, format, width, height, composite, post, true,
        )
    }

    /// Reuse the successful prior scene for source skipRender offers. A missing
    /// or resized capture always refreshes, never samples uninitialized history.
    pub fn composite_retained_view_filter(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        dest: &wgpu::TextureView,
        texture: &wgpu::Texture,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        composite: &ViewFilterComposite,
        post: &FilterPostRender,
        refresh_scene: bool,
    ) -> bool {
        if !self.prepare(device, queue, format, width, height, composite) {
            return false;
        }
        let refresh_scene = refresh_scene || !self.has_scene_capture;
        let gpu = self.gpu.as_mut().expect("prepared resources");
        if refresh_scene {
            encoder.copy_texture_to_texture(
                texture.as_image_copy(),
                gpu.scene_tex.as_image_copy(),
                wgpu::Extent3d {
                    width: width.max(1),
                    height: height.max(1),
                    depth_or_array_layers: 1,
                },
            );
            self.has_scene_capture = true;
        }
        draw_filter_gpu(gpu, device, queue, encoder, dest, composite, post);
        true
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "shader_filter_owner_tests.rs"]
mod owner_tests;
