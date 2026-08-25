//! C++ `W3DStatusCircle::Render` camera-fade overlay.

use gamelogic::scripting::{TFade, get_script_engine};
use std::sync::Mutex;

/// Fullscreen camera-fade overlay produced by `W3DStatusCircle::Render`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraFadeOverlay {
    pub fade: TFade,
    /// C++ `TheScriptEngine->getFadeValue()` in 0..1.
    pub intensity: f32,
    /// Packed ARGB used as the fullscreen quad vertex color (`255 * intensity`).
    pub diffuse: u32,
}

static LAST_OVERLAY: Mutex<Option<CameraFadeOverlay>> = Mutex::new(None);

/// C++ `W3DStatusCircle::Render` fade branch.
pub fn render_camera_fade() -> Option<CameraFadeOverlay> {
    let overlay = get_script_engine().read().ok().and_then(|guard| {
        let engine = guard.as_ref()?;
        let fade = engine.get_fade();
        if fade == TFade::None {
            return None;
        }
        let intensity = engine.get_fade_value().clamp(0.0, 1.0);
        let channel = (255.0 * intensity) as u32;
        Some(CameraFadeOverlay {
            fade,
            intensity,
            diffuse: (0xff << 24) | (channel << 16) | (channel << 8) | channel,
        })
    });
    if let Ok(mut slot) = LAST_OVERLAY.lock() {
        *slot = overlay;
    }
    overlay
}

/// Last fade overlay computed this frame.
pub fn current_camera_fade() -> Option<CameraFadeOverlay> {
    LAST_OVERLAY.lock().ok().and_then(|slot| *slot)
}

static QUEUED_LIVE_FADE: Mutex<Option<CameraFadeOverlay>> = Mutex::new(None);

/// Stamp a frozen presentation fade for the live overlay / render_pipeline blit.
pub fn queue_live_camera_fade(fade: u8, intensity: f32, diffuse: u32) {
    let overlay = overlay_from_packed(fade, intensity, diffuse);
    if let Ok(mut slot) = QUEUED_LIVE_FADE.lock() {
        *slot = overlay;
    }
}

/// Consume the overlay queued by the live letterbox/cinematic pass.
pub fn take_queued_live_camera_fade() -> Option<CameraFadeOverlay> {
    QUEUED_LIVE_FADE
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
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
    format: wgpu::TextureFormat,
    color: wgpu::Buffer,
    bind: wgpu::BindGroup,
    add: wgpu::RenderPipeline,
    subtract: wgpu::RenderPipeline,
    multiply: wgpu::RenderPipeline,
    saturate: wgpu::RenderPipeline,
}

static FADE_GPU: Mutex<Option<FadeGpu>> = Mutex::new(None);

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
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
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
                multiview: None,
                cache: None,
            })
        };
        Self {
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
    record_camera_fade_overlay(device, queue, encoder, dest, format, overlay);
}

/// Draw a frozen `CameraFadeOverlay` with ADD / REVSUBTRACT / 4x SATURATE / MULTIPLY.
pub fn record_camera_fade_overlay(
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
    let mut slot = match FADE_GPU.lock() {
        Ok(slot) => slot,
        Err(_) => return,
    };
    if slot.as_ref().map(|gpu| gpu.format) != Some(format) {
        *slot = Some(FadeGpu::new(device, format));
    }
    let Some(gpu) = slot.as_ref() else {
        return;
    };
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
        let overlay = render_camera_fade();
        if overlay.is_none() {
            assert!(current_camera_fade().is_none());
        }
    }

    #[test]
    fn packed_fade_maps_cpp_tfade_discriminants() {
        queue_live_camera_fade(0, 1.0, 0);
        assert!(take_queued_live_camera_fade().is_none());
        queue_live_camera_fade(4, 0.25, 0xff40_4040);
        let overlay = take_queued_live_camera_fade().expect("multiply fade");
        assert_eq!(overlay.fade, TFade::Multiply);
        assert!((overlay.intensity - 0.25).abs() < f32::EPSILON);
        assert_eq!(overlay.diffuse, 0xff40_4040);
    }
}
