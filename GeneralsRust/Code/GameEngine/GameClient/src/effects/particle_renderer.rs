//! # GPU Particle Renderer
//!
//! High-performance GPU-accelerated particle rendering with WGPU.
//! Supports all C++ particle shader modes: additive, alpha, alpha test, multiply.
//! Uses instanced rendering and GPU compute shaders for maximum performance.

use bytemuck::{Pod, Zeroable};
use image::{DynamicImage, GenericImageView};
use nalgebra::{Matrix4, Point3, Vector3, Vector4};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use wgpu::util::DeviceExt;

use super::decals::DecalRenderItem;
use super::particle_manager::*;
use super::particle_system::{Particle, ParticleSystem};
use super::weather_complete::WeatherParticle;
use crate::system::smudge::{SmudgeSetHandle, get_smudge_manager};

/// C++ `W3DParticleSystemManager::MAX_POINTS_PER_GROUP`.
///
/// W3D builds and submits one point group per `ParticleSystem` in creation
/// order, with this fixed per-system cap.  It is deliberately not a global
/// renderer throughput limit: merging systems changes translucent blend order.
pub const MAX_PARTICLES_PER_BATCH: usize = 512;

/// C++ `SC_MUL_SPRITE`: SRCBLEND_ZERO, DSTBLEND_SRC_COLOR → dest * src.
pub fn particle_multiply_color_blend() -> wgpu::BlendComponent {
    wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::Src,
        operation: wgpu::BlendOperation::Add,
    }
}

/// Particle vertex data for GPU (matches C++ billboard rendering)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ParticleVertex {
    /// World position
    pub position: [f32; 3],
    /// Size (width, height, 0, 0)
    pub size: [f32; 2],
    /// Color (RGBA)
    pub color: [f32; 4],
    /// UV coordinates (u_min, v_min, u_max, v_max)
    pub uv_rect: [f32; 4],
    /// Rotation angle in radians
    pub rotation: f32,
    /// Alpha value (for separate alpha control)
    pub alpha: f32,
    /// 1 = camera billboard (C++ shouldBillboard), 0 = world-XZ ground quad (Y-up host).
    pub billboard: f32,
}

impl Default for ParticleVertex {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            size: [1.0, 1.0],
            color: [1.0; 4],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            rotation: 0.0,
            alpha: 1.0,
            billboard: 1.0,
        }
    }
}

/// GPU uniform data for particle rendering
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ParticleUniforms {
    /// View matrix
    pub view_matrix: [[f32; 4]; 4],
    /// Projection matrix  
    pub projection_matrix: [[f32; 4]; 4],
    /// Camera position
    pub camera_position: [f32; 3],
    /// Time for animation
    pub time: f32,
    /// Screen dimensions
    pub screen_size: [f32; 2],
    /// Particle count this frame
    pub particle_count: u32,
    /// Padding
    pub _padding: u32,
}

impl Default for ParticleUniforms {
    fn default() -> Self {
        Self {
            view_matrix: Matrix4::identity().into(),
            projection_matrix: Matrix4::identity().into(),
            camera_position: [0.0; 3],
            time: 0.0,
            screen_size: [1024.0, 768.0],
            particle_count: 0,
            _padding: 0,
        }
    }
}

/// Particle batch for rendering (groups particles by shader type and texture)
pub struct ParticleBatch {
    /// Shader type for this batch
    pub shader_type: ParticleShaderType,
    /// Texture name/path
    pub texture_name: String,
    /// Particle vertices
    pub vertices: Vec<ParticleVertex>,
    /// GPU vertex buffer
    pub vertex_buffer: Option<wgpu::Buffer>,
    /// Needs buffer update
    pub dirty: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DecalVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub uv: [f32; 2],
}

const SHADOW_DECAL_TYPE: u32 = 0x0000_0001;
const SHADOW_ADDITIVE_DECAL_TYPE: u32 = 0x0000_0040;
const DECAL_DRAPE_GRID: usize = 4;
const DECAL_HEIGHT_LIFT: f32 = 0.15;

fn decal_half_extents(decal: &DecalRenderItem) -> (f32, f32) {
    let hx = if decal.size_x > 0.0 {
        decal.size_x
    } else {
        decal.size
    } * 0.5;
    let hy = if decal.size_y > 0.0 {
        decal.size_y
    } else {
        decal.size
    } * 0.5;
    (hx.max(0.0), hy.max(0.0))
}

fn sample_decal_ground_z(x: f32, y: f32, fallback: f32) -> f32 {
    gamelogic::helpers::TheTerrainLogic::get()
        .map(|terrain| terrain.get_ground_height(x, y, None))
        .filter(|height| height.is_finite())
        .unwrap_or(fallback)
}

fn drape_decal_vertices(decal: &DecalRenderItem) -> Vec<DecalVertex> {
    let (half_x, half_y) = decal_half_extents(decal);
    if half_x <= 0.0 || half_y <= 0.0 {
        return Vec::new();
    }
    let sin_r = decal.rotation.sin();
    let cos_r = decal.rotation.cos();
    let base = decal.position;
    let color = decal.color;
    let cells = DECAL_DRAPE_GRID;
    let mut vertices = Vec::with_capacity(cells * cells * 6);
    for iy in 0..cells {
        let v0 = iy as f32 / cells as f32;
        let v1 = (iy + 1) as f32 / cells as f32;
        for ix in 0..cells {
            let u0 = ix as f32 / cells as f32;
            let u1 = (ix + 1) as f32 / cells as f32;
            let corners = [
                (
                    u0,
                    v0,
                    -half_x + u0 * half_x * 2.0,
                    -half_y + v0 * half_y * 2.0,
                ),
                (
                    u1,
                    v0,
                    -half_x + u1 * half_x * 2.0,
                    -half_y + v0 * half_y * 2.0,
                ),
                (
                    u0,
                    v1,
                    -half_x + u0 * half_x * 2.0,
                    -half_y + v1 * half_y * 2.0,
                ),
                (
                    u1,
                    v1,
                    -half_x + u1 * half_x * 2.0,
                    -half_y + v1 * half_y * 2.0,
                ),
            ];
            let mut world = [[0.0f32; 3]; 4];
            for (i, &(u, v, lx, ly)) in corners.iter().enumerate() {
                let _ = (u, v);
                let rot_x = lx * cos_r - ly * sin_r;
                let rot_y = lx * sin_r + ly * cos_r;
                let wx = base.x + rot_x;
                let wy = base.y + rot_y;
                let wz = sample_decal_ground_z(wx, wy, base.z) + DECAL_HEIGHT_LIFT;
                world[i] = [wx, wy, wz];
            }
            let verts = [
                DecalVertex {
                    position: world[0],
                    color,
                    uv: [corners[0].0, corners[0].1],
                },
                DecalVertex {
                    position: world[1],
                    color,
                    uv: [corners[1].0, corners[1].1],
                },
                DecalVertex {
                    position: world[2],
                    color,
                    uv: [corners[2].0, corners[2].1],
                },
                DecalVertex {
                    position: world[3],
                    color,
                    uv: [corners[3].0, corners[3].1],
                },
            ];
            vertices
                .extend_from_slice(&[verts[0], verts[1], verts[2], verts[2], verts[1], verts[3]]);
        }
    }
    vertices
}

impl ParticleBatch {
    pub fn new(shader_type: ParticleShaderType, texture_name: String) -> Self {
        Self {
            shader_type,
            texture_name,
            vertices: Vec::with_capacity(MAX_PARTICLES_PER_BATCH),
            vertex_buffer: None,
            dirty: true,
        }
    }

    /// Add particle to batch
    pub fn add_particle(&mut self, particle: &Particle, system: &ParticleSystem) {
        if self.vertices.len() >= MAX_PARTICLES_PER_BATCH {
            return; // Batch full
        }

        let vertex = particle_billboard_vertex(particle, system);

        self.vertices.push(vertex);
        self.dirty = true;
    }

    /// C++ `StreakLineClass`: one ribbon through all live particles.
    pub fn add_streak_polyline(&mut self, system: &ParticleSystem) {
        append_streak_polyline(&mut self.vertices, system);
        self.dirty = true;
    }

    /// Add layered volume-particle slices sorted from the camera-facing side.
    pub fn add_volume_particle(
        &mut self,
        particle: &Particle,
        system: &ParticleSystem,
        camera_position: [f32; 3],
    ) {
        let info = system.template().info();
        let layer_count = info
            .volume_particle_depth
            .max(OPTIMUM_VOLUME_PARTICLE_DEPTH)
            .min(MAX_VOLUME_PARTICLE_DEPTH) as usize;
        if layer_count == 0 {
            self.add_particle(particle, system);
            return;
        }

        let view_dir = Vector3::new(
            particle.position.x - camera_position[0],
            particle.position.y - camera_position[1],
            particle.position.z - camera_position[2],
        )
        .try_normalize(0.0001)
        .unwrap_or_else(|| Vector3::new(0.0, 0.0, 1.0));

        let spacing = particle.size / layer_count as f32;
        let first_offset = -0.5 * spacing * (layer_count.saturating_sub(1) as f32);

        for layer in 0..layer_count {
            if self.vertices.len() >= MAX_PARTICLES_PER_BATCH {
                break;
            }

            let mut vertex = particle_billboard_vertex(particle, system);
            let offset = first_offset + spacing * layer as f32;
            vertex.position[0] += view_dir.x * offset;
            vertex.position[1] += view_dir.y * offset;
            vertex.position[2] += view_dir.z * offset;
            vertex.alpha /= layer_count as f32;
            self.vertices.push(vertex);
        }

        self.dirty = true;
    }

    /// Clear batch
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.dirty = true;
    }

    /// Update GPU buffer
    pub fn update_buffer(&mut self, device: &wgpu::Device) {
        if !self.dirty || self.vertices.is_empty() {
            return;
        }

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Vertex Buffer"),
            contents: bytemuck::cast_slice(&self.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        self.vertex_buffer = Some(buffer);
        self.dirty = false;
    }
}

/// C++ PointGroup `!Billboard` keeps the ground-axis constant (Z-up world-XY).
/// Host is Y-up, so the live shader expands on world-XZ (`right=+X`, `up=+Z`).
#[must_use]
pub fn particle_ground_quad_axes() -> ([f32; 3], [f32; 3]) {
    ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0])
}

/// Expand one instanced particle to the four world corners the vertex shader emits.
#[must_use]
pub fn expand_particle_world_corners(vertex: &ParticleVertex) -> [[f32; 3]; 4] {
    let (right, up) = if vertex.billboard > 0.5 {
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
    } else {
        particle_ground_quad_axes()
    };
    let (cos_rot, sin_rot) = (vertex.rotation.cos(), vertex.rotation.sin());
    let corners_2d = [[-0.5, -0.5], [0.5, -0.5], [-0.5, 0.5], [0.5, 0.5]];
    let mut corners = [[0.0; 3]; 4];
    for (i, corner) in corners_2d.iter().enumerate() {
        let rx = corner[0] * cos_rot - corner[1] * sin_rot;
        let ry = corner[0] * sin_rot + corner[1] * cos_rot;
        corners[i] = [
            vertex.position[0] + right[0] * rx * vertex.size[0] + up[0] * ry * vertex.size[1],
            vertex.position[1] + right[1] * rx * vertex.size[0] + up[1] * ry * vertex.size[1],
            vertex.position[2] + right[2] * rx * vertex.size[0] + up[2] * ry * vertex.size[1],
        ];
    }
    corners
}

/// C++ billboard instance vertex used by the wgpu particle pass.
#[must_use]
pub fn bake_particle_gpu_vertex(particle: &Particle, system: &ParticleSystem) -> ParticleVertex {
    particle_billboard_vertex(particle, system)
}

pub fn bake_particle_system_gpu_mesh(system: &ParticleSystem) -> Vec<ParticleVertex> {
    let info = system.template().info();
    let mut vertices = Vec::new();
    if matches!(
        info.particle_type,
        ParticleType::Invalid | ParticleType::Drawable
    ) || system_is_heat_smudge(system)
    {
        return vertices;
    }
    if info.shader_type == ParticleShaderType::Invalid {
        return vertices;
    }
    if info.particle_type == ParticleType::Streak {
        append_streak_polyline(&mut vertices, system);
        return vertices;
    }
    for particle in system.particles() {
        if !particle.is_draw_alive() {
            continue;
        }
        match info.particle_type {
            ParticleType::VolumeParticle | ParticleType::Particle => {
                vertices.push(particle_billboard_vertex(particle, system))
            }
            ParticleType::Streak
            | ParticleType::Invalid
            | ParticleType::Drawable
            | ParticleType::Smudge => {}
        }
    }
    vertices
}

/// C++ `W3DParticleSys.cpp:143` DWORD prefix `0x44554D53` ("SMUD").
#[must_use]
pub fn particle_type_name_is_smud(name: &str) -> bool {
    name.as_bytes().get(..4) == Some(b"SMUD")
}

/// C++ `isUsingSmudge()` plus the SMUD* ParticleName hack.
/// `doParticles` comments out `isUsingSmudge()` and checks the DWORD prefix;
/// authored SMUDGE systems still use ParticleName `"SMUDGE RESERVED"`.
#[must_use]
pub fn system_is_heat_smudge(system: &ParticleSystem) -> bool {
    system.is_using_smudge()
        || particle_type_name_is_smud(&system.template().info().particle_type_name)
}

/// C++ `doParticles` start: `setSmudgeCountLastFrame(0)` then one `addSmudgeSet`.
pub fn begin_particle_heat_smudge_frame() {
    let Ok(mut manager) = get_smudge_manager().lock() else {
        return;
    };
    manager.set_smudge_count_last_frame(0);
    manager.reset();
    let _ = manager.add_smudge_set();
}

fn current_particle_heat_smudge_set() -> Option<SmudgeSetHandle> {
    let Ok(mut manager) = get_smudge_manager().lock() else {
        return None;
    };
    Some(
        manager
            .last_used_set()
            .unwrap_or_else(|| manager.add_smudge_set()),
    )
}

/// Convert a SMUD* / SMUDGE system into `TheSmudgeManager` heat smudges.
/// C++ `W3DParticleSys.cpp:142-172` — never drawn as sprites.
pub fn feed_system_heat_smudges(system: &ParticleSystem) -> usize {
    if !system_is_heat_smudge(system) {
        return 0;
    }
    let use_heat = game_engine::common::global_data::read_safe()
        .map(|data| data.use_heat_effects)
        .unwrap_or(true);
    {
        let Ok(manager) = get_smudge_manager().lock() else {
            return 0;
        };
        if !manager.get_hardware_support() || !use_heat {
            return 0;
        }
    }
    let Some(set) = current_particle_heat_smudge_set() else {
        return 0;
    };
    let mut visible = 0usize;
    if let Ok(mut set) = set.lock() {
        for particle in system.particles() {
            if !particle.is_draw_alive() {
                continue;
            }
            let smudge = set.add_smudge_to_set();
            smudge.pos = glam::Vec3::new(
                particle.position.x,
                particle.position.y,
                particle.position.z,
            );
            smudge.offset = glam::Vec2::new(
                crate::GameClientRandomValueReal!(-0.06, 0.06),
                crate::GameClientRandomValueReal!(-0.03, 0.03),
            );
            smudge.size = particle.size;
            smudge.opacity = particle.alpha;
            visible += 1;
        }
    }
    if let Ok(mut manager) = get_smudge_manager().lock() {
        let added = i32::try_from(visible).unwrap_or(i32::MAX);
        let prev = manager.get_smudge_count_last_frame();
        manager.set_smudge_count_last_frame(prev.saturating_add(added));
    }
    visible
}

fn particle_billboard_vertex(particle: &Particle, system: &ParticleSystem) -> ParticleVertex {
    ParticleVertex {
        position: [
            particle.position.x,
            particle.position.y,
            particle.position.z,
        ],
        size: [particle.size, particle.size],
        color: [particle.color[0], particle.color[1], particle.color[2], 1.0],
        uv_rect: [0.0, 0.0, 1.0, 1.0],
        rotation: particle.angle_z,
        alpha: particle.alpha,
        billboard: if system.should_billboard() { 1.0 } else { 0.0 },
    }
}

fn live_streak_particles(system: &ParticleSystem) -> Vec<&Particle> {
    system
        .particles()
        .iter()
        .filter(|particle| particle.is_draw_alive())
        .collect()
}

/// C++ W3DParticleSys.cpp:233-274 — one StreakLine through creation order.
fn append_streak_polyline(vertices: &mut Vec<ParticleVertex>, system: &ParticleSystem) {
    let live = live_streak_particles(system);
    if live.len() < 2 {
        return;
    }
    for index in 0..live.len() - 1 {
        if vertices.len() >= MAX_PARTICLES_PER_BATCH {
            break;
        }
        let mut vertex = particle_streak_segment(live[index], live[index + 1], system);
        if index == 0 {
            // C++ zeros RGBA[0] to kill the trailing scissor edge.
            vertex.color = [0.0, 0.0, 0.0, 0.0];
            vertex.alpha = 0.0;
        }
        vertices.push(vertex);
    }
}

fn particle_streak_segment(
    from: &Particle,
    to: &Particle,
    system: &ParticleSystem,
) -> ParticleVertex {
    let delta = to.position - from.position;
    let length = (delta.x * delta.x + delta.y * delta.y + delta.z * delta.z)
        .sqrt()
        .max(0.001);
    let midpoint = from.position + delta * 0.5;
    let mut vertex = particle_billboard_vertex(to, system);
    vertex.position = [midpoint.x, midpoint.y, midpoint.z];
    vertex.size = [length, to.size.max(0.001)];
    vertex.rotation = delta.y.atan2(delta.x);
    vertex
}

/// GPU particle renderer
pub struct ParticleRenderer {
    /// Graphics device
    device: Arc<wgpu::Device>,
    /// Command queue
    queue: Arc<wgpu::Queue>,

    /// Render pipelines for different shader modes
    additive_pipeline: wgpu::RenderPipeline,
    alpha_pipeline: wgpu::RenderPipeline,
    alpha_test_pipeline: wgpu::RenderPipeline,
    multiply_pipeline: wgpu::RenderPipeline,
    decal_pipeline: wgpu::RenderPipeline,
    decal_modulate_pipeline: wgpu::RenderPipeline,
    decal_additive_pipeline: wgpu::RenderPipeline,
    heat_haze_pipeline: wgpu::RenderPipeline,

    /// Uniform buffer
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,

    /// Texture atlas for particle textures
    texture_atlas: HashMap<String, wgpu::Texture>,
    texture_bind_groups: HashMap<String, wgpu::BindGroup>,
    /// Texture names that were definitively absent or undecodable.  Keep a
    /// fail-closed default texture, but do not re-query the archive every
    /// frame for an authored asset that is not installed.
    unavailable_textures: HashSet<String>,

    /// Current-frame particle submissions, one per live `ParticleSystem` in
    /// the manager's creation order.  C++ `W3DParticleSystemManager::doParticles`
    /// renders each system before advancing its list iterator; do not coalesce
    /// equal texture/shader systems through a `HashMap`, because alpha and
    /// additive composition are order-visible.
    batches: Vec<ParticleBatch>,

    /// Default texture for particles without specific texture
    default_texture: wgpu::Texture,
    default_bind_group: wgpu::BindGroup,
    /// C++ `W3DSmudgeManager::m_backgroundTexture` — COPY_SRC scene snapshot.
    heat_haze_sampler: wgpu::Sampler,
    scene_copy: Option<wgpu::Texture>,
    scene_copy_bind_group: Option<wgpu::BindGroup>,

    /// Billboard vertices (quad)
    billboard_buffer: wgpu::Buffer,

    /// Performance stats
    pub stats: ParticleRenderStats,
}

/// The currently active WGPU owner for GameClient particle textures.
///
/// A standalone `Display` and Main's shared-frame WGPU renderer can be
/// initialized in either order during shell/game transitions.  `OnceLock` is
/// used only for the slot itself: retaining the first renderer forever makes
/// later asset uploads land in a surface that is no longer presented.  The
/// active owner is therefore replaceable, and readers clone its `Arc` before
/// doing any potentially re-entrant GPU work.
static PARTICLE_RENDERER: OnceLock<RwLock<Option<Arc<Mutex<ParticleRenderer>>>>> = OnceLock::new();

fn particle_renderer_slot() -> &'static RwLock<Option<Arc<Mutex<ParticleRenderer>>>> {
    PARTICLE_RENDERER.get_or_init(|| RwLock::new(None))
}

pub fn register_particle_renderer(renderer: Arc<Mutex<ParticleRenderer>>) {
    if let Ok(mut slot) = particle_renderer_slot().write() {
        *slot = Some(renderer);
    }
}

pub fn with_particle_renderer<R>(f: impl FnOnce(&Arc<Mutex<ParticleRenderer>>) -> R) -> Option<R> {
    let renderer = particle_renderer_slot()
        .read()
        .ok()?
        .as_ref()
        .map(Arc::clone)?;
    Some(f(&renderer))
}

/// Particle rendering statistics
#[derive(Debug, Default)]
pub struct ParticleRenderStats {
    pub particles_rendered: usize,
    pub batches_rendered: usize,
    pub draw_calls: usize,
    pub gpu_memory_used: usize,
    pub render_time_ms: f64,
}

impl ParticleRenderStats {
    fn reset_frame_counters(&mut self) {
        self.particles_rendered = 0;
        self.batches_rendered = 0;
        self.draw_calls = 0;
        self.render_time_ms = 0.0;
    }
}

impl ParticleRenderer {
    /// Create new particle renderer
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with_depth_format(
            device,
            queue,
            surface_format,
            wgpu::TextureFormat::Depth32Float,
        )
    }

    /// Create a particle renderer for an existing WGPU frame target.
    ///
    /// [`Self::new`] keeps the standalone GameClient display on its C++-matching
    /// `Depth32Float` target. A host that owns a different WGPU frame lifecycle
    /// must make particle pipelines agree with that target instead of assuming
    /// the two depth attachments are interchangeable.
    pub fn new_with_depth_format(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Uniforms"),
            size: std::mem::size_of::<ParticleUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout for uniforms
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Particle Uniform Bind Group Layout"),
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

        // Create bind group layout for textures
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Particle Texture Bind Group Layout"),
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
                ],
            });

        // Create uniform bind group
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particle Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create render pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Particle Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Load shaders
        let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Vertex Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particle_vertex.wgsl").into()),
        });

        let additive_fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Additive Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particle_additive.wgsl").into()),
        });

        let alpha_fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Alpha Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particle_alpha.wgsl").into()),
        });

        let alpha_test_fragment_shader =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Particle Alpha Test Fragment Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("shaders/particle_alpha_test.wgsl").into(),
                ),
            });

        let multiply_fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Multiply Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particle_multiply.wgsl").into()),
        });

        let decal_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Decal Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/decal.wgsl").into()),
        });

        // Create vertex buffer layout
        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ParticleVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 12,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 20,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 36,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 52,
                    shader_location: 4,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 56,
                    shader_location: 5,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 60,
                    shader_location: 8,
                },
            ],
        };

        // Create billboard quad vertices
        #[repr(C)]
        #[derive(Clone, Copy, Pod, Zeroable)]
        struct BillboardVertex {
            position: [f32; 2],
            tex_coord: [f32; 2],
        }

        let billboard_vertices = [
            BillboardVertex {
                position: [-0.5, -0.5],
                tex_coord: [0.0, 1.0],
            },
            BillboardVertex {
                position: [0.5, -0.5],
                tex_coord: [1.0, 1.0],
            },
            BillboardVertex {
                position: [-0.5, 0.5],
                tex_coord: [0.0, 0.0],
            },
            BillboardVertex {
                position: [0.5, 0.5],
                tex_coord: [1.0, 0.0],
            },
        ];

        let billboard_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Billboard Vertex Buffer"),
            contents: bytemuck::cast_slice(&billboard_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let billboard_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<BillboardVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 6,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 7,
                },
            ],
        };

        let decal_vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<DecalVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 12,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 28,
                    shader_location: 2,
                },
            ],
        };

        // Create additive blend pipeline
        let additive_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Additive Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("vs_main"),
                buffers: &[billboard_layout.clone(), vertex_buffer_layout.clone()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &additive_fragment_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
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
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false, // Particles don't write depth
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create alpha blend pipeline
        let alpha_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Alpha Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("vs_main"),
                buffers: &[billboard_layout.clone(), vertex_buffer_layout.clone()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &alpha_fragment_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create alpha test pipeline
        let alpha_test_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Alpha Test Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("vs_main"),
                buffers: &[billboard_layout.clone(), vertex_buffer_layout.clone()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &alpha_test_fragment_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None, // No blending for alpha test
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
                depth_write_enabled: true, // Alpha test writes depth
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create multiply pipeline
        let multiply_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Multiply Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("vs_main"),
                buffers: &[billboard_layout.clone(), vertex_buffer_layout.clone()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &multiply_fragment_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: particle_multiply_color_blend(),
                        alpha: wgpu::BlendComponent::OVER,
                    }),
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
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let heat_haze_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Heat Haze Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/particle_heat_haze.wgsl").into(),
            ),
        });
        let heat_haze_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<crate::effects::heat_haze::HeatHazeGpuVertex>()
                as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 12,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 20,
                    shader_location: 2,
                },
            ],
        };
        let heat_haze_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Heat Haze Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &heat_haze_shader,
                entry_point: Some("vs_main"),
                buffers: &[heat_haze_layout],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &heat_haze_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
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

        let decal_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Decal Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &decal_shader,
                entry_point: Some("vs_main"),
                buffers: &[decal_vertex_layout.clone()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &decal_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
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

        let decal_modulate_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Decal Modulate Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &decal_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[decal_vertex_layout.clone()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &decal_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(wgpu::BlendState {
                            color: particle_multiply_color_blend(),
                            alpha: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::Zero,
                                dst_factor: wgpu::BlendFactor::One,
                                operation: wgpu::BlendOperation::Add,
                            },
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
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

        let decal_additive_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Decal Additive Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &decal_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[decal_vertex_layout],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &decal_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::SrcAlpha,
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
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
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

        // Create default white texture
        let default_texture = Self::create_default_texture(&device, &queue);
        let default_bind_group =
            Self::create_texture_bind_group(&device, &texture_bind_group_layout, &default_texture);
        let heat_haze_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Heat Haze Scene Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            device,
            queue,

            additive_pipeline,
            alpha_pipeline,
            alpha_test_pipeline,
            multiply_pipeline,
            decal_pipeline,
            decal_modulate_pipeline,
            decal_additive_pipeline,
            heat_haze_pipeline,

            uniform_buffer,
            uniform_bind_group,
            texture_bind_group_layout,

            texture_atlas: HashMap::new(),
            texture_bind_groups: HashMap::new(),
            unavailable_textures: HashSet::new(),

            batches: Vec::new(),

            default_texture,
            default_bind_group,
            heat_haze_sampler,
            scene_copy: None,
            scene_copy_bind_group: None,

            billboard_buffer,

            stats: ParticleRenderStats::default(),
        })
    }

    /// Render all particle systems
    pub fn render_particles(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        systems: &[&ParticleSystem],
        uniforms: &ParticleUniforms,
    ) {
        let start_time = std::time::Instant::now();
        self.stats.reset_frame_counters();

        // Update uniforms
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*uniforms]));

        // C++ uses one transient point group submission for each live system.
        // Clearing the frame list preserves that order rather than retaining a
        // cross-system texture bucket from an earlier frame.
        self.batches.clear();
        // C++ `doParticles` allocates one smudge set, then SMUD* systems
        // `addSmudgeToSet` instead of filling the point-group buffers.
        begin_particle_heat_smudge_frame();

        // Collect particles into batches
        for system in systems {
            self.collect_system_particles(system, uniforms.camera_position);
        }

        // C++ ParticleSystemManager preloads direct `ParticleName` textures.
        // The Rust WGPU renderer owns its own texture atlas, so hydrate an
        // authored texture the first time a live batch needs it.  This uses
        // the normal BIG-backed image resolver rather than a guessed path.
        let texture_names: Vec<String> = self
            .batches
            .iter()
            .filter(|batch| !batch.vertices.is_empty())
            .map(|batch| batch.texture_name.clone())
            .collect();
        for texture_name in texture_names {
            self.ensure_authored_texture_loaded(&texture_name);
        }

        // Update GPU buffers for batches
        for batch in &mut self.batches {
            batch.update_buffer(&self.device);
        }

        // Render batches
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Particle Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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

            // Set uniform bind group
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            // Set billboard vertices
            render_pass.set_vertex_buffer(0, self.billboard_buffer.slice(..));

            let mut rendered_batches = 0usize;

            // Render each batch
            for batch in &self.batches {
                if batch.vertices.is_empty() || batch.vertex_buffer.is_none() {
                    continue;
                }

                // Select pipeline based on shader type
                match batch.shader_type {
                    ParticleShaderType::Invalid => continue,
                    ParticleShaderType::Additive => {
                        render_pass.set_pipeline(&self.additive_pipeline);
                    }
                    ParticleShaderType::Alpha => {
                        render_pass.set_pipeline(&self.alpha_pipeline);
                    }
                    ParticleShaderType::AlphaTest => {
                        render_pass.set_pipeline(&self.alpha_test_pipeline);
                    }
                    ParticleShaderType::Multiply => {
                        render_pass.set_pipeline(&self.multiply_pipeline);
                    }
                }

                // Set texture bind group
                let texture_bind_group = self
                    .texture_bind_groups
                    .get(&batch.texture_name)
                    .unwrap_or(&self.default_bind_group);
                render_pass.set_bind_group(1, texture_bind_group, &[]);

                // Set particle data
                render_pass.set_vertex_buffer(1, batch.vertex_buffer.as_ref().unwrap().slice(..));

                // Draw instanced
                render_pass.draw(0..4, 0..batch.vertices.len() as u32);

                self.stats.draw_calls += 1;
                rendered_batches += 1;
            }

            self.stats.batches_rendered = rendered_batches;
        }

        self.stats.render_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    }

    /// Render weather particles (rain/snow/dust) using the alpha pipeline.
    pub fn render_weather_particles(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        particles: &[WeatherParticle],
        uniforms: &ParticleUniforms,
    ) {
        if particles.is_empty() {
            return;
        }

        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*uniforms]));

        let mut vertices = Vec::with_capacity(particles.len());
        for particle in particles {
            if particle.age >= particle.lifetime || particle.alpha <= 0.0 {
                continue;
            }
            let alpha = (particle.alpha * particle.color[3]).clamp(0.0, 1.0);
            let vertex = ParticleVertex {
                position: [
                    particle.position.x,
                    particle.position.y,
                    particle.position.z,
                ],
                size: [particle.size, particle.size],
                color: [particle.color[0], particle.color[1], particle.color[2], 1.0],
                uv_rect: [0.0, 0.0, 1.0, 1.0],
                rotation: particle.rotation,
                alpha,
                billboard: 1.0,
            };
            vertices.push(vertex);
        }

        if vertices.is_empty() {
            return;
        }

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Weather Particle Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let start_time = std::time::Instant::now();
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Weather Particle Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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

            render_pass.set_pipeline(&self.alpha_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &self.default_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.billboard_buffer.slice(..));
            render_pass.set_vertex_buffer(1, vertex_buffer.slice(..));
            render_pass.draw(0..4, 0..vertices.len() as u32);
        }

        self.stats.draw_calls += 1;
        self.stats.particles_rendered += vertices.len();
        self.stats.render_time_ms += start_time.elapsed().as_secs_f64() * 1000.0;
    }

    /// Submit FXList tracer streaks and ray beams into the live wgpu pass.
    pub fn render_tracer_and_ray_fx(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        uniforms: &ParticleUniforms,
    ) {
        let mut vertices = Vec::new();
        for mesh in crate::effects::tracer_fx::bake_all_tracer_gpu_meshes() {
            if mesh.vertices.len() < 4 {
                continue;
            }
            let start = mesh.vertices[0].position;
            let end = mesh.vertices[2].position;
            let dx = end[0] - start[0];
            let dy = end[1] - start[1];
            let dz = end[2] - start[2];
            let length = (dx * dx + dy * dy + dz * dz).sqrt().max(0.001);
            let color = mesh.vertices[0].color;
            vertices.push(ParticleVertex {
                position: [
                    (start[0] + end[0]) * 0.5,
                    (start[1] + end[1]) * 0.5,
                    (start[2] + end[2]) * 0.5,
                ],
                size: [
                    length,
                    mesh.vertices.len() as f32 * 0.0 + {
                        let p0 = mesh.vertices[0].position;
                        let p1 = mesh.vertices[1].position;
                        let wx = p1[0] - p0[0];
                        let wy = p1[1] - p0[1];
                        let wz = p1[2] - p0[2];
                        (wx * wx + wy * wy + wz * wz).sqrt().max(0.05)
                    },
                ],
                color,
                uv_rect: [0.0, 0.0, 1.0, 1.0],
                rotation: dy.atan2(dx),
                alpha: color[3],
                billboard: 1.0,
            });
        }
        for ray in crate::effects::ray_effect_system::live_ray_effects() {
            let dx = ray.end[0] - ray.start[0];
            let dy = ray.end[1] - ray.start[1];
            let dz = ray.end[2] - ray.start[2];
            let length = (dx * dx + dy * dy + dz * dz).sqrt().max(0.001);
            let alpha = ray.width_scalar.clamp(0.0, 1.0);
            let width = (ray.outer_beam_width * alpha.max(0.05)).max(0.05);
            let mut color = ray.color;
            color[3] *= alpha;
            if !ray.texture_name.is_empty() {
                self.ensure_authored_texture_loaded(&ray.texture_name);
            }
            vertices.push(ParticleVertex {
                position: ray.midpoint,
                size: [length, width],
                color,
                uv_rect: [0.0, 0.0, 1.0, 1.0],
                rotation: dy.atan2(dx),
                alpha,
                billboard: 1.0,
            });
        }
        if vertices.is_empty() {
            return;
        }
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*uniforms]));
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Tracer Ray FX Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Tracer Ray FX Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            render_pass.set_pipeline(&self.additive_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &self.default_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.billboard_buffer.slice(..));
            render_pass.set_vertex_buffer(1, vertex_buffer.slice(..));
            render_pass.draw(0..4, 0..vertices.len() as u32);
        }
        self.stats.draw_calls += 1;
        self.stats.particles_rendered += vertices.len();
    }

    pub fn render_heat_haze(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        scene_source: Option<&wgpu::Texture>,
        smudges: &[crate::effects::heat_haze::HeatHazeSmudge],
        uniforms: &ParticleUniforms,
    ) {
        let (vertices, indices) = crate::effects::heat_haze::heat_haze_gpu_mesh(
            smudges,
            &uniforms.view_matrix,
            &uniforms.projection_matrix,
            [0.5, 0.5],
            [1.0, 1.0],
        );
        if vertices.is_empty() || indices.is_empty() {
            return;
        }
        // C++ `W3DSmudgeManager::render` CopyRects of the backbuffer into
        // `m_backgroundTexture` so the 5-vertex quads can sample the scene
        // while writing the color target.
        if let Some(source) = scene_source {
            self.copy_scene_for_heat_haze(encoder, source);
        }
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*uniforms]));
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Heat Haze Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Heat Haze Index Buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Heat Haze Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            render_pass.set_pipeline(&self.heat_haze_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            let scene_bind_group = self
                .scene_copy_bind_group
                .as_ref()
                .unwrap_or(&self.default_bind_group);
            render_pass.set_bind_group(1, scene_bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }
        self.stats.draw_calls += 1;
    }

    pub fn render_decals(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        decals: &[DecalRenderItem],
        uniforms: &ParticleUniforms,
    ) {
        if decals.is_empty() {
            return;
        }

        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*uniforms]));

        // C++ flushDecals batches by texture + ShadowType. Group the same way.
        let mut groups: Vec<(String, u32, Vec<DecalVertex>)> = Vec::new();
        for decal in decals {
            let verts = drape_decal_vertices(decal);
            if verts.is_empty() {
                continue;
            }
            if !decal.texture_name.is_empty() {
                self.ensure_authored_texture_loaded(&decal.texture_name);
            }
            let key = (decal.texture_name.clone(), decal.shadow_type);
            if let Some((_, _, existing)) = groups
                .iter_mut()
                .find(|(tex, kind, _)| tex == &key.0 && *kind == key.1)
            {
                existing.extend(verts);
            } else {
                groups.push((key.0, key.1, verts));
            }
        }

        if groups.is_empty() {
            return;
        }

        let start_time = std::time::Instant::now();
        for (texture_name, shadow_type, vertices) in groups {
            let vertex_buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Decal Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Decal Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
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

                let pipeline = if shadow_type == SHADOW_ADDITIVE_DECAL_TYPE {
                    &self.decal_additive_pipeline
                } else if shadow_type == SHADOW_DECAL_TYPE {
                    &self.decal_modulate_pipeline
                } else {
                    &self.decal_pipeline
                };
                let bind_group = self
                    .texture_bind_groups
                    .get(&texture_name)
                    .unwrap_or(&self.default_bind_group);
                render_pass.set_pipeline(pipeline);
                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                render_pass.set_bind_group(1, bind_group, &[]);
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.draw(0..vertices.len() as u32, 0..1);
            }
            self.stats.draw_calls += 1;
            self.stats.particles_rendered += vertices.len();
        }
        self.stats.render_time_ms += start_time.elapsed().as_secs_f64() * 1000.0;
    }

    /// Collect particles from a system into appropriate batches
    fn collect_system_particles(&mut self, system: &ParticleSystem, camera_position: [f32; 3]) {
        let template = system.template();
        let info = template.info();
        if matches!(
            info.particle_type,
            ParticleType::Invalid | ParticleType::Drawable
        ) {
            return;
        }
        // C++ DWORD "SMUD" prefix / Type=SMUDGE: convert visible particles
        // into heat smudges and continue — never submit as sprites
        // (`W3DParticleSys.cpp:142-172`).
        if system_is_heat_smudge(system) {
            let _ = feed_system_heat_smudges(system);
            return;
        }
        if info.shader_type == ParticleShaderType::Invalid {
            return;
        }

        let texture_name = if info.particle_type_name.is_empty() {
            "default".to_string()
        } else {
            info.particle_type_name.clone()
        };

        // `W3DParticleSystemManager::doParticles` renders every live system
        // immediately in `m_allParticleSystemList` order.  Keep a separate
        // submission even when adjacent systems share a texture/shader.
        let mut batch = ParticleBatch::new(info.shader_type, texture_name);

        if info.particle_type == ParticleType::Streak {
            let before = batch.vertices.len();
            batch.add_streak_polyline(system);
            self.stats.particles_rendered += batch.vertices.len().saturating_sub(before);
        } else {
            for particle in system.particles() {
                if !particle.is_draw_alive() {
                    continue;
                }
                match info.particle_type {
                    ParticleType::VolumeParticle => {
                        let before = batch.vertices.len();
                        batch.add_volume_particle(particle, system, camera_position);
                        self.stats.particles_rendered +=
                            batch.vertices.len().saturating_sub(before);
                        continue;
                    }
                    ParticleType::Particle => batch.add_particle(particle, system),
                    ParticleType::Streak
                    | ParticleType::Invalid
                    | ParticleType::Drawable
                    | ParticleType::Smudge => continue,
                }
                self.stats.particles_rendered += 1;
            }
        }

        // C++ skips texture lookup and draw submission for an empty system.
        if !batch.vertices.is_empty() {
            self.batches.push(batch);
        }
    }

    /// Load texture for particles
    pub fn load_texture(
        &mut self,
        name: &str,
        texture_data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        if texture_data.is_empty() {
            return Err("Texture data is empty".into());
        }

        let image = image::load_from_memory(texture_data)?;
        self.load_texture_image(name, &image)
    }

    /// Upload an already decoded texture.  The engine filesystem resolver
    /// produces decoded TGA/DDS images for authored particle names, while the
    /// asset pipeline uses [`Self::load_texture`] for raw loaded bytes.
    fn load_texture_image(
        &mut self,
        name: &str,
        image: &DynamicImage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let rgba = image.to_rgba8();
        let (width, height) = image.dimensions();

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Particle Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let bind_group = Self::create_texture_bind_group(
            &self.device,
            &self.texture_bind_group_layout,
            &texture,
        );

        self.texture_atlas.insert(name.to_string(), texture);
        self.texture_bind_groups
            .insert(name.to_string(), bind_group);
        // An archive may become available after an early presentation frame,
        // or AssetManager may upload the same texture later.  A successful
        // upload must reopen the exact-name path rather than preserve an old
        // miss forever.
        self.unavailable_textures.remove(name);
        self.stats.gpu_memory_used = self.stats.gpu_memory_used.saturating_add(
            (width as usize)
                .saturating_mul(height as usize)
                .saturating_mul(4),
        );

        Ok(())
    }

    fn ensure_authored_texture_loaded(&mut self, name: &str) {
        if name == "default"
            || self.texture_bind_groups.contains_key(name)
            || self.unavailable_textures.contains(name)
        {
            return;
        }

        match crate::display::image::load_image_from_engine_filesystem(name) {
            Ok(image) => {
                if let Err(error) = self.load_texture_image(name, &image) {
                    log::warn!("failed to upload particle texture {name}: {error}");
                    self.unavailable_textures.insert(name.to_string());
                }
            }
            Err(error) => {
                log::debug!("particle texture {name} is unavailable: {error}");
                self.unavailable_textures.insert(name.to_string());
            }
        }
    }

    /// Preload exact `ParticleName` assets before the first live effect frame.
    ///
    /// This is the WGPU counterpart of C++
    /// `ParticleSystemManager::preloadAssets` → `Display::preloadTextureAssets`.
    /// It is also safe to call before a renderer is registered: the live draw
    /// path repeats the same idempotent check if a texture was not preloaded.
    pub fn preload_authored_textures(&mut self, names: &[String]) {
        for name in names {
            // The explicit C++ preload phase happens after asset initialization
            // and is the right time to retry a one-off early lookup miss.
            self.unavailable_textures.remove(name);
            self.ensure_authored_texture_loaded(name);
        }
    }

    /// Create default white texture
    fn create_default_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default Particle Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload white pixel.  The old implementation encoded a copy but
        // never submitted that encoder, leaving fallback particles with
        // undefined GPU contents.  A queue write is immediate and matches the
        // texture upload path used for actual authored particle images.
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        texture
    }

    /// Create bind group for texture
    fn create_texture_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        texture: &wgpu::Texture,
    ) -> wgpu::BindGroup {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Particle Texture Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particle Texture Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        })
    }

    /// C++ `W3DSmudgeManager::render` `background->Copy(..., backBuffer)`.
    fn copy_scene_for_heat_haze(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Texture,
    ) {
        if !source.usage().contains(wgpu::TextureUsages::COPY_SRC) || source.sample_count() != 1 {
            return;
        }
        let size = source.size();
        let format = source.format();
        let rebuild = self
            .scene_copy
            .as_ref()
            .is_none_or(|tex| tex.size() != size || tex.format() != format);
        if rebuild {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Heat Haze Scene Copy"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Heat Haze Scene Bind Group"),
                layout: &self.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.heat_haze_sampler),
                    },
                ],
            });
            self.scene_copy = Some(texture);
            self.scene_copy_bind_group = Some(bind_group);
        }
        let Some(dest) = self.scene_copy.as_ref() else {
            return;
        };
        encoder.copy_texture_to_texture(
            source.as_image_copy(),
            dest.as_image_copy(),
            wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::particle_system::ParticleInfo;
    use super::*;
    use parking_lot::Mutex;

    static HEAT_SMUDGE_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn test_system(particle_type: ParticleType, depth: u32) -> ParticleSystem {
        let mut template = ParticleSystemTemplate::new("test".to_string());
        template.info_mut().particle_type = particle_type;
        template.info_mut().volume_particle_depth = depth;
        ParticleSystem::new(Arc::new(template), 1, false)
    }

    #[test]
    fn test_particle_vertex_layout() {
        // Test that ParticleVertex is correctly sized and aligned
        assert_eq!(std::mem::size_of::<ParticleVertex>(), 64);
        assert_eq!(std::mem::align_of::<ParticleVertex>(), 4);
        assert_eq!(std::mem::offset_of!(ParticleVertex, position), 0);
        assert_eq!(std::mem::offset_of!(ParticleVertex, size), 12);
        assert_eq!(std::mem::offset_of!(ParticleVertex, color), 20);
        assert_eq!(std::mem::offset_of!(ParticleVertex, uv_rect), 36);
        assert_eq!(std::mem::offset_of!(ParticleVertex, rotation), 52);
        assert_eq!(std::mem::offset_of!(ParticleVertex, alpha), 56);
        assert_eq!(std::mem::offset_of!(ParticleVertex, billboard), 60);
    }

    #[test]
    fn test_particle_batch() {
        let mut batch = ParticleBatch::new(ParticleShaderType::Alpha, "test.tga".to_string());
        assert_eq!(batch.vertices.len(), 0);
        assert!(batch.dirty);
    }
    #[test]
    fn streak_system_emits_polyline_segments_in_creation_order() {
        let mut system = test_system(ParticleType::Streak, 0);
        let mut first = ParticleInfo::default();
        first.position = Point3::new(0.0, 0.0, 2.0);
        first.size = 0.5;
        first.color_keys[0].color = [1.0, 0.0, 0.0];
        let mut second = ParticleInfo::default();
        second.position = Point3::new(4.0, 3.0, 2.0);
        second.size = 0.5;
        second.color_keys[0].color = [0.0, 1.0, 0.0];
        let mut third = ParticleInfo::default();
        third.position = Point3::new(4.0, 3.0, 6.0);
        third.size = 0.25;
        third.color_keys[0].color = [0.0, 0.0, 1.0];
        system.push_particle(Particle::new(&first, 0, 0));
        system.push_particle(Particle::new(&second, 1, 0));
        system.push_particle(Particle::new(&third, 2, 0));

        let vertices = bake_particle_system_gpu_mesh(&system);
        assert_eq!(vertices.len(), 2);
        assert_eq!(vertices[0].position, [2.0, 1.5, 2.0]);
        assert_eq!(vertices[0].size[0], 5.0);
        assert_eq!(vertices[0].color, [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(vertices[0].alpha, 0.0);
        assert_eq!(vertices[1].position, [4.0, 3.0, 4.0]);
        assert_eq!(vertices[1].size[0], 4.0);
        assert_eq!(vertices[1].size[1], 0.25);
        assert_eq!(vertices[1].color[2], 1.0);
    }

    #[test]
    fn ground_aligned_particles_disable_camera_billboard() {
        let mut template = ParticleSystemTemplate::new("ground".to_string());
        template.info_mut().particle_type = ParticleType::Particle;
        template.info_mut().is_ground_aligned = true;
        let system = ParticleSystem::new(Arc::new(template), 1, false);
        let info = ParticleInfo::default();
        let particle = Particle::new(&info, 0, 0);
        let vertex = bake_particle_gpu_vertex(&particle, &system);
        assert_eq!(vertex.billboard, 0.0);
        assert!(!system.should_billboard());
    }

    /// C++ PointGroup `!Billboard` keeps Z constant (Z-up ground = world-XY).
    /// Host is Y-up, so the live collect path must emit world-XZ quads.
    #[test]
    fn collect_system_particles_emits_world_xz_quads_when_ground_aligned() {
        let mut template = ParticleSystemTemplate::new("ground".to_string());
        template.info_mut().particle_type = ParticleType::Particle;
        template.info_mut().is_ground_aligned = true;
        let mut system = ParticleSystem::new(Arc::new(template), 1, false);
        let mut info = ParticleInfo::default();
        info.position = Point3::new(10.0, 5.0, 20.0);
        info.size = 4.0;
        system.push_particle(Particle::new(&info, 0, 0));

        let vertices = bake_particle_system_gpu_mesh(&system);
        assert_eq!(vertices.len(), 1);
        assert_eq!(vertices[0].billboard, 0.0);
        assert!(!system.should_billboard());

        let corners = expand_particle_world_corners(&vertices[0]);
        for corner in corners {
            assert!(
                (corner[1] - 5.0).abs() < 1e-5,
                "ground-aligned quad must stay on world-XZ (Y-up); got {corner:?}"
            );
        }
        let spans_x = corners.iter().any(|c| (c[0] - 10.0).abs() > 0.1);
        let spans_z = corners.iter().any(|c| (c[2] - 20.0).abs() > 0.1);
        assert!(
            spans_x && spans_z,
            "quad must extend in X and Z, not stand up: {corners:?}"
        );

        let shader = include_str!("shaders/particle_vertex.wgsl");
        assert!(
            shader.contains("up = vec3<f32>(0.0, 0.0, 1.0)"),
            "live vertex shader must use world-Z as the ground-quad V axis (Y-up host)"
        );
        assert!(
            !shader.contains("up = vec3<f32>(0.0, 1.0, 0.0)"),
            "world-Y as V stands particles up on a Y-up host"
        );
    }

    #[test]
    fn volume_particle_emits_default_depth_layers() {
        let system = test_system(ParticleType::VolumeParticle, 0);
        let mut info = ParticleInfo::default();
        info.position = Point3::new(0.0, 0.0, 3.0);
        info.size = 6.0;
        let particle = Particle::new(&info, 0, 0);
        let mut batch = ParticleBatch::new(ParticleShaderType::Alpha, "test.tga".to_string());

        batch.add_volume_particle(&particle, &system, [0.0, 0.0, 0.0]);

        assert_eq!(batch.vertices.len(), OPTIMUM_VOLUME_PARTICLE_DEPTH as usize);
        let alpha_sum: f32 = batch.vertices.iter().map(|vertex| vertex.alpha).sum();
        assert!((alpha_sum - particle.alpha).abs() < 0.0001);
        assert!(batch.vertices.first().unwrap().position[2] < particle.position.z);
        assert!(batch.vertices.last().unwrap().position[2] > particle.position.z);
    }

    #[test]
    fn particle_stats_frame_reset_preserves_gpu_memory() {
        let mut stats = ParticleRenderStats {
            particles_rendered: 17,
            batches_rendered: 3,
            draw_calls: 3,
            gpu_memory_used: 4096,
            render_time_ms: 2.5,
        };

        stats.reset_frame_counters();

        assert_eq!(stats.particles_rendered, 0);
        assert_eq!(stats.batches_rendered, 0);
        assert_eq!(stats.draw_calls, 0);
        assert_eq!(stats.gpu_memory_used, 4096);
        assert_eq!(stats.render_time_ms, 0.0);
    }

    fn smudge_named_system(type_name: &str, particle_type: ParticleType) -> ParticleSystem {
        let mut template = ParticleSystemTemplate::new("heat".to_string());
        template.info_mut().particle_type = particle_type;
        template.info_mut().particle_type_name = type_name.to_string();
        ParticleSystem::new(Arc::new(template), 1, false)
    }

    fn live_particle(x: f32, y: f32, z: f32, size: f32, alpha: f32) -> Particle {
        let mut info = ParticleInfo::default();
        info.position = Point3::new(x, y, z);
        info.size = size;
        let mut particle = Particle::new(&info, 0, 0);
        particle.alpha = alpha;
        particle
    }

    #[test]
    fn particle_type_name_smud_matches_cpp_dword_prefix() {
        assert!(particle_type_name_is_smud("SMUD"));
        assert!(particle_type_name_is_smud("SMUDGE RESERVED"));
        assert!(particle_type_name_is_smud("SMUDjetExhaust.tga"));
        assert!(!particle_type_name_is_smud("smudge.tga"));
        assert!(!particle_type_name_is_smud("SMU"));
        assert!(!particle_type_name_is_smud("EXSmokePuff.tga"));
        assert!(!particle_type_name_is_smud(""));
    }

    #[test]
    fn smud_prefix_system_feeds_smudge_manager_not_sprites() {
        let _guard = HEAT_SMUDGE_TEST_LOCK.lock();
        begin_particle_heat_smudge_frame();

        let mut system = smudge_named_system("SMUDGE RESERVED", ParticleType::Particle);
        system.push_particle(live_particle(10.0, 20.0, 30.0, 8.0, 0.4));
        let mut dead = live_particle(1.0, 1.0, 1.0, 4.0, 1.0);
        dead.lifetime_left = 0;
        system.push_particle(dead);
        let mut culled = live_particle(2.0, 2.0, 2.0, 4.0, 1.0);
        culled.is_culled = true;
        system.push_particle(culled);

        assert!(bake_particle_system_gpu_mesh(&system).is_empty());
        assert_eq!(feed_system_heat_smudges(&system), 1);

        let items = get_smudge_manager()
            .lock()
            .unwrap()
            .collect_decal_render_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].position, Point3::new(10.0, 20.0, 30.0));
        assert_eq!(items[0].size, 8.0);
        assert!((items[0].color[3] - 0.4).abs() < f32::EPSILON);

        let smudges = get_smudge_manager().lock().unwrap().collect_used_smudges();
        assert_eq!(smudges.len(), 1);
        assert!(smudges[0].offset.x >= -0.06 && smudges[0].offset.x <= 0.06);
        assert!(smudges[0].offset.y >= -0.03 && smudges[0].offset.y <= 0.03);

        begin_particle_heat_smudge_frame();
        assert!(
            get_smudge_manager()
                .lock()
                .unwrap()
                .collect_decal_render_items()
                .is_empty()
        );
    }

    #[test]
    fn heat_effects_off_skips_smudge_insert_and_sprites() {
        let _guard = HEAT_SMUDGE_TEST_LOCK.lock();
        let previous = game_engine::common::global_data::read().use_heat_effects;
        game_engine::common::global_data::write().use_heat_effects = false;
        begin_particle_heat_smudge_frame();

        let mut system = smudge_named_system("SMUDjetExhaust.tga", ParticleType::Smudge);
        system.push_particle(live_particle(0.0, 0.0, 5.0, 3.0, 0.8));

        let feed_count = feed_system_heat_smudges(&system);
        let empty_mesh = bake_particle_system_gpu_mesh(&system).is_empty();
        let empty_items = get_smudge_manager()
            .lock()
            .unwrap()
            .collect_decal_render_items()
            .is_empty();

        game_engine::common::global_data::write().use_heat_effects = previous;
        begin_particle_heat_smudge_frame();

        assert!(empty_mesh);
        assert_eq!(feed_count, 0);
        assert!(empty_items);
    }

    #[test]
    fn smudge_type_feeds_even_without_smud_prefix() {
        let _guard = HEAT_SMUDGE_TEST_LOCK.lock();
        begin_particle_heat_smudge_frame();

        let mut system = smudge_named_system("HeatHaze.tga", ParticleType::Smudge);
        system.push_particle(live_particle(4.0, 5.0, 6.0, 2.5, 0.55));

        assert!(system.is_using_smudge());
        assert!(system_is_heat_smudge(&system));
        assert!(bake_particle_system_gpu_mesh(&system).is_empty());
        assert_eq!(feed_system_heat_smudges(&system), 1);

        let items = get_smudge_manager()
            .lock()
            .unwrap()
            .collect_decal_render_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].position, Point3::new(4.0, 5.0, 6.0));
        assert_eq!(items[0].size, 2.5);
        assert!((items[0].color[3] - 0.55).abs() < f32::EPSILON);

        begin_particle_heat_smudge_frame();
    }

    #[test]
    fn ordinary_particle_does_not_feed_heat_smudges() {
        let _guard = HEAT_SMUDGE_TEST_LOCK.lock();
        begin_particle_heat_smudge_frame();

        let mut system = smudge_named_system("EXSmokePuff.tga", ParticleType::Particle);
        system.push_particle(live_particle(1.0, 2.0, 3.0, 1.0, 1.0));

        assert!(!system.is_using_smudge());
        assert!(!system_is_heat_smudge(&system));
        assert_eq!(feed_system_heat_smudges(&system), 0);
        assert_eq!(bake_particle_system_gpu_mesh(&system).len(), 1);
        assert!(
            get_smudge_manager()
                .lock()
                .unwrap()
                .collect_decal_render_items()
                .is_empty()
        );

        begin_particle_heat_smudge_frame();
    }

    #[test]
    fn multiply_blend_is_dest_times_src_not_subtract_to_black() {
        let blend = particle_multiply_color_blend();
        assert_eq!(blend.src_factor, wgpu::BlendFactor::Zero);
        assert_eq!(blend.dst_factor, wgpu::BlendFactor::Src);
        assert_eq!(blend.operation, wgpu::BlendOperation::Add);
        let dest = 0.8_f32;
        let src = 0.5_f32;
        let src_term = src * blend_factor_value(src, dest, blend.src_factor);
        let dest_term = dest * blend_factor_value(src, dest, blend.dst_factor);
        let out = src_term + dest_term;
        assert!((out - dest * src).abs() < 1.0e-5);
        assert!((out - 0.4).abs() < 1.0e-5);
    }

    fn blend_factor_value(src: f32, dest: f32, factor: wgpu::BlendFactor) -> f32 {
        match factor {
            wgpu::BlendFactor::Zero => 0.0,
            wgpu::BlendFactor::One => 1.0,
            wgpu::BlendFactor::Src => src,
            wgpu::BlendFactor::Dst => dest,
            _ => panic!("unexpected factor"),
        }
    }
}
