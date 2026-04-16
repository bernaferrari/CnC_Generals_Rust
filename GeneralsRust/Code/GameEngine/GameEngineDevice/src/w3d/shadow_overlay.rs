//! Shadow Overlay Renderer
//!
//! Renders a shadow darkening pass over the 3D scene using stencil buffer
//! shadow volumes (Carmack's reverse). This is the infrastructure for
//! C++ W3DVolumetricShadow parity.
//!
//! C++ Reference: W3DVolumetricShadow.cpp (~4075 lines)
//! Shadow color: 0x7fa0a0a0 (semi-transparent dark gray)

use std::sync::Arc;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, BlendComponent, BlendFactor, BlendOperation, BlendState,
    Buffer, BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites, CompareFunction,
    DepthBiasState, DepthStencilState, Device, FragmentState, FrontFace, MultisampleState,
    PipelineLayout, PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology,
    RenderPipeline, RenderPipelineDescriptor, ShaderStages, StencilFaceState, StencilOperation,
    StencilState, TextureFormat, VertexState,
};

/// C++ shadow color: 0x7fa0a0a0 → RGBA(160/255, 160/255, 160/255, 127/255)
const SHADOW_COLOR: [f32; 4] = [0.627, 0.627, 0.627, 0.498];

/// Shadow uniform data (world-space light direction)
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadowUniforms {
    light_direction: [f32; 4],
    shadow_color: [f32; 4],
}

/// Shadow Overlay Renderer
///
/// Provides stencil-based shadow darkening. In the full implementation:
/// 1. Shadow volumes are rendered to the stencil buffer (front INCR, back DECR)
/// 2. This overlay renders a fullscreen dark quad where stencil != 0
///
/// Currently implements step 2 (the darkening pass). Shadow volume geometry
/// collection (step 1) will be added as the render pipeline is enhanced.
pub struct ShadowOverlay {
    queue: Arc<wgpu::Queue>,
    pipeline: RenderPipeline,
    uniform_buffer: Buffer,
    bind_group: BindGroup,
    bind_group_layout: BindGroupLayout,
}

impl ShadowOverlay {
    pub fn new(
        device: Arc<Device>,
        queue: Arc<wgpu::Queue>,
        color_format: TextureFormat,
        depth_format: TextureFormat,
    ) -> Self {
        let shader_source = include_str!("shadow_overlay.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow Overlay Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Shadow Overlay Bind Group Layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Shadow Overlay Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let uniforms = ShadowUniforms {
            light_direction: [0.0, -1.0, 0.0, 0.0],
            shadow_color: SHADOW_COLOR,
        };

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Shadow Uniform Buffer"),
            size: std::mem::size_of::<ShadowUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        queue.write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Shadow Overlay Bind Group"),
            layout: &bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Shadow Overlay Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_shadow_mask"),
                compilation_options: Default::default(),
                targets: &[Some(ColorTargetState {
                    format: color_format,
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: CompareFunction::LessEqual,
                stencil: StencilState {
                    front: StencilFaceState {
                        compare: CompareFunction::NotEqual,
                        fail_op: StencilOperation::Keep,
                        depth_fail_op: StencilOperation::Keep,
                        pass_op: StencilOperation::Keep,
                    },
                    back: StencilFaceState {
                        compare: CompareFunction::NotEqual,
                        fail_op: StencilOperation::Keep,
                        depth_fail_op: StencilOperation::Keep,
                        pass_op: StencilOperation::Keep,
                    },
                    read_mask: 0xFF,
                    write_mask: 0,
                },
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Self {
            queue,
            pipeline,
            uniform_buffer,
            bind_group,
            bind_group_layout,
        }
    }

    /// Set light direction (normalized)
    pub fn set_light_direction(&self, direction: [f32; 3]) {
        let len = (direction[0] * direction[0]
            + direction[1] * direction[1]
            + direction[2] * direction[2])
            .sqrt();
        let normalized = if len > 0.0 {
            [
                direction[0] / len,
                direction[1] / len,
                direction[2] / len,
                0.0,
            ]
        } else {
            [0.0, -1.0, 0.0, 0.0]
        };
        let uniforms = ShadowUniforms {
            light_direction: normalized,
            shadow_color: SHADOW_COLOR,
        };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    /// Render shadow darkening overlay
    pub fn render<'rpass>(&'rpass self, render_pass: &mut wgpu::RenderPass<'rpass>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..4, 0..1);
    }
}
