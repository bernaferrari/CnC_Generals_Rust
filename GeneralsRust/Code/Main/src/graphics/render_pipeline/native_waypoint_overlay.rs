//! The SCMNode and EXLaser draw between ground shroud and terrain bibs.
use super::*;
use crate::presentation_frame::waypoint_overlay::WaypointOverlayPath;
use std::ops::Range;

const NODE_MODEL: &str = "SCMNode";
const LINE_TEXTURE: &str = "EXLaser.tga";

impl RenderPipeline {
    pub(crate) fn stage_native_waypoint_overlay(
        &mut self,
        graphics: &mut GraphicsSystem,
        paths: Vec<WaypointOverlayPath>,
    ) {
        self.forward_pass.pending_waypoint_overlay.clear();
        if paths.is_empty() || self.native_terrain.is_none() {
            return;
        }

        if graphics.get_model(NODE_MODEL).is_none() {
            let mut no_deferred_loads = 0;
            let _ = Self::ensure_render_model_loaded(
                self.forward_pass.application_assets.as_deref(),
                &mut self.failed_model_resolutions,
                graphics,
                NODE_MODEL,
                NODE_MODEL,
                true,
                &mut no_deferred_loads,
            );
        }
        self.forward_pass.pending_waypoint_overlay = paths;
    }
}

struct WaypointPathDraw {
    nodes: Vec<Arc<MeshClass>>,
    ribbon: Range<u32>,
}

pub(super) struct NativeWaypointDraw {
    paths: Vec<WaypointPathDraw>,
}

impl NativeWaypointDraw {
    pub(super) fn record(
        &self,
        frame: &mut ww3d_engine::RenderFrame,
        renderer: &Arc<Mutex<ww3d_renderer_3d::Renderer>>,
        gpu: Option<&WaypointRibbonGpu>,
        viewport: (f32, f32, f32, f32),
    ) -> RendererResult<()> {
        for path in &self.paths {
            if !path.nodes.is_empty() {
                renderer
                    .lock()
                    .map_err(|_| {
                        ww3d_renderer_3d::Error::InvalidOperation(
                            "waypoint node renderer poisoned".into(),
                        )
                    })?
                    .render_mesh_batch_into_frame(frame, &path.nodes)?;
            }
            if !path.ribbon.is_empty()
                && let Some(gpu) = gpu
            {
                gpu.record(frame, path.ribbon.clone(), viewport);
            }
        }
        Ok(())
    }
}

impl ForwardPass {
    pub(super) fn prepare_native_waypoint_overlay(
        &mut self,
        graphics: &GraphicsSystem,
        paths: Vec<WaypointOverlayPath>,
        eye: Vec3,
        view_proj: Mat4,
    ) -> Option<NativeWaypointDraw> {
        if paths.is_empty() {
            return None;
        }
        let mut ambient = LightEnvironmentClass::new();
        ambient.reset(Vec3::ZERO, Vec3::ONE);
        let ambient = Arc::new(ambient);
        let node_model = graphics.get_model(NODE_MODEL);
        let mut vertices = Vec::new();
        let mut draws = Vec::with_capacity(paths.len());
        for path in paths {
            let ribbon_start = vertices.len() as u32;
            append_ribbon(&path, eye, &mut vertices);
            let mut nodes = Vec::new();
            if let Some(model) = node_model {
                for &goal in path.goals.iter().filter(|goal| goal.is_finite()) {
                    for (index, node) in model.meshes.iter().enumerate() {
                        let mut item = RenderItem::new_unbound_client_drawable(
                            0,
                            NODE_MODEL.into(),
                            index,
                            goal,
                            Mat4::from_translation(goal),
                            &node.material,
                            RenderPass::ForwardOpaque,
                        );
                        item.fow_visibility = ObjectVisibility::FULLY_VISIBLE;
                        if let Ok(Some(mut mesh)) =
                            self.prepare_mesh_instance(graphics, &item, None)
                        {
                            // prepare_mesh_instance returns a new, unshared instance.
                            if let Some(mesh) = Arc::get_mut(&mut mesh) {
                                mesh.set_lighting_environment(Some(Arc::clone(&ambient)));
                                mesh.set_projected_shroud_eligible(false);
                            }
                            nodes.push(mesh);
                        }
                    }
                }
            }
            draws.push(WaypointPathDraw {
                nodes,
                ribbon: ribbon_start..vertices.len() as u32,
            });
        }
        if !vertices.is_empty() {
            if self.waypoint_ribbon_gpu.is_none() {
                let authored = self
                    .application_assets
                    .as_ref()
                    .and_then(|assets| assets.lock().ok())
                    .and_then(|mut assets| {
                        assets.prime_texture_raw_blocking(LINE_TEXTURE);
                        assets
                            .gpu_texture_for_cached_raw(&self.device, &self.queue, LINE_TEXTURE)
                            .ok()
                            .map(|resource| resource.view.clone())
                    });
                let format = self.renderer.engine.lock().color_format();
                self.waypoint_ribbon_gpu = Some(WaypointRibbonGpu::new(
                    &self.device,
                    &self.queue,
                    format,
                    authored,
                ));
            }
            if !self
                .waypoint_ribbon_gpu
                .as_mut()
                .is_some_and(|gpu| gpu.upload(&self.device, &self.queue, &vertices, view_proj))
            {
                for draw in &mut draws {
                    draw.ribbon = 0..0;
                }
            }
        }
        Some(NativeWaypointDraw { paths: draws })
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RibbonVertex {
    position: [f32; 3],
    uv: [f32; 2],
    color: [f32; 3],
}

fn append_ribbon(path: &WaypointOverlayPath, eye: Vec3, vertices: &mut Vec<RibbonVertex>) {
    let points: Vec<_> = path.ribbon_points().collect();
    if points.len() < 2 || points.iter().any(|point| !point.is_finite()) {
        return;
    }
    let half_width = path.style.width() * 0.5;
    let color = path.style.rgb();
    let edge = |index: usize| {
        let point = points[index];
        let view = (eye - point).normalize_or_zero();
        let incoming = index.checked_sub(1).map_or(Vec3::ZERO, |prev| {
            view.cross(point - points[prev]).normalize_or_zero()
        });
        let outgoing = points.get(index + 1).map_or(Vec3::ZERO, |next| {
            view.cross(*next - point).normalize_or_zero()
        });
        let miter = (incoming + outgoing).normalize_or_zero();
        let scale = miter.dot(if outgoing == Vec3::ZERO {
            incoming
        } else {
            outgoing
        });
        if scale > 0.25 {
            miter * (half_width / scale)
        } else {
            outgoing * half_width
        }
    };

    let mut previous_edge = edge(0);
    for (index, pair) in points.windows(2).enumerate() {
        let next_edge = edge(index + 1);
        if pair[0] != pair[1] {
            let v = index as f32;
            let a = RibbonVertex {
                position: (pair[0] - previous_edge).to_array(),
                uv: [0.0, v],
                color,
            };
            let b = RibbonVertex {
                position: (pair[0] + previous_edge).to_array(),
                uv: [1.0, v],
                color,
            };
            let c = RibbonVertex {
                position: (pair[1] - next_edge).to_array(),
                uv: [0.0, v + 1.0],
                color,
            };
            let d = RibbonVertex {
                position: (pair[1] + next_edge).to_array(),
                uv: [1.0, v + 1.0],
                color,
            };
            vertices.extend([a, b, c, c, b, d]);
        }
        previous_edge = next_edge;
    }
}

pub(super) struct WaypointRibbonGpu {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    camera: wgpu::Buffer,
    vertex: wgpu::Buffer,
}

impl WaypointRibbonGpu {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        authored: Option<wgpu::TextureView>,
    ) -> Self {
        let camera = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Waypoint camera"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let texture = authored.unwrap_or_else(|| {
            // SegmentedLine has no texture if EXLaser was not admitted.
            let extent = wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            };
            let white = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Untextured waypoint modulation"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                white.as_image_copy(),
                &[255; 4],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
                extent,
            );
            white.create_view(&Default::default())
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Waypoint ribbon"),
            source: wgpu::ShaderSource::Wgsl(include_str!("native_waypoint_ribbon.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Waypoint ribbon"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<RibbonVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x3],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: ww3d_gpu::SCENE_DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Waypoint ribbon bindings"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        Self {
            pipeline,
            bind_group,
            camera,
            vertex: Self::vertex_buffer(device, 256),
        }
    }

    fn vertex_buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Waypoint ribbon vertices"),
            size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertices: &[RibbonVertex],
        view_proj: Mat4,
    ) -> bool {
        let bytes = bytemuck::cast_slice(vertices);
        let needed = bytes.len() as u64;
        let limit = device.limits().max_buffer_size;
        if needed > limit {
            return false;
        }
        if needed > self.vertex.size() {
            self.vertex = Self::vertex_buffer(device, needed.next_power_of_two().min(limit));
        }
        queue.write_buffer(&self.camera, 0, bytemuck::bytes_of(&view_proj));
        queue.write_buffer(&self.vertex, 0, bytes);
        true
    }

    fn record(
        &self,
        frame: &mut ww3d_engine::RenderFrame,
        vertices: Range<u32>,
        (x, y, width, height): (f32, f32, f32, f32),
    ) {
        let pass = frame.scene_pass();
        pass.set_viewport(x, y, width, height, 0.0, 1.0);
        pass.set_scissor_rect(x as u32, y as u32, width as u32, height as u32);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex.slice(..));
        pass.draw(vertices, 0..1);
    }
}
#[cfg(test)]
#[path = "native_waypoint_overlay_tests.rs"]
mod tests;
