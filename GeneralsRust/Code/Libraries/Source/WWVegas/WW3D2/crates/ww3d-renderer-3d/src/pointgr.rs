//! Point graphics system ported from the legacy WW3D PointGroupClass.
//!
//! The original DX8 implementation constructed billboarded triangles/quads per point with
//! orientation, per-point color overrides, active-point tables, and optional frames.  This
//! Rust port recreates that behaviour and feeds the generated geometry into the modern
//! renderer by synthesising transient mesh instances each frame.

use crate::core::error::{Error, RendererResult};
use crate::material_system::{MaterialPassClass, VertexMaterialClass};
use crate::render_object_system::AABoxClass;
use crate::render_object_system::RenderInfoClass;
use crate::rendering::mesh_system::{MeshClass, MeshModelClass};
use crate::rendering::shader_system::ShaderClass;
use crate::Renderer;
use glam::{Mat2, Vec2, Vec3, Vec4};
use lazy_static::lazy_static;
use std::sync::Arc;
use ww3d_collision::SphereClass;
use ww3d_core::w3d_format::{W3dTexCoordStruct, W3dTriangleStruct, W3dVectorStruct};
use ww3d_core::wwstring::StringClass;

/// Point rendering mode – mirrored from the original PointGroupClass::PointModeEnum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointMode {
    Tris,
    Quads,
    ScreenSpace,
}

impl Default for PointMode {
    fn default() -> Self {
        PointMode::Quads
    }
}

/// Primary point group implementation.
#[derive(Debug, Clone)]
pub struct PointGroupClass {
    point_mode: PointMode,
    default_point_size: f32,
    default_point_color: Vec3,
    default_point_alpha: f32,
    default_point_orientation: u8,
    default_point_frame: u8,
    positions: Vec<Vec3>,
    diffuse_overrides: Vec<Vec4>,
    size_overrides: Vec<f32>,
    orientation_overrides: Vec<u8>,
    frame_overrides: Vec<u8>,
    active_points: Vec<bool>,
    mesh_dirty: bool,
    cached_mesh: Option<Arc<MeshClass>>,
    name: StringClass,
}

impl PointGroupClass {
    pub fn new() -> Self {
        Self {
            point_mode: PointMode::Quads,
            default_point_size: 1.0,
            default_point_color: Vec3::ONE,
            default_point_alpha: 1.0,
            default_point_orientation: 0,
            default_point_frame: 0,
            positions: Vec::new(),
            diffuse_overrides: Vec::new(),
            size_overrides: Vec::new(),
            orientation_overrides: Vec::new(),
            frame_overrides: Vec::new(),
            active_points: Vec::new(),
            mesh_dirty: true,
            cached_mesh: None,
            name: StringClass::from("PointGroup"),
        }
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = StringClass::from(name);
        self.mesh_dirty = true;
    }

    pub fn set_point_mode(&mut self, mode: PointMode) {
        if self.point_mode != mode {
            self.point_mode = mode;
            self.mesh_dirty = true;
        }
    }

    pub fn set_point_size(&mut self, size: f32) {
        if (self.default_point_size - size).abs() > f32::EPSILON {
            self.default_point_size = size.max(0.0);
            self.mesh_dirty = true;
        }
    }

    pub fn set_point_color(&mut self, color: Vec3) {
        if self.default_point_color != color {
            self.default_point_color = color;
            self.mesh_dirty = true;
        }
    }

    pub fn set_point_alpha(&mut self, alpha: f32) {
        if (self.default_point_alpha - alpha).abs() > f32::EPSILON {
            self.default_point_alpha = alpha.clamp(0.0, 1.0);
            self.mesh_dirty = true;
        }
    }

    pub fn set_point_orientation(&mut self, orientation: u8) {
        if self.default_point_orientation != orientation {
            self.default_point_orientation = orientation;
            self.mesh_dirty = true;
        }
    }

    pub fn set_point_frame(&mut self, frame: u8) {
        if self.default_point_frame != frame {
            self.default_point_frame = frame;
            self.mesh_dirty = true;
        }
    }

    /// Set core arrays; any slice may be empty to indicate "use defaults".
    pub fn set_arrays(
        &mut self,
        positions: Vec<Vec3>,
        diffuse: Option<Vec<Vec4>>,
        active: Option<Vec<bool>>,
        sizes: Option<Vec<f32>>,
        orientations: Option<Vec<u8>>,
        frames: Option<Vec<u8>>,
    ) {
        self.positions = positions;
        self.diffuse_overrides = diffuse.unwrap_or_default();
        self.active_points = active.unwrap_or_default();
        self.size_overrides = sizes.unwrap_or_default();
        self.orientation_overrides = orientations.unwrap_or_default();
        self.frame_overrides = frames.unwrap_or_default();
        self.mesh_dirty = true;
    }

    pub fn clear(&mut self) {
        self.positions.clear();
        self.diffuse_overrides.clear();
        self.size_overrides.clear();
        self.orientation_overrides.clear();
        self.frame_overrides.clear();
        self.active_points.clear();
        self.cached_mesh = None;
        self.mesh_dirty = true;
    }

    fn ensure_mesh(&mut self) -> Option<Arc<MeshClass>> {
        if !self.mesh_dirty {
            return self.cached_mesh.clone();
        }

        let mesh = self.build_render_mesh()?;
        self.cached_mesh = Some(mesh.clone());
        self.mesh_dirty = false;
        Some(mesh)
    }

    fn build_render_mesh(&self) -> Option<Arc<MeshClass>> {
        if self.positions.is_empty() {
            return None;
        }

        // Pre-allocate vectors based on expected usage
        // Most common case is Quads mode: 4 vertices per point, 6 indices per point (2 triangles)
        let point_count = self.positions.len();
        let mut vertices = Vec::with_capacity(point_count * 4);
        let mut indices = Vec::with_capacity(point_count * 6);
        let mut texcoords = Vec::with_capacity(point_count * 4);
        let mut vertex_colors = Vec::with_capacity(point_count * 4);

        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);

        let orientation_table = orientation_table();

        for i in 0..self.positions.len() {
            if !self.active_points.is_empty() && !self.active_points[i] {
                continue;
            }

            let position = self.positions[i];
            let diffuse = self.diffuse_overrides.get(i).copied().unwrap_or_else(|| {
                Vec4::new(
                    self.default_point_color.x,
                    self.default_point_color.y,
                    self.default_point_color.z,
                    self.default_point_alpha,
                )
            });
            let size = self
                .size_overrides
                .get(i)
                .copied()
                .unwrap_or(self.default_point_size);
            let orientation_index = self
                .orientation_overrides
                .get(i)
                .copied()
                .unwrap_or(self.default_point_orientation);
            let frame = self
                .frame_overrides
                .get(i)
                .copied()
                .unwrap_or(self.default_point_frame);

            let orientation = orientation_table[orientation_index as usize];
            let base_tri = TRIANGLE_BASE;
            let base_quad = QUAD_BASE;

            match self.point_mode {
                PointMode::Tris => {
                    let verts = base_tri
                        .iter()
                        .map(|offset| rotate_offset(*offset, orientation, size))
                        .collect::<Vec<_>>();
                    let base_index = vertices.len() as u32;
                    for v in &verts {
                        let wp = position + *v;
                        min = min.min(wp);
                        max = max.max(wp);
                        vertices.push(W3dVectorStruct::from(wp));
                        texcoords.push(frame_to_uv(frame, Vec2::ZERO));
                        vertex_colors.push(diffuse);
                    }
                    indices.extend_from_slice(&[base_index, base_index + 1, base_index + 2]);
                }
                PointMode::Quads | PointMode::ScreenSpace => {
                    let verts = base_quad
                        .iter()
                        .map(|offset| rotate_offset(*offset, orientation, size))
                        .collect::<Vec<_>>();
                    let base_index = vertices.len() as u32;
                    let uvs = [
                        Vec2::new(0.0, 1.0),
                        Vec2::new(1.0, 1.0),
                        Vec2::new(1.0, 0.0),
                        Vec2::new(0.0, 0.0),
                    ];
                    for (v, uv) in verts.iter().zip(uvs.iter()) {
                        let wp = position + *v;
                        min = min.min(wp);
                        max = max.max(wp);
                        vertices.push(W3dVectorStruct::from(wp));
                        texcoords.push(frame_to_uv(frame, *uv));
                        vertex_colors.push(diffuse);
                    }
                    indices.extend_from_slice(&[
                        base_index,
                        base_index + 1,
                        base_index + 2,
                        base_index,
                        base_index + 2,
                        base_index + 3,
                    ]);
                }
            }
        }

        if vertices.is_empty() {
            return None;
        }

        let mut model = MeshModelClass::new("PointGroupMesh");
        model.vertices = vertices;
        model.vertex_count = model.vertices.len() as u32;
        model.index_count = indices.len() as u32;
        model.triangles = indices
            .chunks(3)
            .filter_map(|chunk| {
                if chunk.len() == 3 {
                    Some(W3dTriangleStruct {
                        vindex: [chunk[0], chunk[1], chunk[2]],
                        attributes: 0,
                        normal: W3dVectorStruct::from(Vec3::Z),
                        distance: 0.0,
                    })
                } else {
                    None
                }
            })
            .collect();
        model.texture_coords = texcoords;

        let average_alpha =
            vertex_colors.iter().map(|c| c.w).sum::<f32>().max(0.0) / vertex_colors.len() as f32;

        let mut pass = MaterialPassClass::new();
        let shader = if average_alpha < 1.0 {
            ShaderClass::get_alpha_shader()
        } else {
            ShaderClass::get_opaque_shader()
        };
        pass.shader = shader;
        pass.diffuse_vertex_colors = Some(vertex_colors);

        let mut vertex_material = VertexMaterialClass::new("PointGroupMaterial");
        vertex_material.diffuse = self.default_point_color;
        vertex_material.opacity = average_alpha;
        vertex_material.ambient = self.default_point_color * 0.25;
        vertex_material.emissive = self.default_point_color * 0.1;
        pass.vertex_material = Some(Arc::new(vertex_material));
        model.material_passes = vec![pass];
        model.register_for_rendering();

        let mut mesh = MeshClass::new();
        mesh.name = self.name.to_string();
        mesh.model = Some(Arc::new(model));
        mesh.alpha_override = average_alpha;
        mesh.sort_level = 0;

        let center = (min + max) * 0.5;
        let radius = (max - min).length() * 0.5;
        let bbox = AABoxClass::from_min_max(min, max);
        mesh.bounding_box = bbox;
        mesh.bounding_sphere = SphereClass::new(center, radius);
        mesh.set_transform(glam::Mat4::IDENTITY);
        mesh.update_cached_bounding_volumes();

        Some(Arc::new(mesh))
    }

    pub fn render(&mut self, _rinfo: &RenderInfoClass) -> RendererResult<()> {
        let Some(mesh) = self.ensure_mesh() else {
            return Ok(());
        };

        Renderer::with_global_mut(|renderer| {
            renderer.queue_mesh(mesh.clone())?;
            Ok::<(), Error>(())
        })?;
        Ok(())
    }
}

impl Default for PointGroupClass {
    fn default() -> Self {
        Self::new()
    }
}

/// Standalone orientation lookup table (256 discrete orientations).
fn orientation_table() -> &'static [Mat2; 256] {
    lazy_static! {
        static ref TABLE: [Mat2; 256] = {
            let mut data = [Mat2::IDENTITY; 256];
            for (i, entry) in data.iter_mut().enumerate() {
                let angle = (i as f32 / 256.0) * std::f32::consts::TAU;
                let (s, c) = angle.sin_cos();
                *entry = Mat2::from_cols_array(&[c, s, -s, c]);
            }
            data
        };
    }
    &TABLE
}

const TRIANGLE_BASE: [Vec2; 3] = [
    Vec2::new(0.0, 0.5),
    Vec2::new(-0.4330127, -0.25),
    Vec2::new(0.4330127, -0.25),
];

const QUAD_BASE: [Vec2; 4] = [
    Vec2::new(-0.5, -0.5),
    Vec2::new(0.5, -0.5),
    Vec2::new(0.5, 0.5),
    Vec2::new(-0.5, 0.5),
];

fn rotate_offset(offset: Vec2, rotation: Mat2, scale: f32) -> Vec3 {
    let rotated = rotation * offset;
    Vec3::new(rotated.x * scale, rotated.y * scale, 0.0)
}

fn frame_to_uv(_frame: u8, uv: Vec2) -> W3dTexCoordStruct {
    // Legacy implementation supported texture atlases – for now treat frames as direct UVs.
    W3dTexCoordStruct { u: uv.x, v: uv.y }
}

/// Simple sphere object retained for compatibility with existing effect code paths.
#[derive(Debug, Clone)]
pub struct SphereObjClass {
    pub center: Vec3,
    pub radius: f32,
    pub color: Vec4,
}

impl SphereObjClass {
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self {
            center,
            radius,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
        }
    }

    pub fn set_color(&mut self, color: Vec4) {
        self.color = color;
    }

    pub fn render(&self, _rinfo: &RenderInfoClass) -> RendererResult<()> {
        // Note: GPU sphere rendering implementation is in the WGPU renderer backend.
        // SphereClass renders as a point sprite or billboard quad with sphere impostor.
        // C++ equivalent: SphereClass::Render uses DX8 point sprites (pointgr.cpp)
        // WGPU implementation: Geometry shader emulation via vertex expansion
        Ok(())
    }
}

/// Compatibility alias for callers expecting the legacy enum name.
pub use PointMode as PointGroupMode;
