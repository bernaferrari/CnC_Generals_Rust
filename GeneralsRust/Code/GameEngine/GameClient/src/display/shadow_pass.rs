//! Live W3DDisplay shadow / occlusion pass.
//!
//! C++ `DoShadows` + `flushOccludedObjectsIntoStencil` + `renderStencilShadows`.
//! Projected unit decals are queued onto terrain; volumetric volumes write
//! stencil then a 0x7fa0a0a0 fill quad. Occluded units get a player-color pass.

use crate::display::view::with_tactical_view_ref;
use crate::drawable::StealthLook;
use crate::drawable::drawable_manager::with_drawable_manager;
use crate::effects::decals::DecalRenderItem;
use crate::radius_decal::get_projected_shadow_manager;
use crate::terrain::TerrainVisual;
use crate::terrain::terrain_visual::THE_TERRAIN_VISUAL;
use game_engine::common::ini::ini_game_data::get_global_data;
use gamelogic::common::SHADOW_VOLUME;
use gamelogic::common::types::KindOf;
use gamelogic::helpers::TheGameLogic;
use gamelogic::object::registry::OBJECT_REGISTRY;
use std::collections::BTreeMap;
use std::sync::Mutex;
use wgpu::util::DeviceExt;

#[derive(Debug, Clone)]
pub struct UnitShadowCaster {
    pub position: [f32; 3],
    pub size_x: f32,
    pub size_y: f32,
    pub angle: f32,
    pub player_color: u32,
    pub occluded: bool,
    pub heat_vision: bool,
    pub volume: bool,
}

static UNIT_CASTERS: Mutex<Vec<UnitShadowCaster>> = Mutex::new(Vec::new());
static SHADOW_REBUILD_SERIAL: Mutex<u32> = Mutex::new(0);

/// C++ `W3DShadowManager::invalidateCachedLightPositions` / GameLOD hook.
pub fn rebuild_shadows() {
    if let Ok(mut serial) = SHADOW_REBUILD_SERIAL.lock() {
        *serial = serial.wrapping_add(1);
    }
    if let Ok(mut casters) = UNIT_CASTERS.lock() {
        casters.clear();
    }
}

pub fn shadow_rebuild_serial() -> u32 {
    SHADOW_REBUILD_SERIAL.lock().map(|g| *g).unwrap_or(0)
}

pub fn register_unit_shadow(caster: UnitShadowCaster) {
    if let Ok(mut list) = UNIT_CASTERS.lock() {
        list.push(caster);
    }
}

pub fn clear_unit_shadows() {
    if let Ok(mut list) = UNIT_CASTERS.lock() {
        list.clear();
    }
}

/// C++ `W3DProjectedShadowManager::flushDecals` / `queueDecal`.
/// Only allocated projected decals (`addDecal` / `addShadow`). C++ has no
/// per-drawable fallback blob — inventing one double-draws once real decals land.
pub fn collect_unit_decal_items() -> Vec<DecalRenderItem> {
    get_projected_shadow_manager().read().collect_render_items()
}

/// C++ `GlobalData::m_occludedLuminanceScale` default (`GlobalData.cpp:746`).
pub const OCCLUDED_LUMINANCE_SCALE: f32 = 0.5;
/// C++ `Convert_Color(rgb, 0.5f)` (`W3DScene.cpp:1383`).
pub const OCCLUDED_COLOR_ALPHA: f32 = 0.5;
/// C++ D24S8 present target used by `W3DVolumetricShadowManager`.
pub const DISPLAY_DEPTH_STENCIL_FORMAT: wgpu::TextureFormat =
    wgpu::TextureFormat::Depth24PlusStencil8;
/// C++ `TheW3DShadowManager->getShadowColor()` default `0x7fa0a0a0`.
const STENCIL_SHADOW_COLOR: [f32; 4] = [160.0 / 255.0, 160.0 / 255.0, 160.0 / 255.0, 127.0 / 255.0];
const MAX_VISIBLE_OCCLUDED_PLAYER_OBJECTS: usize = 512;

/// C++ `DX8Wrapper::Has_Stencil` — D24S8 / D24X4S4 / D32S8.
pub fn depth_format_has_stencil(format: wgpu::TextureFormat) -> bool {
    matches!(
        format,
        wgpu::TextureFormat::Depth24PlusStencil8
            | wgpu::TextureFormat::Depth32FloatStencil8
            | wgpu::TextureFormat::Stencil8
    )
}

/// Live host present depth (`Display::create_depth_texture`).
/// Display/Main stay on Depth32Float so existing pipelines keep matching.
pub fn volumetric_stencil_supported() -> bool {
    volumetric_stencil_supported_for(wgpu::TextureFormat::Depth32Float)
}

pub fn volumetric_stencil_supported_for(format: wgpu::TextureFormat) -> bool {
    depth_format_has_stencil(format)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumetricPresentStatus {
    Submitted { volume_count: usize },
    SkippedNoStencil,
}

/// SHADOW_VOLUME casters for this frame (`UnitShadowCaster.volume` plus live objects).
pub fn collect_volume_casters() -> Vec<UnitShadowCaster> {
    let mut out = UNIT_CASTERS
        .lock()
        .map(|g| g.iter().filter(|c| c.volume).cloned().collect())
        .unwrap_or_default();
    let volumes_on = get_global_data()
        .map(|g| g.read().use_shadow_volumes)
        .unwrap_or(true);
    if !volumes_on {
        return out;
    }
    for id in OBJECT_REGISTRY.get_all_object_ids() {
        let Some(caster) = OBJECT_REGISTRY.with_object(id, |obj| {
            if (obj.get_template().get_shadow_type_bits() & SHADOW_VOLUME) == 0 {
                return None;
            }
            let pos = obj.get_position();
            let geom = obj.get_geometry_info();
            Some(UnitShadowCaster {
                position: [pos.x, pos.y, pos.z],
                size_x: (geom.get_major_radius() * 2.0).max(4.0),
                size_y: (geom.get_minor_radius() * 2.0).max(4.0),
                angle: obj.get_orientation(),
                player_color: pack_player_color(obj),
                occluded: false,
                heat_vision: false,
                volume: true,
            })
        }) else {
            continue;
        };
        if let Some(caster) = caster {
            out.push(caster);
        }
    }
    out
}

/// C++ `W3DVolumetricShadowManager::renderShadows` + `renderStencilShadows`.
pub fn present_volumetric_shadows(depth_format: wgpu::TextureFormat) -> VolumetricPresentStatus {
    if !volumetric_stencil_supported_for(depth_format) {
        return VolumetricPresentStatus::SkippedNoStencil;
    }
    VolumetricPresentStatus::Submitted {
        volume_count: collect_volume_casters().len(),
    }
}

/// C++ `RGB_To_HSV` (`colorspace.h`) then `hsv.Z *= m_occludedLuminanceScale`.
pub fn scale_occluded_player_color(argb: u32) -> [f32; 4] {
    let r = ((argb >> 16) & 0xff) as f32 / 255.0;
    let g = ((argb >> 8) & 0xff) as f32 / 255.0;
    let b = (argb & 0xff) as f32 / 255.0;
    let (h, s, v) = rgb_to_hsv(r, g, b);
    let (r, g, b) = hsv_to_rgb(h, s, (v * OCCLUDED_LUMINANCE_SCALE).clamp(0.0, 1.0));
    [r, g, b, OCCLUDED_COLOR_ALPHA]
}

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let v = max;
    let s = if max != 0.0 { (max - min) / max } else { 0.0 };
    let h = if s == 0.0 {
        -1.0
    } else {
        let delta = max - min;
        let hue = if r == max {
            (g - b) / delta
        } else if g == max {
            2.0 + (b - r) / delta
        } else {
            4.0 + (r - g) / delta
        };
        let hue = hue * 60.0;
        if hue < 0.0 { hue + 360.0 } else { hue }
    };
    (h, s, v)
}

fn hsv_to_rgb(mut h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    if s == 0.0 {
        return (v, v, v);
    }
    if h == 360.0 {
        h = 0.0;
    }
    h /= 60.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - (s * f));
    let t = v * (1.0 - (s * (1.0 - f)));
    match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        5 => (v, p, q),
        _ => (v, v, v),
    }
}

fn pack_player_color(obj: &gamelogic::object::Object) -> u32 {
    obj.get_controlling_player()
        .and_then(|player| {
            player.read().ok().map(|guard| {
                let color = guard.get_player_color();
                ((color.r as u32) << 16) | ((color.g as u32) << 8) | (color.b as u32)
            })
        })
        .unwrap_or(0)
}

fn score_occludee_kind(obj: &gamelogic::object::Object) -> bool {
    obj.is_kind_of(KindOf::Score)
        || obj.is_kind_of(KindOf::ScoreCreate)
        || obj.is_kind_of(KindOf::ScoreDestroy)
        || obj.is_kind_of(KindOf::CountsForVictory)
}

fn ray_sphere_occludes(
    camera: [f32; 3],
    target: [f32; 3],
    sphere_center: [f32; 3],
    sphere_radius: f32,
) -> bool {
    let to_target = [
        target[0] - camera[0],
        target[1] - camera[1],
        target[2] - camera[2],
    ];
    let max_distance =
        (to_target[0] * to_target[0] + to_target[1] * to_target[1] + to_target[2] * to_target[2])
            .sqrt();
    if max_distance <= f32::EPSILON {
        return false;
    }
    let direction = [
        to_target[0] / max_distance,
        to_target[1] / max_distance,
        to_target[2] / max_distance,
    ];
    let sphere_vector = [
        sphere_center[0] - camera[0],
        sphere_center[1] - camera[1],
        sphere_center[2] - camera[2],
    ];
    let alpha = sphere_vector[0] * direction[0]
        + sphere_vector[1] * direction[1]
        + sphere_vector[2] * direction[2];
    let sv2 = sphere_vector[0] * sphere_vector[0]
        + sphere_vector[1] * sphere_vector[1]
        + sphere_vector[2] * sphere_vector[2];
    let beta = sphere_radius * sphere_radius - (sv2 - alpha * alpha);
    if beta < 0.0 {
        return false;
    }
    let distance = alpha - beta.sqrt();
    distance >= 0.0 && distance < max_distance
}

/// Collect occluded-unit player-color and detected-stealth heat-vision overlays.
///
/// C++ `flushOccludedObjectsIntoStencil` paints luminance-dimmed player color
/// at 0.5 alpha. Heat-vision is only the detected-stealth second material pass.
pub fn collect_occlusion_overlays() -> Vec<OcclusionOverlay> {
    let mut out = Vec::new();
    with_drawable_manager(|manager| {
        for id in manager.get_all_drawable_ids() {
            let Some(drawable) = manager.get_drawable(id) else {
                continue;
            };
            if !drawable.is_visible() {
                continue;
            }
            let look = drawable.get_stealth_look();
            if !matches!(
                look,
                StealthLook::VisibleDetected | StealthLook::VisibleFriendlyDetected
            ) {
                continue;
            }
            let pos = drawable.get_position();
            out.push(OcclusionOverlay {
                position: [pos.x, pos.y, pos.z],
                color: [1.0, 0.35, 0.05, 0.85],
                kind: OverlayKind::HeatVision,
            });
        }
    });
    let markers_on = get_global_data()
        .map(|g| g.read().enable_behind_building_markers)
        .unwrap_or(true);
    if markers_on {
        let camera = with_tactical_view_ref(|view| {
            let cam = view.get_3d_camera_position();
            [cam.x, cam.y, cam.z]
        });
        let current_frame = TheGameLogic::get_frame();
        let mut candidates = Vec::new();
        for id in OBJECT_REGISTRY.get_all_object_ids() {
            if let Some(candidate) = OBJECT_REGISTRY.with_object(id, |obj| {
                let pos = obj.get_position();
                let geom = obj.get_geometry_info();
                let is_structure = obj.is_kind_of(KindOf::Structure);
                (
                    [pos.x, pos.y, pos.z],
                    geom.get_major_radius().max(4.0),
                    is_structure,
                    !is_structure
                        && score_occludee_kind(obj)
                        && obj.get_safe_occlusion_frame() <= current_frame,
                    obj.get_controlling_player_id().unwrap_or(0) as usize,
                    pack_player_color(obj),
                )
            }) {
                candidates.push(candidate);
            }
        }
        let occluders: Vec<_> = candidates
            .iter()
            .filter(|c| c.2)
            .map(|c| (c.0, c.1))
            .collect();
        if !occluders.is_empty() {
            let mut buckets: BTreeMap<usize, Vec<([f32; 3], u32)>> = BTreeMap::new();
            for (pos, _radius, is_structure, is_occludee, player_index, color) in &candidates {
                if *is_structure || !*is_occludee {
                    continue;
                }
                if !occluders
                    .iter()
                    .any(|(center, radius)| ray_sphere_occludes(camera, *pos, *center, *radius))
                {
                    continue;
                }
                let bucket = buckets.entry(*player_index).or_default();
                if bucket.len() < MAX_VISIBLE_OCCLUDED_PLAYER_OBJECTS {
                    bucket.push((*pos, *color));
                }
            }
            for units in buckets.into_values() {
                for (position, color) in units {
                    out.push(OcclusionOverlay {
                        position,
                        color: scale_occluded_player_color(color),
                        kind: OverlayKind::PlayerColor,
                    });
                }
            }
        }
    }
    let casters = UNIT_CASTERS.lock().map(|g| g.clone()).unwrap_or_default();
    for caster in casters {
        if caster.occluded {
            out.push(OcclusionOverlay {
                position: caster.position,
                color: scale_occluded_player_color(caster.player_color),
                kind: OverlayKind::PlayerColor,
            });
        }
        if caster.heat_vision {
            out.push(OcclusionOverlay {
                position: caster.position,
                color: [1.0, 0.35, 0.05, 0.85],
                kind: OverlayKind::HeatVision,
            });
        }
    }
    out
}

/// C++ `flushOccludedObjectsIntoStencil` + `renderStenciledPlayerColor`.
/// Live Main draws the classified silhouette via `occlusion_bridge`.
pub fn present_occluded_player_color_silhouette() -> Vec<OcclusionOverlay> {
    collect_occlusion_overlays()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    PlayerColor,
    HeatVision,
}

#[derive(Debug, Clone)]
pub struct OcclusionOverlay {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub kind: OverlayKind,
}

const VOLUME_OVERLAY_SHADER: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    shadow_color: vec4<f32>,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_volume(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return uniforms.view_proj * vec4<f32>(position, 1.0);
}

@fragment
fn fs_volume() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}

@vertex
fn vs_fill(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    var pos = array<vec2<f32>, 4>(
        vec2(-1.0, -1.0),
        vec2( 1.0, -1.0),
        vec2(-1.0,  1.0),
        vec2( 1.0,  1.0),
    );
    return vec4<f32>(pos[vertex_index], 0.999, 1.0);
}

@fragment
fn fs_fill() -> @location(0) vec4<f32> {
    return uniforms.shadow_color;
}

@vertex
fn vs_overlay(@location(0) position: vec3<f32>, @location(1) color: vec4<f32>) -> VsOut {
    var out: VsOut;
    out.position = uniforms.view_proj * vec4<f32>(position, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_overlay(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadowPassUniforms {
    view_proj: [[f32; 4]; 4],
    shadow_color: [f32; 4],
}

struct ShadowPassGpu {
    color_format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
    volume_pipeline: wgpu::RenderPipeline,
    fill_pipeline: wgpu::RenderPipeline,
    overlay_pipeline: wgpu::RenderPipeline,
    equivalent_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}
const VOLUME_VERTEX_ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];
const OVERLAY_VERTEX_ATTRS: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];

static SHADOW_PASS_GPU: Mutex<Option<ShadowPassGpu>> = Mutex::new(None);

fn create_shadow_pass_gpu(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
) -> ShadowPassGpu {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shadow_pass_volume_overlay"),
        source: wgpu::ShaderSource::Wgsl(VOLUME_OVERLAY_SHADER.into()),
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("shadow_pass_uniforms"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("shadow_pass_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let volume_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("shadow_volume_stencil"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_volume"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: 12,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &VOLUME_VERTEX_ATTRS,
            }],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_volume"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: None,
                write_mask: wgpu::ColorWrites::empty(),
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState {
                front: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::IncrementWrap,
                },
                back: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::DecrementWrap,
                },
                read_mask: 0xff,
                write_mask: 0xff,
            },
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });
    let dest_color_blend = wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Dst,
            dst_factor: wgpu::BlendFactor::Zero,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent::REPLACE,
    };
    let fill_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("shadow_stencil_fill"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_fill"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_fill"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(dest_color_blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::Always,
            stencil: wgpu::StencilState {
                front: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::NotEqual,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Keep,
                },
                back: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::NotEqual,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Keep,
                },
                read_mask: 0xff,
                write_mask: 0,
            },
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });
    let overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("occluded_player_color_overlay"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_overlay"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: 28,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &OVERLAY_VERTEX_ATTRS,
            }],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_overlay"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::Greater,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });
    let equivalent_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("shadow_volume_equivalent"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_volume"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: 12,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &VOLUME_VERTEX_ATTRS,
            }],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_fill"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(dest_color_blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });
    ShadowPassGpu {
        color_format,
        depth_format,
        volume_pipeline,
        fill_pipeline,
        overlay_pipeline,
        equivalent_pipeline,
        bind_group_layout,
    }
}

fn with_shadow_pass_gpu<R>(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
    f: impl FnOnce(&ShadowPassGpu) -> R,
) -> R {
    let mut slot = SHADOW_PASS_GPU.lock().unwrap_or_else(|e| e.into_inner());
    let rebuild = match slot.as_ref() {
        None => true,
        Some(gpu) => gpu.color_format != color_format || gpu.depth_format != depth_format,
    };
    if rebuild {
        *slot = Some(create_shadow_pass_gpu(device, color_format, depth_format));
    }
    f(slot.as_ref().expect("shadow gpu"))
}

fn extrusion_direction(light_pos: [f32; 3], origin: [f32; 3]) -> [f32; 3] {
    let lp = glam::Vec3::from_array(light_pos);
    let origin = glam::Vec3::from_array(origin);
    let mut dir = if lp.length() < 50.0 {
        let n = lp.normalize_or_zero();
        if n.length_squared() < 1e-6 {
            glam::Vec3::new(0.35, 0.35, -1.0).normalize()
        } else {
            -n
        }
    } else {
        (origin - lp).normalize_or_zero()
    };
    if dir.z > 0.0 {
        dir.z = -dir.z;
    }
    if dir.z.abs() < 0.15 {
        dir.z = -0.5;
        dir = dir.normalize_or_zero();
    }
    if dir.length_squared() < 1e-6 {
        dir = glam::Vec3::new(0.35, 0.2, -0.9).normalize();
    }
    dir.to_array()
}

fn push_volume_prism(
    vertices: &mut Vec<[f32; 3]>,
    indices: &mut Vec<u16>,
    caster: &UnitShadowCaster,
    light_pos: [f32; 3],
) {
    let hx = (caster.size_x * 0.5).max(2.0);
    let hy = (caster.size_y * 0.5).max(2.0);
    let height = hx.max(hy).max(8.0);
    let (s, c) = caster.angle.sin_cos();
    let mut near = [[0.0f32; 3]; 4];
    let signs = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    for (i, (sx, sy)) in signs.iter().enumerate() {
        let lx = sx * hx;
        let ly = sy * hy;
        near[i] = [
            caster.position[0] + lx * c - ly * s,
            caster.position[1] + lx * s + ly * c,
            caster.position[2] + height,
        ];
    }
    let dir = extrusion_direction(light_pos, caster.position);
    let len = (height / dir[2].abs().max(0.2)).min(400.0);
    let mut far = [[0.0f32; 3]; 4];
    for i in 0..4 {
        far[i] = [
            near[i][0] + dir[0] * len,
            near[i][1] + dir[1] * len,
            (near[i][2] + dir[2] * len).min(caster.position[2] + 0.05),
        ];
    }
    let base = vertices.len() as u16;
    vertices.extend_from_slice(&near);
    vertices.extend_from_slice(&far);
    let sides = [
        [0u16, 1, 5, 0, 5, 4],
        [1, 2, 6, 1, 6, 5],
        [2, 3, 7, 2, 7, 6],
        [3, 0, 4, 3, 4, 7],
        [0, 2, 1, 0, 3, 2],
        [4, 5, 6, 4, 6, 7],
    ];
    for tri in sides {
        for idx in tri {
            indices.push(base + idx);
        }
    }
}

fn push_overlay_billboard(
    vertices: &mut Vec<f32>,
    position: [f32; 3],
    camera: [f32; 3],
    color: [f32; 4],
) {
    let pos = glam::Vec3::from_array(position);
    let camera = glam::Vec3::from_array(camera);
    let to_cam = camera - pos;
    let to_cam_xy = glam::Vec3::new(to_cam.x, to_cam.y, 0.0);
    let forward = if to_cam_xy.length_squared() < 1e-6 {
        glam::Vec3::Y
    } else {
        to_cam_xy.normalize()
    };
    let mut right = glam::Vec3::Z.cross(forward);
    if right.length_squared() < 1e-6 {
        right = glam::Vec3::X;
    } else {
        right = right.normalize();
    }
    let half = 6.0f32;
    let center = pos + glam::Vec3::Z * half;
    let corners = [
        center - right * half - glam::Vec3::Z * half,
        center + right * half - glam::Vec3::Z * half,
        center + right * half + glam::Vec3::Z * half,
        center - right * half + glam::Vec3::Z * half,
    ];
    for index in [0usize, 1, 2, 0, 2, 3] {
        let p = corners[index];
        vertices.extend_from_slice(&[p.x, p.y, p.z, color[0], color[1], color[2], color[3]]);
    }
}

fn write_shadow_uniforms(
    device: &wgpu::Device,
    gpu: &ShadowPassGpu,
    view_proj: &glam::Mat4,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let uniforms = ShadowPassUniforms {
        view_proj: view_proj.to_cols_array_2d(),
        shadow_color: STENCIL_SHADOW_COLOR,
    };
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("shadow_pass_uniforms"),
        contents: bytemuck::bytes_of(&uniforms),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shadow_pass_bind_group"),
        layout: &gpu.bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });
    (buffer, bind_group)
}

/// Record C++ `renderShadows` + occluded player-color silhouettes into the live flush.
pub fn record_shadow_and_occlusion_passes(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    color_view: &wgpu::TextureView,
    depth_view: &wgpu::TextureView,
    view_proj: glam::Mat4,
    camera: [f32; 3],
    light_pos: [f32; 3],
    color_format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
) {
    let _ = present_volumetric_shadows(depth_format);
    let overlays = present_occluded_player_color_silhouette();
    let casters = collect_volume_casters();
    if casters.is_empty() && overlays.is_empty() {
        return;
    }
    with_shadow_pass_gpu(device, color_format, depth_format, |gpu| {
        let (_uniform_buf, bind_group) = write_shadow_uniforms(device, gpu, &view_proj);

        if !casters.is_empty() {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            for caster in &casters {
                push_volume_prism(&mut vertices, &mut indices, caster, light_pos);
            }
            if !indices.is_empty() {
                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("shadow_volume_vertices"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("shadow_volume_indices"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
                let stencil = depth_format_has_stencil(depth_format);
                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Display Volume Shadow Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: color_view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: stencil.then_some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                        }),
                        occlusion_query_set: None,
                        timestamp_writes: None,
                    });
                    pass.set_pipeline(if stencil {
                        &gpu.volume_pipeline
                    } else {
                        &gpu.equivalent_pipeline
                    });
                    pass.set_bind_group(0, &bind_group, &[]);
                    pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                    pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
                }
                if stencil {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Display Stencil Shadow Fill"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: color_view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                        }),
                        occlusion_query_set: None,
                        timestamp_writes: None,
                    });
                    pass.set_pipeline(&gpu.fill_pipeline);
                    pass.set_bind_group(0, &bind_group, &[]);
                    pass.draw(0..4, 0..1);
                }
            }
        }

        if overlays.is_empty() {
            return;
        }
        let mut overlay_verts = Vec::new();
        for overlay in &overlays {
            push_overlay_billboard(&mut overlay_verts, overlay.position, camera, overlay.color);
        }
        if overlay_verts.is_empty() {
            return;
        }
        let overlay_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("occlusion_overlay_vertices"),
            contents: bytemuck::cast_slice(&overlay_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Display Occlusion Overlay Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(&gpu.overlay_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.set_vertex_buffer(0, overlay_buffer.slice(..));
        pass.draw(0..(overlay_verts.len() / 7) as u32, 0..1);
    });
}

/// Terrain-and-sky AABB used by particle cull (C++ getMaximumVisibleBox(..., TRUE)).
#[derive(Debug, Clone, Copy)]
pub struct VisibleBox {
    pub center: [f32; 3],
    pub extent: [f32; 3],
}

impl VisibleBox {
    pub fn contains_expanded(&self, pos: [f32; 3], size: f32) -> bool {
        (pos[0] - self.center[0]).abs() <= self.extent[0] + size
            && (pos[1] - self.center[1]).abs() <= self.extent[1] + size
            && (pos[2] - self.center[2]).abs() <= self.extent[2] + size
    }
}

/// Clip frustum corners to the terrain min-height plane.
pub fn maximum_visible_box(
    camera: [f32; 3],
    target: [f32; 3],
    near_z: f32,
    far_z: f32,
    fov: f32,
    aspect: f32,
    min_height: f32,
) -> VisibleBox {
    let mut forward = [
        target[0] - camera[0],
        target[1] - camera[1],
        target[2] - camera[2],
    ];
    let fl = (forward[0] * forward[0] + forward[1] * forward[1] + forward[2] * forward[2]).sqrt();
    if fl > 1e-5 {
        forward[0] /= fl;
        forward[1] /= fl;
        forward[2] /= fl;
    } else {
        forward = [0.0, 1.0, 0.0];
    }
    let mut up = [0.0, 0.0, 1.0];
    if forward[2].abs() > 0.99 {
        up = [0.0, 1.0, 0.0];
    }
    let mut right = [
        forward[1] * up[2] - forward[2] * up[1],
        forward[2] * up[0] - forward[0] * up[2],
        forward[0] * up[1] - forward[1] * up[0],
    ];
    let rl = (right[0] * right[0] + right[1] * right[1] + right[2] * right[2])
        .sqrt()
        .max(1e-5);
    right[0] /= rl;
    right[1] /= rl;
    right[2] /= rl;
    up = [
        right[1] * forward[2] - right[2] * forward[1],
        right[2] * forward[0] - right[0] * forward[2],
        right[0] * forward[1] - right[1] * forward[0],
    ];

    let tan_half = (fov * 0.5).tan();
    let mut corners = [[0.0f32; 3]; 8];
    for (i, &dist) in [near_z, far_z].iter().enumerate() {
        let hh = dist * tan_half;
        let hw = hh * aspect;
        let c = [
            camera[0] + forward[0] * dist,
            camera[1] + forward[1] * dist,
            camera[2] + forward[2] * dist,
        ];
        let signs = [(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)];
        for (j, (sx, sy)) in signs.iter().enumerate() {
            corners[i * 4 + j] = [
                c[0] + right[0] * hw * sx + up[0] * hh * sy,
                c[1] + right[1] * hw * sx + up[1] * hh * sy,
                c[2] + right[2] * hw * sx + up[2] * hh * sy,
            ];
        }
    }
    // Clip far corners down to the ground plane (C++ ignoreMaxHeight = TRUE).
    for i in 0..4 {
        let a = corners[i];
        let b = corners[i + 4];
        let dz = b[2] - a[2];
        if dz.abs() > 1e-5 {
            let t = (min_height - a[2]) / dz;
            if (0.0..=1.0).contains(&t) {
                corners[i + 4] = [
                    a[0] + (b[0] - a[0]) * t,
                    a[1] + (b[1] - a[1]) * t,
                    min_height,
                ];
            }
        }
    }
    let mut min = corners[0];
    let mut max = corners[0];
    for c in &corners {
        for k in 0..3 {
            min[k] = min[k].min(c[k]);
            max[k] = max[k].max(c[k]);
        }
    }
    VisibleBox {
        center: [
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        ],
        extent: [
            (max[0] - min[0]) * 0.5,
            (max[1] - min[1]) * 0.5,
            (max[2] - min[2]) * 0.5,
        ],
    }
}

pub fn terrain_min_height() -> f32 {
    if let Ok(guard) = THE_TERRAIN_VISUAL.lock() {
        if let Some(terrain) = guard.as_ref() {
            if let Ok(h) = terrain.get_height_at(0.0, 0.0) {
                return h.min(0.0);
            }
        }
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_box_contains_camera_target() {
        let box_ = maximum_visible_box(
            [0.0, 0.0, 100.0],
            [0.0, 100.0, 0.0],
            1.0,
            500.0,
            1.0,
            1.0,
            0.0,
        );
        assert!(box_.contains_expanded([0.0, 50.0, 20.0], 5.0));
    }

    #[test]
    fn rebuild_increments_serial() {
        let before = shadow_rebuild_serial();
        rebuild_shadows();
        assert!(shadow_rebuild_serial() > before);
    }

    #[test]
    fn collect_unit_decal_items_does_not_invent_caster_blobs() {
        let before = collect_unit_decal_items().len();
        register_unit_shadow(UnitShadowCaster {
            position: [10.0, 20.0, 0.0],
            size_x: 12.0,
            size_y: 12.0,
            angle: 0.0,
            player_color: 0,
            occluded: false,
            heat_vision: false,
            volume: false,
        });
        let after = collect_unit_decal_items();
        clear_unit_shadows();
        assert_eq!(
            after.len(),
            before,
            "C++ flushDecals draws allocated addDecal/addShadow only"
        );
    }

    #[test]
    fn volumetric_present_skips_without_stencil() {
        assert!(!volumetric_stencil_supported());
        assert!(!depth_format_has_stencil(wgpu::TextureFormat::Depth32Float));
        assert!(depth_format_has_stencil(
            wgpu::TextureFormat::Depth24PlusStencil8
        ));
        assert_eq!(
            present_volumetric_shadows(wgpu::TextureFormat::Depth32Float),
            VolumetricPresentStatus::SkippedNoStencil
        );
    }

    #[test]
    fn volumetric_present_submits_volume_casters_when_stencil_available() {
        clear_unit_shadows();
        register_unit_shadow(UnitShadowCaster {
            position: [0.0, 0.0, 0.0],
            size_x: 8.0,
            size_y: 8.0,
            angle: 0.0,
            player_color: 0,
            occluded: false,
            heat_vision: false,
            volume: true,
        });
        register_unit_shadow(UnitShadowCaster {
            position: [4.0, 0.0, 0.0],
            size_x: 8.0,
            size_y: 8.0,
            angle: 0.0,
            player_color: 0,
            occluded: false,
            heat_vision: false,
            volume: false,
        });
        let status = present_volumetric_shadows(wgpu::TextureFormat::Depth24PlusStencil8);
        clear_unit_shadows();
        assert!(matches!(
            status,
            VolumetricPresentStatus::Submitted { volume_count } if volume_count >= 1
        ));
    }

    #[test]
    fn occluded_player_color_uses_hsv_luminance_and_half_alpha() {
        clear_unit_shadows();
        register_unit_shadow(UnitShadowCaster {
            position: [1.0, 2.0, 3.0],
            size_x: 4.0,
            size_y: 4.0,
            angle: 0.0,
            player_color: 0x00ff0000,
            occluded: true,
            heat_vision: false,
            volume: false,
        });
        let overlays = present_occluded_player_color_silhouette();
        clear_unit_shadows();
        let player = overlays
            .iter()
            .find(|overlay| overlay.kind == OverlayKind::PlayerColor)
            .expect("occluded caster must emit player-color silhouette");
        assert!((player.color[0] - 0.5).abs() < 1e-5);
        assert!(player.color[1].abs() < 1e-5);
        assert!(player.color[2].abs() < 1e-5);
        assert!((player.color[3] - OCCLUDED_COLOR_ALPHA).abs() < 1e-5);
        assert!(!overlays.iter().any(|overlay| {
            overlay.kind == OverlayKind::HeatVision && overlay.position == [1.0, 2.0, 3.0]
        }));
    }
}
