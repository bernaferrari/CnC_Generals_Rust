//! Behind-building player markers from accepted W3D scene geometry.
use crate::game_logic::{KindOf, ObjectId};
use crate::graphics::render_item::{RenderItem, RenderItemOwner};
use crate::graphics::render_pipeline::{RenderPass, RenderPipeline};
use crate::presentation_frame::{PresentationFrame, RenderableObject};
use glam::Vec3;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use wgpu::util::DeviceExt;
use ww3d_renderer_3d::rendering::{
    mesh_system::MeshClass,
    wgpu_main_renderer::{FramePhase, WgpuMainRenderer},
    wgpu_renderer::wgpu_pipeline_manager::{
        OCCLUSION_BUILDING_BIT, OCCLUSION_UNIT_BIT, OcclusionMaskLayer,
    },
};

const OCCLUSION_MASK: u32 = OCCLUSION_BUILDING_BIT | OCCLUSION_UNIT_BIT;
const OCCLUDED_COLOR_ALPHA: f32 = 0.5;
const MAX_CACHED_OCCLUSION_COLORS: usize = 16;

enum Role {
    Building,
    Unit(usize),
}

struct Unit {
    id: ObjectId,
    distance: f32,
    color: [f32; 4],
    meshes: Vec<Arc<MeshClass>>,
}

pub(in crate::graphics) struct OcclusionFrame {
    roles: HashMap<ObjectId, Role>,
    buildings: Vec<Arc<MeshClass>>,
    units: Vec<Unit>,
    gpu: Arc<OcclusionOverlayPipeline>,
}

pub(crate) fn stage_occluded_player_color_pass(
    pipeline: &mut RenderPipeline,
    frame: Option<&PresentationFrame>,
    camera: Vec3,
    luminance: Option<f32>,
) {
    pipeline.forward_pass.pending_occlusion = None;
    let Some((frame, luminance)) = frame.zip(luminance) else {
        return;
    };
    let mut units = Vec::new();
    let mut roles = HashMap::new();
    for object in &frame.objects {
        if object.destroyed {
            continue;
        }
        if object.is_structure {
            roles.insert(object.id, Role::Building);
        } else if is_potential_occludee(object, frame.frame.0) {
            let [r, g, b, _] = object.team_color;
            // W3DScene.cpp scales HSV value, which scales each RGB component.
            let color = [
                r * luminance,
                g * luminance,
                b * luminance,
                OCCLUDED_COLOR_ALPHA,
            ];
            roles.insert(object.id, Role::Unit(units.len()));
            units.push(Unit {
                id: object.id,
                distance: camera.distance_squared(object.position),
                color,
                meshes: Vec::new(),
            });
        }
    }
    if units.is_empty() || units.len() == roles.len() {
        return;
    }
    let (device, color, depth) = {
        let engine = pipeline.forward_pass.renderer.engine.lock();
        (
            engine.device(),
            engine.color_format(),
            engine.depth_format(),
        )
    };
    let Some(depth) = depth.filter(wgpu::TextureFormat::has_stencil_aspect) else {
        return;
    };
    let gpu = OcclusionOverlayPipeline::get_or_create(
        &mut pipeline.occlusion_overlay_pipeline,
        device,
        color,
        depth,
    );
    pipeline.forward_pass.pending_occlusion = Some(OcclusionFrame {
        roles,
        buildings: Vec::new(),
        units,
        gpu,
    });
}

fn is_potential_occludee(object: &RenderableObject, frame: u32) -> bool {
    object.safe_occlusion_frame <= frame
        && [
            KindOf::Score,
            KindOf::ScoreCreate,
            KindOf::ScoreDestroy,
            KindOf::MpCountForVictory,
        ]
        .iter()
        .any(|kind| object.kind_of.contains(kind))
}

impl OcclusionFrame {
    pub(in crate::graphics) fn accepts(&self, item: &RenderItem, mesh: &MeshClass) -> bool {
        item.render_pass == RenderPass::ForwardOpaque
            && matches!(item.owner, RenderItemOwner::Object(_))
            && self.roles.contains_key(&item.object_id)
            && mesh.is_not_hidden_at_all()
            && !mesh.is_disabled_by_debugger
            && !mesh.is_alpha()
    }

    pub(in crate::graphics) fn push(&mut self, id: ObjectId, mesh: Arc<MeshClass>) {
        match self.roles.get(&id) {
            Some(Role::Building) => self.buildings.push(mesh),
            Some(Role::Unit(index)) => self.units[*index].meshes.push(mesh),
            None => {}
        }
    }

    pub(in crate::graphics) fn enqueue(
        mut self,
        renderer: &mut WgpuMainRenderer,
        color_bindings: &mut OcclusionColorBindGroupCache,
    ) {
        self.units.retain(|unit| !unit.meshes.is_empty());
        if self.units.is_empty() || self.buildings.is_empty() {
            return;
        }
        self.units
            .sort_by(|a, b| a.distance.total_cmp(&b.distance).then(a.id.0.cmp(&b.id.0)));
        let unit_color_bindings = self
            .units
            .chunk_by(|a, b| a.color == b.color)
            .map(|units| {
                color_bindings.get_or_create(
                    &self.gpu.device,
                    &self.gpu.color_layout,
                    units[0].color,
                )
            })
            .collect::<Vec<_>>();
        let handle = renderer.renderer_handle();
        renderer.enqueue_frame_work(FramePhase::OccludedStencil, move |frame| {
            let mut renderer = handle.lock().map_err(|_| {
                ww3d_renderer_3d::Error::InvalidOperation("mesh owner lock poisoned".into())
            })?;
            // The volume fill has already finished in the receiving scene
            // mesh flush. Reuse only these bits and keep the scene depth.
            self.gpu.clear(frame.scene_pass());
            let result = (|| {
                renderer.render_occlusion_mask(
                    frame,
                    &self.buildings,
                    OcclusionMaskLayer::Building,
                )?;
                // Consecutive units with the same color may mark their union
                // before a single fill. This avoids one full-screen draw and
                // uniform upload per unit when one player owns the visible army.
                for (index, units) in self.units.chunk_by(|a, b| a.color == b.color).enumerate() {
                    renderer.render_occlusion_mask(
                        frame,
                        units.iter().flat_map(|unit| &unit.meshes),
                        OcclusionMaskLayer::Unit,
                    )?;
                    self.gpu
                        .fill(frame.scene_pass(), &unit_color_bindings[index].bind_group);
                }
                Ok(())
            })();
            self.gpu.clear(frame.scene_pass());
            result
        });
    }
}

const FILL_SHADER: &str = r"
@group(0) @binding(0) var<uniform> player_color: vec4<f32>;

@vertex
fn vs_fill(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(p[i], 0.0, 1.0);
}

@fragment
fn fs_fill() -> @location(0) vec4<f32> { return player_color; }

@fragment
fn fs_clear() -> @location(0) vec4<f32> { return vec4(0.0); }
";

pub(in crate::graphics) struct OcclusionOverlayPipeline {
    device: Arc<wgpu::Device>,
    color_format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
    color_layout: Arc<wgpu::BindGroupLayout>,
    fill: wgpu::RenderPipeline,
    clear: wgpu::RenderPipeline,
}

impl OcclusionOverlayPipeline {
    fn get_or_create(
        cached: &mut Option<Arc<Self>>,
        device: Arc<wgpu::Device>,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Arc<Self> {
        if let Some(gpu) = cached
            && Arc::ptr_eq(&device, &gpu.device)
            && color_format == gpu.color_format
            && depth_format == gpu.depth_format
        {
            return Arc::clone(gpu);
        }
        let gpu = Arc::new(Self::new(device, color_format, depth_format));
        *cached = Some(Arc::clone(&gpu));
        gpu
    }

    fn new(
        device: Arc<wgpu::Device>,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("behind-building player color"),
            source: wgpu::ShaderSource::Wgsl(FILL_SHADER.into()),
        });
        let color_layout = Arc::new(device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("behind-building color layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                }],
            },
        ));
        let make_pipeline = |with_color: bool| {
            let groups: [Option<&wgpu::BindGroupLayout>; 1] = [Some(color_layout.as_ref())];
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("behind-building stencil layout"),
                bind_group_layouts: if with_color { &groups } else { &[] },
                immediate_size: 0,
            });
            let face = wgpu::StencilFaceState {
                compare: if with_color {
                    wgpu::CompareFunction::Equal
                } else {
                    wgpu::CompareFunction::Always
                },
                pass_op: wgpu::StencilOperation::Zero,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
            };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(if with_color {
                    "behind-building color fill"
                } else {
                    "behind-building clear bits"
                }),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_fill"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(if with_color { "fs_fill" } else { "fs_clear" }),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: with_color.then_some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::SrcAlpha,
                                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::SrcAlpha,
                                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                operation: wgpu::BlendOperation::Add,
                            },
                        }),
                        write_mask: if with_color {
                            wgpu::ColorWrites::ALL
                        } else {
                            wgpu::ColorWrites::empty()
                        },
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: depth_format,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: wgpu::StencilState {
                        front: face,
                        back: face,
                        read_mask: OCCLUSION_MASK,
                        write_mask: OCCLUSION_MASK,
                    },
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let fill = make_pipeline(true);
        let clear = make_pipeline(false);
        Self {
            device,
            color_format,
            depth_format,
            color_layout,
            fill,
            clear,
        }
    }

    fn clear(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.clear);
        pass.draw(0..3, 0..1);
        pass.set_stencil_reference(0);
    }

    fn fill(&self, pass: &mut wgpu::RenderPass<'_>, group: &wgpu::BindGroup) {
        pass.set_pipeline(&self.fill);
        pass.set_bind_group(0, group, &[]);
        pass.set_stencil_reference(OCCLUSION_MASK);
        pass.draw(0..3, 0..1);
    }
}

struct OcclusionColorBinding {
    _uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

/// Device-owned color bindings persist across frames without synchronization:
/// the ForwardPass owns this bounded cache and populates it before queuing the
/// frame callback.
#[derive(Default)]
pub(in crate::graphics) struct OcclusionColorBindGroupCache {
    device: Option<Arc<wgpu::Device>>,
    layout: Option<Arc<wgpu::BindGroupLayout>>,
    entries: VecDeque<([u32; 4], Arc<OcclusionColorBinding>)>,
    #[cfg(test)]
    creations: u64,
}

impl OcclusionColorBindGroupCache {
    fn get_or_create(
        &mut self,
        device: &Arc<wgpu::Device>,
        layout: &Arc<wgpu::BindGroupLayout>,
        color: [f32; 4],
    ) -> Arc<OcclusionColorBinding> {
        let device_changed = self
            .device
            .as_ref()
            .is_some_and(|cached| !Arc::ptr_eq(cached, device));
        let layout_changed = self
            .layout
            .as_ref()
            .is_some_and(|cached| !Arc::ptr_eq(cached, layout));
        if device_changed || layout_changed {
            self.entries.clear();
            self.device = Some(Arc::clone(device));
            self.layout = Some(Arc::clone(layout));
        } else {
            self.device.get_or_insert_with(|| Arc::clone(device));
            self.layout.get_or_insert_with(|| Arc::clone(layout));
        }

        let key = color.map(f32::to_bits);
        if let Some((_, binding)) = self.entries.iter().find(|(cached, _)| *cached == key) {
            return Arc::clone(binding);
        }

        if self.entries.len() == MAX_CACHED_OCCLUSION_COLORS {
            self.entries.pop_front();
        }
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("behind-building player color"),
            contents: bytemuck::bytes_of(&color),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("behind-building player color"),
            layout: layout.as_ref(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let binding = Arc::new(OcclusionColorBinding {
            _uniform: uniform,
            bind_group,
        });
        self.entries.push_back((key, Arc::clone(&binding)));
        #[cfg(test)]
        {
            self.creations += 1;
        }
        binding
    }

    #[cfg(test)]
    fn creation_count(&self) -> u64 {
        self.creations
    }

    #[cfg(test)]
    fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
#[path = "occlusion_receiver_tests.rs"]
mod tests;
