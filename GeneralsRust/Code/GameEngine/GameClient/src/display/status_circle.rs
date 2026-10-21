//! C++ `W3DStatusCircle::Render` camera-fade overlay.

use gamelogic::scripting::{TFade, get_script_engine};

/// Fullscreen camera-fade overlay produced by `W3DStatusCircle::Render`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraFadeOverlay {
    pub fade: TFade,
    /// C++ `TheScriptEngine->getFadeValue()` in 0..1.
    pub intensity: f32,
    /// Packed ARGB used as the fullscreen quad vertex color (`255 * intensity`).
    pub diffuse: u32,
}

/// Pure C++ fade packing from the driving interpreter's completed observation.
/// No script update or service selection is performed here.
pub fn camera_fade_from_observation(fade: TFade, value: f32) -> Option<CameraFadeOverlay> {
    if fade == TFade::None {
        return None;
    }
    let intensity = value.clamp(0.0, 1.0);
    let channel = (255.0 * intensity) as u32;
    Some(CameraFadeOverlay {
        fade,
        intensity,
        diffuse: (0xff << 24) | (channel << 16) | (channel << 8) | channel,
    })
}

/// Explicit standalone Device compatibility adapter. Native Main frames lend
/// their receiving interpreter observation to `camera_fade_from_observation`.
pub fn render_camera_fade() -> Option<CameraFadeOverlay> {
    let scripts = get_script_engine();
    let guard = scripts.read().ok()?;
    let engine = guard.as_ref()?;
    let (fade, value) = engine.fade_observation();
    camera_fade_from_observation(fade, value)
}

fn overlay_from_packed(fade: u8, intensity: f32, diffuse: u32) -> Option<CameraFadeOverlay> {
    let fade = match fade {
        1 => TFade::Subtract,
        2 => TFade::Add,
        3 => TFade::Saturate,
        4 => TFade::Multiply,
        _ => return None,
    };
    Some(CameraFadeOverlay {
        fade,
        intensity: intensity.clamp(0.0, 1.0),
        diffuse,
    })
}

struct FadeGpu {
    device: wgpu::Device,
    format: wgpu::TextureFormat,
    color: wgpu::Buffer,
    bind: wgpu::BindGroup,
    add: wgpu::RenderPipeline,
    subtract: wgpu::RenderPipeline,
    multiply: wgpu::RenderPipeline,
    saturate: wgpu::RenderPipeline,
}

/// Camera-fade GPU handles owned by the receiving render pipeline.
/// Handles stay on that renderer's thread, including the web backend. A new
/// device or target format rebuilds only this owner's resources.
#[derive(Default)]
pub struct CameraFadeResources {
    gpu: Option<FadeGpu>,
}

impl CameraFadeResources {
    fn for_target(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) -> &FadeGpu {
        if !self
            .gpu
            .as_ref()
            .is_some_and(|gpu| gpu.device == *device && gpu.format == format)
        {
            self.gpu = Some(FadeGpu::new(device, format));
        }
        self.gpu
            .as_ref()
            .expect("receiving fade resources initialized")
    }
}

fn fade_blend(
    src: wgpu::BlendFactor,
    dst: wgpu::BlendFactor,
    op: wgpu::BlendOperation,
) -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: src,
            dst_factor: dst,
            operation: op,
        },
        alpha: wgpu::BlendComponent {
            src_factor: src,
            dst_factor: dst,
            operation: op,
        },
    }
}

impl FadeGpu {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("W3DStatusCircle fade"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}
@group(0) @binding(0) var<uniform> fade_color: vec4<f32>;
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var o: VsOut;
    o.pos = vec4<f32>(p[i], 0.0, 1.0);
    o.color = fade_color;
    return o;
}
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
"#
                .into(),
            ),
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("W3DStatusCircle fade bind"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let color = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("W3DStatusCircle fade color"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("W3DStatusCircle fade group"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: color.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("W3DStatusCircle fade layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let make = |label: &str, blend: wgpu::BlendState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
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
                        blend: Some(blend),
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
            })
        };
        Self {
            device: device.clone(),
            format,
            color,
            bind,
            add: make(
                "W3D fade ADD",
                fade_blend(
                    wgpu::BlendFactor::One,
                    wgpu::BlendFactor::One,
                    wgpu::BlendOperation::Add,
                ),
            ),
            subtract: make(
                "W3D fade SUBTRACT",
                fade_blend(
                    wgpu::BlendFactor::One,
                    wgpu::BlendFactor::One,
                    wgpu::BlendOperation::ReverseSubtract,
                ),
            ),
            multiply: make(
                "W3D fade MULTIPLY",
                fade_blend(
                    wgpu::BlendFactor::Zero,
                    wgpu::BlendFactor::Src,
                    wgpu::BlendOperation::Add,
                ),
            ),
            saturate: make(
                "W3D fade SATURATE",
                fade_blend(
                    wgpu::BlendFactor::Dst,
                    wgpu::BlendFactor::Src,
                    wgpu::BlendOperation::Add,
                ),
            ),
        }
    }
}

/// C++ `W3DStatusCircle::Render` fade quad onto the live 3D dest.
pub fn record_camera_fade(
    resources: &mut CameraFadeResources,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    dest: &wgpu::TextureView,
    format: wgpu::TextureFormat,
    fade: u8,
    intensity: f32,
    diffuse: u32,
) {
    let Some(overlay) = overlay_from_packed(fade, intensity, diffuse) else {
        return;
    };
    record_camera_fade_overlay(resources, device, queue, encoder, dest, format, overlay);
}

/// Draw a frozen `CameraFadeOverlay` with ADD / REVSUBTRACT / 4x SATURATE / MULTIPLY.
pub fn record_camera_fade_overlay(
    resources: &mut CameraFadeResources,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    dest: &wgpu::TextureView,
    format: wgpu::TextureFormat,
    overlay: CameraFadeOverlay,
) {
    if overlay.fade == TFade::None {
        return;
    }
    let gpu = resources.for_target(device, format);
    let a = ((overlay.diffuse >> 24) & 0xff) as f32 / 255.0;
    let r = ((overlay.diffuse >> 16) & 0xff) as f32 / 255.0;
    let g = ((overlay.diffuse >> 8) & 0xff) as f32 / 255.0;
    let b = (overlay.diffuse & 0xff) as f32 / 255.0;
    let color = [r, g, b, a.max(overlay.intensity)];
    queue.write_buffer(&gpu.color, 0, bytemuck::cast_slice(&color));
    let pipeline = match overlay.fade {
        TFade::Add => &gpu.add,
        TFade::Subtract => &gpu.subtract,
        TFade::Multiply => &gpu.multiply,
        TFade::Saturate => &gpu.saturate,
        TFade::None => return,
    };
    let passes = if overlay.fade == TFade::Saturate {
        2
    } else {
        1
    };
    for _ in 0..passes {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("W3DStatusCircle fade"),
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
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &gpu.bind, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_engine_or_none_fade_clears_overlay() {
        assert!(camera_fade_from_observation(TFade::None, 0.75).is_none());
        let _standalone = render_camera_fade();
    }

    #[test]
    fn packed_fade_maps_cpp_tfade_discriminants() {
        // Exercise the same packed conversion that record_camera_fade uses.
        assert!(overlay_from_packed(0, 1.0, 0).is_none());
        for (code, fade) in [(1, TFade::Subtract), (2, TFade::Add), (3, TFade::Saturate)] {
            assert_eq!(
                overlay_from_packed(code, 0.25, 0xff40_4040).unwrap().fade,
                fade
            );
        }
        let overlay = overlay_from_packed(4, 0.25, 0xff40_4040).expect("multiply fade");
        assert_eq!(overlay.fade, TFade::Multiply);
        assert!((overlay.intensity - 0.25).abs() < f32::EPSILON);
        assert_eq!(overlay.diffuse, 0xff40_4040);
    }

    #[test]
    #[ignore = "requires a native WGPU adapter; run this explicit GPU ownership acceptance test"]
    fn owned_fade_gpu_reuses_only_receiving_device_and_target_format() {
        fn record(
            resources: &mut CameraFadeResources,
            device: &wgpu::Device,
            queue: &wgpu::Queue,
            format: wgpu::TextureFormat,
            fade: u8,
        ) -> Option<wgpu::Buffer> {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("owned fade destination"),
                size: wgpu::Extent3d {
                    width: 4,
                    height: 4,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            record_camera_fade(
                resources,
                device,
                queue,
                &mut encoder,
                &view,
                format,
                fade,
                0.25,
                0xff40_4040,
            );
            queue.submit([encoder.finish()]);
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(std::time::Duration::from_secs(20)),
                })
                .expect("owned fade GPU recording must finish");
            resources.gpu.as_ref().map(|gpu| gpu.color.clone())
        }
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .expect("explicit fade GPU acceptance requires an adapter; never silently skipped");
        let request = |label| {
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some(label),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            }))
            .expect("explicit fade GPU acceptance requires each receiving device")
        };
        let (device_a, queue_a) = request("fade receiving device A");
        let (device_b, queue_b) = request("fade receiving device B");
        assert_ne!(device_a, device_b);
        let mut a = CameraFadeResources::default();
        let mut b = CameraFadeResources::default();
        assert!(
            record(
                &mut a,
                &device_a,
                &queue_a,
                wgpu::TextureFormat::Rgba8Unorm,
                0
            )
            .is_none(),
            "no fade must not allocate resources"
        );
        let first = record(
            &mut a,
            &device_a,
            &queue_a,
            wgpu::TextureFormat::Rgba8Unorm,
            2,
        )
        .unwrap();
        let repeated = record(
            &mut a,
            &device_a,
            &queue_a,
            wgpu::TextureFormat::Rgba8Unorm,
            1,
        )
        .unwrap();
        assert_eq!(
            first, repeated,
            "ordinary frames reuse this owner's resources"
        );
        let peer = record(
            &mut b,
            &device_b,
            &queue_b,
            wgpu::TextureFormat::Rgba8Unorm,
            3,
        )
        .unwrap();
        assert_ne!(
            first, peer,
            "same-format peers must have independent buffers"
        );
        assert_eq!(a.gpu.as_ref().unwrap().device, device_a);
        assert_eq!(b.gpu.as_ref().unwrap().device, device_b);
        let reformatted = record(
            &mut a,
            &device_a,
            &queue_a,
            wgpu::TextureFormat::Bgra8Unorm,
            4,
        )
        .unwrap();
        assert_ne!(
            first, reformatted,
            "a target format change rebuilds this owner"
        );
        assert_eq!(
            b.gpu.as_ref().unwrap().color,
            peer,
            "peer resources remain unchanged"
        );
        assert_eq!(
            b.gpu.as_ref().unwrap().format,
            wgpu::TextureFormat::Rgba8Unorm
        );
        let replaced_device = record(
            &mut a,
            &device_b,
            &queue_b,
            wgpu::TextureFormat::Bgra8Unorm,
            2,
        )
        .unwrap();
        assert_ne!(
            reformatted, replaced_device,
            "a replacement device rebuilds at the same format"
        );
        assert_eq!(a.gpu.as_ref().unwrap().device, device_b);
        assert_eq!(
            b.gpu.as_ref().unwrap().color,
            peer,
            "device replacement never changes a peer owner"
        );
    }
}
