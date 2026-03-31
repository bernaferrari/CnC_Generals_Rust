//! Frame graph submission node for orchestrating per-pass queues.
//!
//! This module promotes the temporary frame queues that previously lived inside the renderer
//! into a persistent data structure that tracks per-pass submission data, queue topology, and
//! per-frame context (camera matrices, jitter, and pass classification). The goal is to make
//! render submissions reusable across multiple passes (main, shadows, reflections) and to mirror
//! the behaviour of the original WW3D renderer where composite sort keys governed transparent
//! ordering.

use crate::render_object_system::{RenderInfoClass, RenderObjClass};
use crate::rendering::mesh_system::{MeshClass, StaticSortManager};
use crate::rendering::shader_system::shader::{DepthMaskType, MaterialBlendMode, ShaderClass};
use bitflags::bitflags;
use glam::{Mat4, Vec2, Vec3};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;
use ww3d_core::WW3D;

/// Identifier for a frame-graph pass node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameGraphPass {
    Main,
    Shadow(u32),
    Reflection,
    Custom(u32),
}

/// Queue classifications owned by a frame-graph node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameGraphQueue {
    Opaque,
    Alpha,
    Additive,
    Decal,
    ShadowCaster,
}

bitflags! {
    /// Submission mask describing the passes a mesh participates in.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct FrameGraphPassMask: u8 {
        const OPAQUE        = 0b0000_0001;
        const ALPHA         = 0b0000_0010;
        const ADDITIVE      = 0b0000_0100;
        const DECAL         = 0b0000_1000;
        const SHADOW_CASTER = 0b0001_0000;
    }
}

/// Hint used by downstream passes to fast-path pipeline lookups.
#[derive(Debug, Clone, Copy, Default)]
pub struct PipelineHint {
    pub shader_signature: u32,
    pub pass_count: u8,
    pub is_skinned: bool,
}

/// Context captured for a given pass node so subsequent passes (shadows/reflections) can reuse
/// the same submission list with different view-projection data if required.
#[derive(Debug, Clone)]
pub struct FrameGraphPassContext {
    pub pass: FrameGraphPass,
    pub view: Mat4,
    pub projection: Mat4,
    pub view_projection: Mat4,
    pub jitter: Vec2,
}

impl FrameGraphPassContext {
    pub fn new(pass: FrameGraphPass, view: Mat4, projection: Mat4, jitter: Vec2) -> Self {
        let view_projection = projection * view;
        Self {
            pass,
            view,
            projection,
            view_projection,
            jitter,
        }
    }

    pub fn from_render_info(pass: FrameGraphPass, info: &RenderInfoClass) -> Self {
        let camera = &info.camera;
        let view = camera.get_cached_view_matrix();
        let projection = camera.get_cached_projection_matrix();
        let jitter = Vec2::new(0.0, 0.0);
        Self::new(pass, view, projection, jitter)
    }
}

/// One prepared frame worth of queue data.
#[derive(Debug, Default)]
pub struct FrameGraphPreparedQueues {
    pub opaque: Vec<Arc<MeshClass>>,
    pub alpha: Vec<Arc<MeshClass>>,
    pub additive: Vec<Arc<MeshClass>>,
    pub decals: Vec<Arc<MeshClass>>,
    pub shadow_casters: Vec<Arc<MeshClass>>,
}

impl FrameGraphPreparedQueues {
    pub fn combined_translucent(&self) -> Vec<Arc<MeshClass>> {
        let mut combined = Vec::with_capacity(self.alpha.len() + self.additive.len());
        combined.extend(self.alpha.iter().cloned());
        combined.extend(self.additive.iter().cloned());
        combined
    }
}

#[derive(Debug, Clone)]
struct SubmissionMaterialTraits {
    dominant_blend: MaterialBlendMode,
    stage_hint: u16,
    z_write: bool,
    pass_mask: FrameGraphPassMask,
    shader_signature: u32,
    pass_count: u8,
    is_skinned: bool,
}

impl SubmissionMaterialTraits {
    fn analyse(mesh: &MeshClass) -> Self {
        let mut dominant_blend = MaterialBlendMode::Opaque;
        let mut stage_hint: u16 = 0;
        let mut z_write = false;
        let mut pass_mask = FrameGraphPassMask::empty();
        let mut shader_signature: u32 = 0;
        let mut pass_count: u8 = 0;
        let mut is_skinned = false;

        if let Some(model) = &mesh.model {
            is_skinned = model.is_skinned();

            for (index, pass) in model.material_passes.iter().enumerate() {
                let shader: &ShaderClass = &pass.shader;
                z_write |= shader.get_depth_mask() == DepthMaskType::Enable;

                let blend = shader.blend_mode();
                dominant_blend = select_dominant_blend(dominant_blend, blend);
                pass_mask |= pass_mask_for_blend(blend);

                stage_hint = stage_hint.max(index as u16);
                shader_signature = shader_signature.wrapping_add(shader.id());
                pass_count = pass_count.saturating_add(1);
            }
        }

        SubmissionMaterialTraits {
            dominant_blend,
            stage_hint,
            z_write,
            pass_mask,
            shader_signature,
            pass_count,
            is_skinned,
        }
    }
}

fn select_dominant_blend(
    current: MaterialBlendMode,
    candidate: MaterialBlendMode,
) -> MaterialBlendMode {
    if blend_rank(candidate) > blend_rank(current) {
        candidate
    } else {
        current
    }
}

fn pass_mask_for_blend(blend: MaterialBlendMode) -> FrameGraphPassMask {
    match blend {
        MaterialBlendMode::Opaque => FrameGraphPassMask::OPAQUE,
        MaterialBlendMode::Alpha => FrameGraphPassMask::ALPHA,
        MaterialBlendMode::Additive => FrameGraphPassMask::ADDITIVE,
        MaterialBlendMode::Decal => FrameGraphPassMask::DECAL,
        MaterialBlendMode::Multiply => FrameGraphPassMask::OPAQUE, // Darken blend
        MaterialBlendMode::Screen => FrameGraphPassMask::ADDITIVE, // Lighten blend
    }
}

fn blend_rank(blend: MaterialBlendMode) -> u8 {
    match blend {
        MaterialBlendMode::Opaque => 0,
        MaterialBlendMode::Decal => 1,
        MaterialBlendMode::Multiply => 1, // Darken blend (same priority as decal)
        MaterialBlendMode::Alpha => 2,
        MaterialBlendMode::Screen => 3, // Lighten blend (same priority as additive)
        MaterialBlendMode::Additive => 3,
    }
}

#[derive(Debug, Clone)]
struct FrameGraphSubmission {
    mesh: Arc<MeshClass>,
    sort_level: u32,
    #[allow(dead_code)] // C++ parity
    pass_mask: FrameGraphPassMask,
    #[allow(dead_code)] // C++ parity
    pipeline_hint: PipelineHint,
    sort_key: u64,
    blend_mode: MaterialBlendMode,
    stage_hint: u16,
    z_write: bool,
    tie_breaker: usize,
    bounds_center: Vec3,
    #[allow(dead_code)] // C++ parity
    bounds_radius: f32,
}

impl FrameGraphSubmission {
    fn new(mesh: Arc<MeshClass>, traits: SubmissionMaterialTraits, sort_level: u32) -> Self {
        let tie_breaker = Arc::as_ptr(&mesh) as usize;
        let sphere = mesh.get_bounding_sphere();
        let sort_key = compose_sort_key(
            traits.z_write,
            traits.dominant_blend,
            traits.stage_hint,
            sort_level,
            tie_breaker,
        );

        FrameGraphSubmission {
            mesh,
            sort_level,
            pass_mask: traits.pass_mask,
            pipeline_hint: PipelineHint {
                shader_signature: traits.shader_signature,
                pass_count: traits.pass_count,
                is_skinned: traits.is_skinned,
            },
            sort_key,
            blend_mode: traits.dominant_blend,
            stage_hint: traits.stage_hint,
            z_write: traits.z_write,
            tie_breaker,
            bounds_center: sphere.center,
            bounds_radius: sphere.radius,
        }
    }

    fn mesh(&self) -> Arc<MeshClass> {
        Arc::clone(&self.mesh)
    }

    fn distance_sq(&self, camera_pos: Vec3) -> f32 {
        (self.bounds_center - camera_pos).length_squared()
    }
}

fn compose_sort_key(
    z_write: bool,
    blend: MaterialBlendMode,
    stage_hint: u16,
    sort_level: u32,
    tie_breaker: usize,
) -> u64 {
    let z_bucket = if z_write { 0u64 } else { 1u64 };
    let blend_bucket = blend_rank(blend) as u64;
    let stage_bucket = stage_hint as u64;

    (z_bucket << 63)
        | ((blend_bucket & 0x7) << 60)
        | ((stage_bucket & 0x0FFF) << 48)
        | ((sort_level as u64) << 16)
        | ((tie_breaker as u64) & 0xFFFF)
}

fn queue_from_blend(blend: MaterialBlendMode) -> FrameGraphQueue {
    match blend {
        MaterialBlendMode::Opaque => FrameGraphQueue::Opaque,
        MaterialBlendMode::Alpha => FrameGraphQueue::Alpha,
        MaterialBlendMode::Additive => FrameGraphQueue::Additive,
        MaterialBlendMode::Decal => FrameGraphQueue::Decal,
        MaterialBlendMode::Multiply => FrameGraphQueue::Opaque, // Darken blend
        MaterialBlendMode::Screen => FrameGraphQueue::Additive, // Lighten blend
    }
}

fn resolve_queue(hint: FrameGraphQueue, blend: MaterialBlendMode) -> FrameGraphQueue {
    match hint {
        FrameGraphQueue::Decal | FrameGraphQueue::ShadowCaster => hint,
        _ => {
            let blend_queue = queue_from_blend(blend);
            if matches!(blend_queue, FrameGraphQueue::Opaque) {
                hint
            } else {
                blend_queue
            }
        }
    }
}

#[derive(Default)]
struct NodeQueues {
    opaque: Vec<usize>,
    alpha: Vec<usize>,
    additive: Vec<usize>,
    decals: Vec<usize>,
    shadow: Vec<usize>,
}

impl NodeQueues {
    fn clear(&mut self) {
        self.opaque.clear();
        self.alpha.clear();
        self.additive.clear();
        self.decals.clear();
        self.shadow.clear();
    }
}

/// Frame-graph node that owns submissions for a single pass.
pub struct FrameGraphNode {
    #[allow(dead_code)] // C++ parity
    pass: FrameGraphPass,
    context: Option<FrameGraphPassContext>,
    submissions: Vec<FrameGraphSubmission>,
    queues: NodeQueues,
}

impl FrameGraphNode {
    pub fn new(pass: FrameGraphPass) -> Self {
        Self {
            pass,
            context: None,
            submissions: Vec::new(),
            queues: NodeQueues::default(),
        }
    }

    pub fn reset(&mut self) {
        self.submissions.clear();
        self.queues.clear();
    }

    pub fn set_context(&mut self, context: FrameGraphPassContext) {
        self.context = Some(context);
    }

    pub fn context(&self) -> Option<&FrameGraphPassContext> {
        self.context.as_ref()
    }

    pub fn submit_mesh(
        &mut self,
        mesh: Arc<MeshClass>,
        queue_hint: Option<FrameGraphQueue>,
        sort_level_override: Option<u32>,
    ) {
        let traits = SubmissionMaterialTraits::analyse(mesh.as_ref());
        self.submit_mesh_with_traits(mesh, traits, queue_hint, sort_level_override);
    }

    fn submit_mesh_with_traits(
        &mut self,
        mesh: Arc<MeshClass>,
        traits: SubmissionMaterialTraits,
        queue_hint: Option<FrameGraphQueue>,
        sort_level_override: Option<u32>,
    ) {
        let is_decal = mesh.is_decal_instance;
        let resolved_queue = match queue_hint {
            Some(hint) => resolve_queue(hint, traits.dominant_blend),
            None => queue_from_blend(traits.dominant_blend),
        };
        let sort_level = sort_level_override.unwrap_or(mesh.sort_level);
        let submission = FrameGraphSubmission::new(Arc::clone(&mesh), traits, sort_level);
        let index = self.submissions.len();
        self.submissions.push(submission);
        self.enqueue(index, resolved_queue);

        if !is_decal && WW3D::are_decals_enabled() && !mesh.decal_meshes.is_empty() {
            for decal in &mesh.decal_meshes {
                if decal.model.is_none() {
                    continue;
                }
                let decal_traits = SubmissionMaterialTraits::analyse(decal.as_ref());
                self.submit_mesh_with_traits(
                    Arc::clone(decal),
                    decal_traits,
                    Some(FrameGraphQueue::Decal),
                    Some(sort_level),
                );
            }
        }
    }

    fn enqueue(&mut self, submission_index: usize, queue: FrameGraphQueue) {
        match queue {
            FrameGraphQueue::Opaque => self.queues.opaque.push(submission_index),
            FrameGraphQueue::Alpha => self.queues.alpha.push(submission_index),
            FrameGraphQueue::Additive => self.queues.additive.push(submission_index),
            FrameGraphQueue::Decal => self.queues.decals.push(submission_index),
            FrameGraphQueue::ShadowCaster => self.queues.shadow.push(submission_index),
        }
    }

    pub fn prepare(&mut self, camera_pos: Vec3) -> FrameGraphPreparedQueues {
        let mut prepared = FrameGraphPreparedQueues::default();
        let submissions = &self.submissions;

        self.queues
            .opaque
            .sort_by(|a, b| compare_opaque(submissions, *a, *b));
        prepared.opaque = collect_meshes(&self.queues.opaque, submissions);

        self.queues
            .alpha
            .sort_by(|a, b| compare_transparent(submissions, *a, *b, camera_pos));
        prepared.alpha = collect_meshes(&self.queues.alpha, submissions);

        self.queues
            .additive
            .sort_by(|a, b| compare_transparent(submissions, *a, *b, camera_pos));
        prepared.additive = collect_meshes(&self.queues.additive, submissions);

        self.queues
            .decals
            .sort_by(|a, b| compare_decals(submissions, *a, *b));
        prepared.decals = collect_meshes(&self.queues.decals, submissions);

        self.queues
            .shadow
            .sort_by(|a, b| compare_shadow(submissions, *a, *b));
        prepared.shadow_casters = collect_meshes(&self.queues.shadow, submissions);

        prepared
    }

    pub fn ingest_static_sort_entries(&mut self) -> Vec<Arc<dyn RenderObjClass>> {
        let mut fallback: Vec<Arc<dyn RenderObjClass>> = Vec::new();

        if let Some((entries, sort_levels)) = StaticSortManager::snapshot_static_sort_list() {
            for (entry, sort_level) in entries.into_iter().zip(sort_levels.into_iter()) {
                if let Some(mesh_arc) = entry.mesh_arc() {
                    let traits = SubmissionMaterialTraits::analyse(mesh_arc.as_ref());
                    self.submit_mesh_with_traits(mesh_arc, traits, None, Some(sort_level));
                } else {
                    fallback.push(entry.render_object());
                }
            }
        }

        fallback
    }
}

fn collect_meshes(indices: &[usize], submissions: &[FrameGraphSubmission]) -> Vec<Arc<MeshClass>> {
    indices
        .iter()
        .map(|&index| submissions[index].mesh())
        .collect()
}

fn compare_opaque(submissions: &[FrameGraphSubmission], a: usize, b: usize) -> Ordering {
    let left = &submissions[a];
    let right = &submissions[b];
    left.sort_key
        .cmp(&right.sort_key)
        .then_with(|| left.tie_breaker.cmp(&right.tie_breaker))
}

fn compare_decals(submissions: &[FrameGraphSubmission], a: usize, b: usize) -> Ordering {
    let left = &submissions[a];
    let right = &submissions[b];
    left.sort_level
        .cmp(&right.sort_level)
        .then_with(|| left.tie_breaker.cmp(&right.tie_breaker))
}

fn compare_shadow(submissions: &[FrameGraphSubmission], a: usize, b: usize) -> Ordering {
    let left = &submissions[a];
    let right = &submissions[b];
    left.sort_level
        .cmp(&right.sort_level)
        .then_with(|| left.tie_breaker.cmp(&right.tie_breaker))
}

fn compare_transparent(
    submissions: &[FrameGraphSubmission],
    a: usize,
    b: usize,
    camera_pos: Vec3,
) -> Ordering {
    let left = &submissions[a];
    let right = &submissions[b];

    let z_cmp = right.z_write.cmp(&left.z_write);
    let blend_cmp = blend_rank(left.blend_mode).cmp(&blend_rank(right.blend_mode));
    let stage_cmp = left.stage_hint.cmp(&right.stage_hint);
    let sort_level_cmp = right.sort_level.cmp(&left.sort_level);

    let dist_a = left.distance_sq(camera_pos);
    let dist_b = right.distance_sq(camera_pos);
    let distance_cmp = dist_b.partial_cmp(&dist_a).unwrap_or(Ordering::Equal);

    z_cmp
        .then(blend_cmp)
        .then(stage_cmp)
        .then(sort_level_cmp)
        .then(distance_cmp)
        .then_with(|| left.tie_breaker.cmp(&right.tie_breaker))
}

/// Simple frame graph container for now – holds one node per pass.
#[derive(Default)]
pub struct FrameGraph {
    nodes: HashMap<FrameGraphPass, FrameGraphNode>,
}

impl FrameGraph {
    pub fn new() -> Self {
        let mut graph = FrameGraph {
            nodes: HashMap::new(),
        };
        graph.ensure_node(FrameGraphPass::Main);
        graph
    }

    pub fn begin_frame(&mut self) {
        for node in self.nodes.values_mut() {
            node.reset();
        }
    }

    pub fn ensure_node(&mut self, pass: FrameGraphPass) {
        self.nodes
            .entry(pass)
            .or_insert_with(|| FrameGraphNode::new(pass));
    }

    pub fn node_mut(&mut self, pass: FrameGraphPass) -> &mut FrameGraphNode {
        self.ensure_node(pass);
        self.nodes.get_mut(&pass).expect("frame graph node present")
    }

    pub fn node(&self, pass: FrameGraphPass) -> Option<&FrameGraphNode> {
        self.nodes.get(&pass)
    }
}
