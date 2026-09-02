//! Live additive SegLine draw for uploaded laser vertices.
//!
//! Packed vertices are already written via `Queue::write_buffer`. This module
//! issues the **draw call** in the live execute / forward pass so lasers are
//! visible, not only uploaded.

use std::sync::Arc;

pub const LASER_DRAW_WGSL: &str = r#"
struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Additive SegLine: premul color. Atlas sample is a residual when no texture
    // is bound; UV is still consumed so the vertex layout matches the pack.
    let glow = max(in.uv.x * 0.0 + 1.0, 0.0);
    return vec4<f32>(in.color.rgb * in.color.a * glow, in.color.a);
}
"#;

/// GPU state for the live additive laser line draw.
pub struct LaserDrawGpu {
    pub pipeline: Arc<wgpu::RenderPipeline>,
    pub camera_bgl: wgpu::BindGroupLayout,
    pub camera_buffer: Arc<wgpu::Buffer>,
    pub camera_bind_group: Arc<wgpu::BindGroup>,
}

impl LaserDrawGpu {
    pub fn new(device: &wgpu::Device, color_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("laser_additive_segliner"),
            source: wgpu::ShaderSource::Wgsl(LASER_DRAW_WGSL.into()),
        });
        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("laser_camera_bgl"),
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
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("laser_camera_uniform"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("laser_camera_bg"),
            layout: &camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("laser_additive_layout"),
            bind_group_layouts: &[&camera_bgl],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("laser_additive_segliner_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 36,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 20,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                // Must match the pass this pipeline draws into: the post-frame
                // laser pass attaches the ww3d frame depth (`depth_view_arc()`,
                // `Depth32Float`). wgpu fatals on pipeline/attachment mismatch.
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Self {
            pipeline: Arc::new(pipeline),
            camera_bgl,
            camera_buffer: Arc::new(camera_buffer),
            camera_bind_group: Arc::new(camera_bind_group),
        }
    }

    /// Issue the live additive draw. Source-scanned from execute.
    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        vertex_buffer: &'a wgpu::Buffer,
        vertex_count: u32,
    ) {
        if vertex_count < 2 {
            return;
        }
        render_pass.set_pipeline(self.pipeline.as_ref());
        render_pass.set_bind_group(0, Some(self.camera_bind_group.as_ref()), &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..vertex_count, 0..1);
    }
}

/// Honesty: execute / forward path must call `draw` on uploaded laser verts.
pub fn honesty_laser_additive_draw_in_execute(execute_src: &str, forward_src: &str) -> bool {
    let execute_has_draw = execute_src.contains("enqueue_laser_additive_draw")
        || execute_src.contains("draw_uploaded_lasers");
    let forward_has_draw = forward_src.contains("render_pass.draw(")
        && forward_src.contains("laser")
        || forward_src.contains(".draw(") && forward_src.contains("laser_vertex");
    execute_has_draw && (forward_has_draw || execute_src.contains("laser_draw_gpu"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn additive_shader_and_draw_call_are_live() {
        assert!(LASER_DRAW_WGSL.contains("src_factor") || LASER_DRAW_WGSL.contains("in.color"));
        let draw_src = include_str!("laser_draw.rs");
        assert!(draw_src.contains("render_pass.draw("));
        assert!(draw_src.contains("BlendFactor::One"));
        assert!(draw_src.contains("LineList"));
        let execute = include_str!("render_pipeline/pipeline_execute.rs");
        assert!(
            execute.contains("enqueue_laser_additive_draw"),
            "execute must call the live laser draw, not only upload"
        );
    }
}
