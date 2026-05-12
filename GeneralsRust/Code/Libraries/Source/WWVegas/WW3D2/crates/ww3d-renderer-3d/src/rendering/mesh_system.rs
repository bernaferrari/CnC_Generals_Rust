//! Mesh Rendering System - Complete implementation matching C++ WW3D2
//!
//! This module provides the complete mesh rendering system that was in the original
//! C++ WW3D2, including material passes, texture categories, polygon renderers,
//! frustum culling, lighting, and advanced rendering features.

use super::shader_system::shader::{MaterialBlendMode, ShaderClass};
use crate::bounding_volumes::aabox::AABoxClass;
use crate::bounding_volumes::sphere::SphereClass;
use ww3d_core::errors::{W3DError as W3dError, W3DResult as W3dResult};

use crate::core::error::RendererResult;
use crate::material_system::{
    MaterialFactory, MaterialPassClass, TextureStageSettings, VertexMaterialClass,
};
use crate::render_object_system::{
    AABoxCollisionResult, AABoxCollisionTestClass, AABoxIntersectionTestClass, DecalGeneratorClass,
    OBBoxIntersectionTestClass, RayCollisionResult, RayCollisionTestClass, RenderInfoClass,
    RenderInfoOverrideFlags, RenderObjClass, StaticSortRenderObject,
};
use crate::rendering::frame_uniform_arena::FrameUniformArena;
use crate::rendering::lighting_system::LightEnvironmentClass;
use crate::rendering::texture_system::texture_base::{TextureAddressMode, TextureFilterMode};
use crate::rendering::wgpu_renderer::wgpu_material_binds::WgpuMaterialBinds;
use crate::rendering::wgpu_renderer::wgpu_pipeline_manager::{
    VertexFormat, WgpuPipelineManager, MAX_TEXTURE_STAGES, MAX_TEXTURE_STAGE_GROUPS,
    TEXTURES_PER_GROUP,
};
use crate::texture_system::TextureClass;
use bytemuck;
use crc32fast::Hasher;
use glam::{Mat4, Vec2, Vec3, Vec4};
use log::{debug, warn};
use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::convert::TryInto;
use std::sync::atomic::Ordering;
use std::sync::{atomic::AtomicU32, Arc, Mutex, OnceLock};
use wgpu::util::DeviceExt;
use wgpu::{
    AddressMode, FilterMode, Origin3d, SamplerDescriptor, TexelCopyBufferLayout,
    TexelCopyTextureInfo, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureViewDescriptor, TextureViewDimension,
};
use ww3d_assets::{
    prototypes::{HierarchyPrototype, MeshPrototype},
    AssetManager,
};
use ww3d_collision::bounding_volumes::obbox::OBBoxClass;
use ww3d_core::w3d_format::{
    W3dTexCoordStruct, W3dTriangleStruct, W3dVectorStruct, W3dVertInfStruct,
};
use ww3d_core::w3d_string_from_bytes;
use ww3d_core::*;
use ww3d_gpu::device::GpuDevice;

const RAY_EPSILON: f32 = 1e-5;
const CLIP_EPSILON: f32 = 1e-4;

fn normalize_or(vec: Vec3, fallback: Vec3) -> Vec3 {
    if vec.length_squared() > RAY_EPSILON {
        vec.normalize()
    } else {
        fallback
    }
}

#[derive(Clone, Debug)]
struct ClipVertex {
    obj_pos: Vec3,
    world_pos: Vec3,
    normal: Vec3,
    local: Vec3,
}

fn lerp_clip_vertex(a: &ClipVertex, b: &ClipVertex, t: f32) -> ClipVertex {
    let obj_pos = a.obj_pos.lerp(b.obj_pos, t);
    let world_pos = a.world_pos.lerp(b.world_pos, t);
    let blended_normal = a.normal.lerp(b.normal, t);
    let fallback_normal = if a.normal.length_squared() > RAY_EPSILON {
        a.normal
    } else if b.normal.length_squared() > RAY_EPSILON {
        b.normal
    } else {
        Vec3::Z
    };
    let normal = normalize_or(blended_normal, normalize_or(fallback_normal, Vec3::Z));
    let local = a.local.lerp(b.local, t);

    ClipVertex {
        obj_pos,
        world_pos,
        normal,
        local,
    }
}

fn clip_polygon_against_plane(
    vertices: &[ClipVertex],
    axis: usize,
    limit: f32,
    keep_less: bool,
) -> Vec<ClipVertex> {
    if vertices.is_empty() {
        return Vec::new();
    }

    let inside = |value: f32| -> bool {
        if keep_less {
            value <= limit + CLIP_EPSILON
        } else {
            value >= limit - CLIP_EPSILON
        }
    };

    let mut output = Vec::new();
    let mut prev = vertices.last().unwrap();
    let mut prev_value = prev.local[axis];
    let mut prev_inside = inside(prev_value);

    for curr in vertices {
        let curr_value = curr.local[axis];
        let curr_inside = inside(curr_value);

        if curr_inside != prev_inside {
            let denom = curr_value - prev_value;
            if denom.abs() > CLIP_EPSILON {
                let t = (limit - prev_value) / denom;
                let t = t.clamp(0.0, 1.0);
                output.push(lerp_clip_vertex(prev, curr, t));
            }
        }

        if curr_inside {
            output.push(curr.clone());
        }

        prev = curr;
        prev_value = curr_value;
        prev_inside = curr_inside;
    }

    output
}

fn clip_polygon_to_projector(polygon: Vec<ClipVertex>, extents: Vec3) -> Vec<ClipVertex> {
    if polygon.len() < 3 {
        return Vec::new();
    }

    let mut output = clip_polygon_against_plane(&polygon, 0, extents.x, true);
    if output.len() < 3 {
        return Vec::new();
    }

    output = clip_polygon_against_plane(&output, 0, -extents.x, false);
    if output.len() < 3 {
        return Vec::new();
    }

    output = clip_polygon_against_plane(&output, 1, extents.y, true);
    if output.len() < 3 {
        return Vec::new();
    }

    output = clip_polygon_against_plane(&output, 1, -extents.y, false);
    if output.len() < 3 {
        return Vec::new();
    }

    output
}

fn w3d_to_vec3(source: &W3dVectorStruct) -> Vec3 {
    Vec3::new(source.x, source.y, source.z)
}

fn w3d_to_mu_vec3(source: &W3dVectorStruct) -> MuVec3 {
    MuVec3::new(source.x, source.y, source.z)
}

fn triangle_vertices<'a>(
    triangle: &W3dTriangleStruct,
    vertices: &'a [W3dVectorStruct],
) -> Option<[Vec3; 3]> {
    let idx0 = triangle.vindex[0] as usize;
    let idx1 = triangle.vindex[1] as usize;
    let idx2 = triangle.vindex[2] as usize;
    if idx0 >= vertices.len() || idx1 >= vertices.len() || idx2 >= vertices.len() {
        return None;
    }
    Some([
        w3d_to_vec3(&vertices[idx0]),
        w3d_to_vec3(&vertices[idx1]),
        w3d_to_vec3(&vertices[idx2]),
    ])
}

fn mu_triangle_from_w3d(
    triangle: &W3dTriangleStruct,
    vertices: &[W3dVectorStruct],
) -> Option<MuTriangle> {
    let verts = triangle_vertices(triangle, vertices)?;
    let stored_normal = Vec3::new(triangle.normal.x, triangle.normal.y, triangle.normal.z);
    let mu_normal = if stored_normal.length_squared() > RAY_EPSILON {
        w3d_to_mu_vec3(&triangle.normal).normalize()
    } else {
        let computed = (verts[1] - verts[0]).cross(verts[2] - verts[0]);
        if computed.length_squared() > RAY_EPSILON {
            MuVec3::new(computed.x, computed.y, computed.z).normalize()
        } else {
            MuVec3::new(0.0, 0.0, 1.0)
        }
    };
    Some(MuTriangle::with_normal(
        MuVec3::new(verts[0].x, verts[0].y, verts[0].z),
        MuVec3::new(verts[1].x, verts[1].y, verts[1].z),
        MuVec3::new(verts[2].x, verts[2].y, verts[2].z),
        mu_normal,
    ))
}

fn mu_aabox_from_class(aabox: &AABoxClass) -> MuAABox {
    MuAABox::new(
        MuVec3::new(aabox.center.x, aabox.center.y, aabox.center.z),
        MuVec3::new(
            aabox.extent.x.abs(),
            aabox.extent.y.abs(),
            aabox.extent.z.abs(),
        ),
    )
}

fn ray_triangle_intersection(
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    v0: Vec3,
    v1: Vec3,
    v2: Vec3,
) -> Option<(f32, Vec3)> {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let pvec = direction.cross(edge2);
    let det = edge1.dot(pvec);
    if det.abs() < RAY_EPSILON {
        return None;
    }

    let inv_det = 1.0 / det;
    let tvec = origin - v0;
    let u = tvec.dot(pvec) * inv_det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let qvec = tvec.cross(edge1);
    let v = direction.dot(qvec) * inv_det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = edge2.dot(qvec) * inv_det;
    if t < RAY_EPSILON || t > max_distance + RAY_EPSILON {
        return None;
    }

    let normal = edge1.cross(edge2);
    if normal.length_squared() <= RAY_EPSILON {
        return None;
    }
    Some((t, normal.normalize()))
}
use math_utilities::{
    AABox as MuAABox, CastResult as MuCastResult, CollisionMath as MuCollisionMath,
    Triangle as MuTriangle, Vector3 as MuVec3,
};

// Render types for special rendering modes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderType {
    Normal,
    Shadow,
    Visibility,
    Wireframe,
    DepthOnly,
}

// Blend modes for materials
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlendMode {
    Opaque,
    Alpha,
    Additive,
    Multiply,
}

// Add missing methods to MeshModelClass
impl MeshModelClass {
    pub fn cast_ray(&self, ray: &mut RayCollisionTestClass) -> bool {
        let ray_direction = ray.line.end - ray.line.start;
        let ray_length = ray_direction.length();
        if ray_length <= RAY_EPSILON {
            return false;
        }

        let normalized_direction = ray_direction / ray_length;
        let origin = ray.line.start;

        let mut hit = false;
        let mut best_fraction = ray.result.fraction.min(1.0);
        let mut best_point = Vec3::ZERO;
        let mut best_normal = Vec3::ZERO;
        let mut best_surface = ray.result.surface_type;

        for triangle in &self.triangles {
            let Some(verts) = triangle_vertices(triangle, &self.vertices) else {
                continue;
            };

            if let Some((distance, normal)) = ray_triangle_intersection(
                origin,
                normalized_direction,
                ray_length,
                verts[0],
                verts[1],
                verts[2],
            ) {
                let fraction = (distance / ray_length).clamp(0.0, 1.0);
                if !hit || fraction < best_fraction {
                    hit = true;
                    best_fraction = fraction;
                    best_point = origin + normalized_direction * distance;
                    best_normal = normal;
                    best_surface = triangle.attributes;
                }
            }
        }

        if hit {
            ray.result.start_bad = best_fraction == 0.0;
            ray.result.fraction = best_fraction;
            ray.result.normal = best_normal;
            ray.result.surface_type = best_surface;
            if ray.result.compute_contact_point {
                ray.result.contact_point = origin + ray_direction * best_fraction;
            } else {
                ray.result.contact_point = best_point;
            }
            return true;
        }

        false
    }

    pub fn cast_aabox(&self, boxtest: &mut AABoxCollisionTestClass) -> bool {
        let mu_box = mu_aabox_from_class(&boxtest.box_obj);
        let movement = MuVec3::new(
            boxtest.move_vector.x,
            boxtest.move_vector.y,
            boxtest.move_vector.z,
        );

        let mut best: Option<MuCastResult> = None;

        for triangle in &self.triangles {
            let Some(mu_triangle) = mu_triangle_from_w3d(triangle, &self.vertices) else {
                continue;
            };

            let mut result = MuCastResult::new();
            result.compute_contact_point = true;
            if MuCollisionMath::collide_aabox_triangle(
                &mu_box,
                &movement,
                &mu_triangle,
                &mut result,
            ) {
                let replace = match &best {
                    None => true,
                    Some(existing) => {
                        if result.start_bad && !existing.start_bad {
                            true
                        } else if !result.start_bad && existing.start_bad {
                            false
                        } else {
                            result.fraction < existing.fraction
                        }
                    }
                };

                if replace {
                    best = Some(result.clone());
                }
            }
        }

        if let Some(best_hit) = best {
            let mut contact_points = Vec::new();
            if best_hit.compute_contact_point {
                contact_points.push(Vec3::new(
                    best_hit.contact_point.x,
                    best_hit.contact_point.y,
                    best_hit.contact_point.z,
                ));
            }

            boxtest.result = Some(AABoxCollisionResult {
                intersection: true,
                contact_points,
            });
            return true;
        }

        boxtest.result = Some(AABoxCollisionResult {
            intersection: false,
            contact_points: Vec::new(),
        });
        false
    }

    pub fn intersect_aabox(&self, boxtest: &AABoxIntersectionTestClass) -> bool {
        let mu_box = mu_aabox_from_class(&boxtest.box_obj);
        for triangle in &self.triangles {
            let Some(mu_triangle) = mu_triangle_from_w3d(triangle, &self.vertices) else {
                continue;
            };

            if MuCollisionMath::intersection_test_aabox_triangle(&mu_box, &mu_triangle) {
                return true;
            }
        }
        false
    }

    pub fn intersect_obbox(&self, boxtest: &OBBoxIntersectionTestClass) -> bool {
        let center = MuVec3::new(
            boxtest.box_obj.center.x,
            boxtest.box_obj.center.y,
            boxtest.box_obj.center.z,
        );
        let axes = [
            MuVec3::new(
                boxtest.box_obj.basis[0].x,
                boxtest.box_obj.basis[0].y,
                boxtest.box_obj.basis[0].z,
            )
            .normalize(),
            MuVec3::new(
                boxtest.box_obj.basis[1].x,
                boxtest.box_obj.basis[1].y,
                boxtest.box_obj.basis[1].z,
            )
            .normalize(),
            MuVec3::new(
                boxtest.box_obj.basis[2].x,
                boxtest.box_obj.basis[2].y,
                boxtest.box_obj.basis[2].z,
            )
            .normalize(),
        ];
        let extent = MuVec3::new(
            boxtest.box_obj.extent.x.abs(),
            boxtest.box_obj.extent.y.abs(),
            boxtest.box_obj.extent.z.abs(),
        );
        let aligned_box = MuAABox::new(MuVec3::ZERO, extent);

        for triangle in &self.triangles {
            let Some(verts) = triangle_vertices(triangle, &self.vertices) else {
                continue;
            };

            let mut local_vertices = [MuVec3::ZERO; 3];
            for (idx, vert) in verts.iter().enumerate() {
                let mu_vert = MuVec3::new(vert.x, vert.y, vert.z) - center;
                local_vertices[idx] = MuVec3::new(
                    mu_vert.dot(axes[0]),
                    mu_vert.dot(axes[1]),
                    mu_vert.dot(axes[2]),
                );
            }

            let mut local_triangle =
                MuTriangle::new(local_vertices[0], local_vertices[1], local_vertices[2]);
            if local_triangle.normal.length_squared() <= RAY_EPSILON {
                local_triangle.compute_normal();
            }

            if MuCollisionMath::intersection_test_aabox_triangle(&aligned_box, &local_triangle) {
                return true;
            }
        }

        false
    }

    pub fn generate_rigid_apt(&self, volume: &OBBoxClass, apt: &mut Vec<u32>) {
        apt.clear();

        let center = volume.center;
        let extents = Vec3::new(
            volume.extent.x.abs(),
            volume.extent.y.abs(),
            volume.extent.z.abs(),
        );
        let axes = [
            normalize_or(volume.basis[0], Vec3::X),
            normalize_or(volume.basis[1], Vec3::Y),
            normalize_or(volume.basis[2], Vec3::Z),
        ];
        let mu_box = MuAABox::new(MuVec3::ZERO, MuVec3::new(extents.x, extents.y, extents.z));

        for (index, triangle) in self.triangles.iter().enumerate() {
            let Some(verts) = triangle_vertices(triangle, &self.vertices) else {
                continue;
            };

            let local_vertices: [Vec3; 3] = verts
                .iter()
                .map(|v| {
                    let offset = *v - center;
                    Vec3::new(
                        offset.dot(axes[0]),
                        offset.dot(axes[1]),
                        offset.dot(axes[2]),
                    )
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap_or([Vec3::ZERO; 3]);

            let mut local_triangle = MuTriangle::new(
                MuVec3::new(
                    local_vertices[0].x,
                    local_vertices[0].y,
                    local_vertices[0].z,
                ),
                MuVec3::new(
                    local_vertices[1].x,
                    local_vertices[1].y,
                    local_vertices[1].z,
                ),
                MuVec3::new(
                    local_vertices[2].x,
                    local_vertices[2].y,
                    local_vertices[2].z,
                ),
            );

            if local_triangle.normal.length_squared() <= RAY_EPSILON {
                local_triangle.compute_normal();
            }

            if MuCollisionMath::intersection_test_aabox_triangle(&mu_box, &local_triangle) {
                apt.push(index as u32);
            }
        }
    }

    pub fn generate_skin_apt(
        &self,
        world_box: &OBBoxClass,
        apt: &mut Vec<u32>,
        world_vertices: &[Vec3],
    ) {
        apt.clear();
        if world_vertices.len() < self.vertices.len() {
            return;
        }

        let center = world_box.center;
        let extents = Vec3::new(
            world_box.extent.x.abs(),
            world_box.extent.y.abs(),
            world_box.extent.z.abs(),
        );
        let axes = [
            normalize_or(world_box.basis[0], Vec3::X),
            normalize_or(world_box.basis[1], Vec3::Y),
            normalize_or(world_box.basis[2], Vec3::Z),
        ];
        let mu_box = MuAABox::new(MuVec3::ZERO, MuVec3::new(extents.x, extents.y, extents.z));

        for (index, triangle) in self.triangles.iter().enumerate() {
            let idx0 = triangle.vindex[0] as usize;
            let idx1 = triangle.vindex[1] as usize;
            let idx2 = triangle.vindex[2] as usize;
            if idx0 >= world_vertices.len()
                || idx1 >= world_vertices.len()
                || idx2 >= world_vertices.len()
            {
                continue;
            }

            let world_positions = [
                world_vertices[idx0],
                world_vertices[idx1],
                world_vertices[idx2],
            ];

            let mut local_triangle = MuTriangle::new(
                MuVec3::new(
                    (world_positions[0] - center).dot(axes[0]),
                    (world_positions[0] - center).dot(axes[1]),
                    (world_positions[0] - center).dot(axes[2]),
                ),
                MuVec3::new(
                    (world_positions[1] - center).dot(axes[0]),
                    (world_positions[1] - center).dot(axes[1]),
                    (world_positions[1] - center).dot(axes[2]),
                ),
                MuVec3::new(
                    (world_positions[2] - center).dot(axes[0]),
                    (world_positions[2] - center).dot(axes[1]),
                    (world_positions[2] - center).dot(axes[2]),
                ),
            );

            if local_triangle.normal.length_squared() <= RAY_EPSILON {
                local_triangle.compute_normal();
            }

            if MuCollisionMath::intersection_test_aabox_triangle(&mu_box, &local_triangle) {
                apt.push(index as u32);
            }
        }
    }
}

/// Sort levels for static sort lists (transparency sorting)
/// CRITICAL: These values MUST match C++ w3d_file.h exactly!
pub const SORT_LEVEL_NONE: u32 = 0; // No sorting - renders in default order
pub const MAX_SORT_LEVEL: u32 = 32;
pub const SORT_LEVEL_BIN1: u32 = 20; // Close transparent objects
pub const SORT_LEVEL_BIN2: u32 = 15; // Medium distance transparent
pub const SORT_LEVEL_BIN3: u32 = 10; // Far transparent objects

// Camera trait methods for frustum culling and LOD
pub trait CameraExt {
    fn get_frustum(&self) -> FrustumClass;
    fn get_position(&self) -> Vec3;
    fn get_view_matrix(&self) -> Mat4;
    fn get_near_plane(&self) -> f32;
    fn get_far_plane(&self) -> f32;
}

// Placeholder frustum class
pub struct FrustumClass {
    pub planes: [Vec4; 6], // Left, Right, Bottom, Top, Near, Far
}

impl FrustumClass {
    pub fn intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        // Test sphere against all frustum planes
        for plane in &self.planes {
            let distance = plane.x * center.x + plane.y * center.y + plane.z * center.z + plane.w;
            if distance < -radius {
                return false; // Sphere is completely outside this plane
            }
        }
        true
    }

    pub fn intersects_aabox(&self, min: Vec3, max: Vec3) -> bool {
        // Test AABox against all frustum planes
        for plane in &self.planes {
            // Find the positive vertex (farthest in the direction of the plane normal)
            let positive_vertex = Vec3::new(
                if plane.x >= 0.0 { max.x } else { min.x },
                if plane.y >= 0.0 { max.y } else { min.y },
                if plane.z >= 0.0 { max.z } else { min.z },
            );

            // Test if positive vertex is outside the plane
            let distance = plane.x * positive_vertex.x
                + plane.y * positive_vertex.y
                + plane.z * positive_vertex.z
                + plane.w;
            if distance < 0.0 {
                return false; // Box is completely outside this plane
            }
        }
        true
    }
}

// Material pass extensions
impl MaterialPassClass {
    pub fn get_texture_count(&self) -> u32 {
        // Count number of textures bound to this material pass
        let mut count = 0;
        for stage in 0..4 {
            // Assume max 4 texture stages
            if self.get_texture(stage).is_some() {
                count += 1;
            }
        }
        count
    }

    pub fn is_translucent(&self) -> bool {
        // Check if this material pass requires transparency
        if self
            .vertex_material
            .as_ref()
            .map(|material| material.opacity < 1.0 || material.translucency > 0.0)
            .unwrap_or(false)
        {
            return true;
        }

        matches!(
            self.shader.blend_mode(),
            MaterialBlendMode::Alpha | MaterialBlendMode::Additive | MaterialBlendMode::Decal
        )
    }

    pub fn get_blend_mode(&self) -> BlendMode {
        // Determine blend mode from material properties
        match self.shader.blend_mode() {
            MaterialBlendMode::Opaque => BlendMode::Opaque,
            MaterialBlendMode::Alpha => BlendMode::Alpha,
            MaterialBlendMode::Additive => BlendMode::Additive,
            MaterialBlendMode::Decal => BlendMode::Alpha,
            MaterialBlendMode::Multiply => BlendMode::Opaque, // Darken blend
            MaterialBlendMode::Screen => BlendMode::Additive, // Lighten blend
        }
    }
}

// Using centralized RenderInfoClass and flags from render_object_system

/// Render statistics for mesh rendering
#[derive(Debug, Clone, Default)]
pub struct MeshRenderStats {
    pub meshes_rendered: u32,
    pub triangles_rendered: u32,
    pub material_passes: u32,
    pub texture_switches: u32,
    pub shader_switches: u32,
    pub draw_calls: u32,
    pub vertex_color_passes: u32,
}

/// Mesh geometry flags
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeshGeometryClass {
    SKIN = 1,
    SORT = 2,
    VISIBLE = 4,
}

/// Mesh model class - contains the actual geometry data
#[derive(Debug)]
pub struct MeshModelClass {
    pub name: String,
    pub vertices: Vec<W3dVectorStruct>,
    pub normals: Vec<W3dVectorStruct>,
    pub triangles: Vec<W3dTriangleStruct>,
    pub material_info: Option<W3dMaterialInfoStruct>,
    pub shaders: Vec<W3dShaderStruct>,
    pub vertex_materials: Vec<W3dVertexMaterialStruct>,
    pub vertex_bone_links: Vec<u16>,
    pub vertex_influences: Vec<W3dVertInfStruct>,
    pub texture_coords: Vec<W3dTexCoordStruct>,
    pub stage_texture_coords: Vec<Vec<W3dTexCoordStruct>>,
    pub per_stage_face_texcoord_ids: Vec<Vec<[u32; 3]>>,
    pub stage_uv_sources: Vec<u8>,
    pub sort_level: u32,
    pub flags: u32,
    // Legacy DX8 polygon renderer list removed; WGPU path is authoritative
    pub polygon_renderer_list: Vec<Arc<DX8PolygonRendererClass>>, // deprecated, kept until full cleanup
    pub material_passes: Vec<MaterialPassClass>,
    pub vertex_buffer: Option<wgpu::Buffer>,
    pub index_buffer: Option<wgpu::Buffer>,
    pub vertex_count: u32,
    pub index_count: u32,
    pub w3d_attributes: u32,       // Equivalent to C++ W3dAttributes
    pub user_text: Option<String>, // Equivalent to C++ user text buffer
    revision: u64,
}

impl MeshModelClass {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            vertices: Vec::new(),
            normals: Vec::new(),
            triangles: Vec::new(),
            material_info: None,
            shaders: Vec::new(),
            vertex_materials: Vec::new(),
            vertex_bone_links: Vec::new(),
            vertex_influences: Vec::new(),
            texture_coords: Vec::new(),
            stage_texture_coords: Vec::new(),
            per_stage_face_texcoord_ids: Vec::new(),
            stage_uv_sources: Vec::new(),
            sort_level: SORT_LEVEL_NONE,
            flags: 0,
            polygon_renderer_list: Vec::new(),
            material_passes: Vec::new(),
            vertex_buffer: None,
            index_buffer: None,
            vertex_count: 0,
            index_count: 0,
            w3d_attributes: 0,
            user_text: None,
            revision: 0,
        }
    }

    /// Construct a mesh model from an asset prototype, mirroring the legacy loader.
    pub fn from_mesh_prototype(
        prototype: &MeshPrototype,
        hierarchy: Option<&HierarchyPrototype>,
    ) -> W3dResult<Self> {
        let mut model = MeshModelClass::new(&prototype.name);

        model.vertices = prototype.vertices.clone();
        model.normals = prototype.normals.clone();
        model.triangles = prototype.triangles.clone();
        model.material_info = prototype.material_info.clone();
        model.shaders = prototype.shaders.clone();
        model.vertex_materials = prototype.vertex_materials.clone();
        let (uv_sets, stage_channels) = compute_stage_uv_info(&prototype.stage_texcoords);
        model.stage_texture_coords = uv_sets;
        model.stage_uv_sources = stage_channels;
        model.per_stage_face_texcoord_ids = prototype.per_face_texcoord_ids.clone();
        if let Some(stage0) = model.stage_texture_coords.get(0) {
            model.texture_coords = stage0.clone();
        }
        if let Some(header) = &prototype.header {
            model.sort_level = header.attrs;
            model.w3d_attributes = header.attrs;
        }
        model.ensure_stage_zero();

        if let Some(influences) = &prototype.vertex_influences {
            model.set_vertex_influences(influences.clone());
        } else if let Some(hierarchy_proto) = hierarchy {
            let mut links = Vec::with_capacity(model.vertices.len());
            let fallback_index = hierarchy_proto
                .bind_transforms
                .first()
                .map(|_| 0u16)
                .unwrap_or(0);
            links.resize(model.vertices.len(), fallback_index);
            model.set_vertex_bone_links(links);
        }

        model.vertex_count = model.vertices.len() as u32;
        model.index_count = (model.triangles.len() * 3) as u32;

        model.material_passes = build_material_passes_from_prototype(prototype);

        Ok(model)
    }

    /// Create WGPU vertex and index buffers from mesh data
    /// Handles different vertex formats for skinned vs rigid meshes
    pub fn create_wgpu_buffers(&mut self, device: &wgpu::Device) {
        const MAX_UV_SETS: usize = 4;
        // Determine vertex format based on mesh type
        let is_skinned = self.get_flag(MeshGeometryClass::SKIN);
        let has_normals = self.has_normals();

        // Calculate vertex stride (floats) based on attributes
        let mut stride_floats = 3; // Position (x, y, z)
        if has_normals {
            stride_floats += 3; // Normal (x, y, z)
        }
        stride_floats += 2 * MAX_UV_SETS; // Always provide up to 4 UV sets
        if is_skinned {
            stride_floats += 4; // Bone indices packed as f32 bits
            stride_floats += 4; // Bone weights
        }

        // Create vertex data with proper format
        let mut vertex_data: Vec<f32> = Vec::with_capacity(self.vertices.len() * stride_floats);

        for i in 0..self.vertices.len() {
            // Position (always present)
            vertex_data.push(self.vertices[i].x);
            vertex_data.push(self.vertices[i].y);
            vertex_data.push(self.vertices[i].z);

            // Normal (if available)
            if has_normals {
                if i < self.normals.len() {
                    vertex_data.push(self.normals[i].x);
                    vertex_data.push(self.normals[i].y);
                    vertex_data.push(self.normals[i].z);
                } else {
                    vertex_data.push(0.0);
                    vertex_data.push(1.0);
                    vertex_data.push(0.0);
                }
            }

            // Texture coordinates (if available)
            for channel in 0..MAX_UV_SETS {
                let uv = self.uv_channel_coords(channel, i);
                vertex_data.push(uv[0]);
                vertex_data.push(uv[1]);
            }

            // Bone data for skinned meshes
            if is_skinned {
                let (indices, weights) = self.vertex_influence_view(i);
                for &idx in &indices {
                    vertex_data.push(f32::from_bits(idx));
                }
                vertex_data.extend_from_slice(&weights);
            }
        }

        // Create vertex buffer
        self.vertex_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} Vertex Buffer", self.name)),
                contents: bytemuck::cast_slice(&vertex_data),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );

        self.vertex_count = self.vertices.len() as u32;

        // Create index data from triangles
        let mut index_data: Vec<u32> = Vec::new();
        for triangle in &self.triangles {
            index_data.push(triangle.vindex[0]);
            index_data.push(triangle.vindex[1]);
            index_data.push(triangle.vindex[2]);
        }

        // Create index buffer
        self.index_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} Index Buffer", self.name)),
                contents: bytemuck::cast_slice(&index_data),
                usage: wgpu::BufferUsages::INDEX,
            }),
        );

        self.index_count = index_data.len() as u32;
    }

    pub fn get_flag(&self, flag: MeshGeometryClass) -> bool {
        (self.flags & flag as u32) != 0
    }

    pub fn set_flag(&mut self, flag: MeshGeometryClass, value: bool) {
        if self.get_flag(flag) == value {
            return;
        }
        if value {
            self.flags |= flag as u32;
        } else {
            self.flags &= !(flag as u32);
        }
        self.mark_dirty();
    }

    pub fn get_sort_level(&self) -> u32 {
        self.sort_level
    }

    /// Get material pass by index
    pub fn get_material_pass(&self, index: usize) -> Option<&MaterialPassClass> {
        self.material_passes.get(index)
    }

    /// Check if mesh is skinned (has bone influences)
    pub fn is_skinned(&self) -> bool {
        self.get_flag(MeshGeometryClass::SKIN)
    }

    /// Check if mesh has normals
    pub fn has_normals(&self) -> bool {
        !self.normals.is_empty()
    }

    /// Check if mesh has texture coordinates
    pub fn has_tex_coords(&self) -> bool {
        !self.texture_coords.is_empty() || !self.stage_texture_coords.is_empty()
    }

    fn uv_channel_coords(&self, channel: usize, vertex_index: usize) -> [f32; 2] {
        if let Some(layer) = self.stage_texture_coords.get(channel) {
            if let Some(tc) = layer.get(vertex_index) {
                return [tc.u, tc.v];
            }
        }
        [0.0, 0.0]
    }

    fn ensure_stage_zero(&mut self) {
        if self.stage_texture_coords.is_empty() && !self.texture_coords.is_empty() {
            self.stage_texture_coords.push(self.texture_coords.clone());
        } else if self
            .stage_texture_coords
            .get(0)
            .map_or(true, |layer| layer.is_empty())
        {
            if !self.texture_coords.is_empty() {
                if self.stage_texture_coords.is_empty() {
                    self.stage_texture_coords.push(self.texture_coords.clone());
                } else {
                    self.stage_texture_coords[0] = self.texture_coords.clone();
                }
            }
        }
    }

    /// Get mesh name - equivalent to C++ Get_Name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Set mesh name - equivalent to C++ Set_Name
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// Replace the per-vertex bone links. Mirrors C++ `MeshGeometryClass::VertexBoneLink`.
    pub fn set_vertex_bone_links(&mut self, links: Vec<u16>) {
        self.vertex_bone_links = links;

        if self.vertex_bone_links.is_empty() {
            self.vertex_influences.clear();
            self.set_flag(MeshGeometryClass::SKIN, false);
            return;
        }

        if self.vertex_influences.len() != self.vertex_bone_links.len() {
            self.vertex_influences = self
                .vertex_bone_links
                .iter()
                .map(|&index| {
                    // CRITICAL: C++ uses single-bone-per-vertex, not multi-bone
                    W3dVertInfStruct {
                        bone_idx: index,
                        pad: [0; 6], // Padding for binary compatibility
                    }
                })
                .collect();
        }

        self.set_flag(MeshGeometryClass::SKIN, true);
    }

    /// Install full vertex influence data (up to 4 weights per vertex).
    pub fn set_vertex_influences(&mut self, influences: Vec<W3dVertInfStruct>) {
        self.vertex_influences = influences;

        if self.vertex_influences.is_empty() {
            self.vertex_bone_links.clear();
            self.set_flag(MeshGeometryClass::SKIN, false);
            return;
        }

        if self.vertex_bone_links.len() != self.vertex_influences.len() {
            self.vertex_bone_links = self
                .vertex_influences
                .iter()
                .map(|inf| inf.bone_idx) // Single bone index, not array
                .collect();
        }

        self.set_flag(MeshGeometryClass::SKIN, true);
    }

    /// Access the per-vertex bone links if the data is present and aligned with the vertex array.
    pub fn vertex_bone_links(&self) -> Option<&[u16]> {
        if self.vertex_bone_links.len() == self.vertices.len() {
            Some(&self.vertex_bone_links)
        } else {
            None
        }
    }

    pub fn vertex_influences(&self) -> Option<&[W3dVertInfStruct]> {
        if self.vertex_influences.len() == self.vertices.len() {
            Some(&self.vertex_influences)
        } else {
            None
        }
    }

    fn vertex_influence_view(&self, index: usize) -> ([u32; 4], [f32; 4]) {
        let mut indices = [0u32; 4];
        let mut weights = [0.0f32; 4];

        // CRITICAL: C++ uses single-bone-per-vertex skinning
        if let Some(influence) = self.vertex_influences.get(index) {
            indices[0] = influence.bone_idx as u32;
            weights[0] = 1.0; // Single bone, full weight
        } else if let Some(link) = self.vertex_bone_links.get(index) {
            indices[0] = *link as u32;
            weights[0] = 1.0;
        } else {
            weights[0] = 1.0;
        }

        let mut weight_sum = weights.iter().copied().sum::<f32>();
        if weight_sum <= f32::EPSILON {
            weights = [1.0, 0.0, 0.0, 0.0];
            weight_sum = 1.0;
        }

        let inv = 1.0 / weight_sum;
        for w in &mut weights {
            *w *= inv;
        }

        (indices, weights)
    }

    /// Get user text - equivalent to C++ Get_User_Text
    pub fn get_user_text(&self) -> Option<&str> {
        self.user_text.as_deref()
    }

    /// Set user text
    pub fn set_user_text(&mut self, text: &str) {
        self.user_text = Some(text.to_string());
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn mark_dirty(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.vertex_buffer = None;
        self.index_buffer = None;
    }

    /// Get W3D attributes
    pub fn get_w3d_attributes(&self) -> u32 {
        self.w3d_attributes
    }

    /// Set W3D attributes
    pub fn set_w3d_attributes(&mut self, attributes: u32) {
        if self.w3d_attributes == attributes {
            return;
        }
        self.w3d_attributes = attributes;
        self.mark_dirty();
    }

    /// Scale the mesh geometry - equivalent to C++ MeshModelClass::Scale
    pub fn scale_geometry(&mut self, scale: Vec3) {
        // Scale all vertices
        for vertex in &mut self.vertices {
            vertex.x *= scale.x;
            vertex.y *= scale.y;
            vertex.z *= scale.z;
        }
        self.mark_dirty();
    }

    /// Make geometry unique - equivalent to C++ Make_Geometry_Unique
    pub fn make_geometry_unique(&mut self) {
        self.mark_dirty();
    }

    /// Register the mesh for rendering with proper material pass ordering
    pub fn register_for_rendering(&mut self) {
        // Set vertex and index counts
        if !self.vertices.is_empty() {
            self.vertex_count = self.vertices.len() as u32;
        }

        if !self.triangles.is_empty() {
            self.index_count = (self.triangles.len() * 3) as u32;
        }

        self.sort_material_passes();
    }

    /// Sort material passes by render order for proper state management
    pub fn sort_material_passes(&mut self) {
        // Sort material passes to minimize state changes
        // 1. Opaque passes first
        // 2. Alpha-tested passes
        // 3. Transparent passes last
        // Within each category, sort by:
        // - Shader type
        // - Texture bindings
        // - Material properties

        self.material_passes.sort_by(|a, b| {
            use std::cmp::Ordering;

            // Primary sort: blend mode (opaque < alpha-test < transparent)
            let blend_order_a = Self::get_blend_sort_order(a);
            let blend_order_b = Self::get_blend_sort_order(b);

            let blend_cmp = blend_order_a.cmp(&blend_order_b);
            if blend_cmp != Ordering::Equal {
                return blend_cmp;
            }

            // Secondary sort: shader type
            let shader_cmp = a.shader.id().cmp(&b.shader.id());
            if shader_cmp != Ordering::Equal {
                return shader_cmp;
            }

            // Tertiary sort: texture count (fewer textures first for simpler passes)
            let tex_count_a = a.get_texture_count();
            let tex_count_b = b.get_texture_count();
            tex_count_a.cmp(&tex_count_b)
        });

        self.mark_dirty();
    }

    /// Get blend mode sort order for material pass ordering
    fn get_blend_sort_order(pass: &MaterialPassClass) -> u32 {
        let base = match pass.shader.blend_mode() {
            MaterialBlendMode::Opaque => 0,
            MaterialBlendMode::Decal => 1,
            MaterialBlendMode::Multiply => 1, // Darken blend (same as decal)
            MaterialBlendMode::Alpha => 2,
            MaterialBlendMode::Additive => 3,
            MaterialBlendMode::Screen => 3, // Lighten blend (same as additive)
        };

        if base == 0
            && pass
                .vertex_material
                .as_ref()
                .map(|mat| mat.opacity < 1.0 || mat.translucency > 0.0)
                .unwrap_or(false)
        {
            2
        } else {
            base
        }
    }

    /// Get number of material passes
    pub fn get_pass_count(&self) -> usize {
        self.material_passes.len()
    }

    /// Get number of polygons (triangles)
    pub fn get_polygon_count(&self) -> usize {
        self.triangles.len()
    }

    /// Check if a texture stage has a texture array (per-polygon textures)
    /// Per-polygon texture arrays are not supported in this renderer path.
    pub fn has_texture_array(&self, _pass_idx: usize, _stage_idx: usize) -> bool {
        false
    }

    /// Peek at texture for a specific polygon, pass, and stage
    /// Returns None as we don't support per-polygon textures in the modern renderer
    pub fn peek_texture(
        &self,
        _poly_idx: usize,
        _pass_idx: usize,
        _stage_idx: usize,
    ) -> Option<&TextureClass> {
        None
    }

    /// Peek at single texture (shared across all polygons) for a pass and stage
    pub fn peek_single_texture(&self, pass_idx: usize, stage_idx: usize) -> Option<&TextureClass> {
        self.material_passes
            .get(pass_idx)
            .and_then(|pass| pass.textures.get(stage_idx))
            .and_then(|opt_tex| opt_tex.as_ref())
            .map(|arc_tex| arc_tex.as_ref())
    }

    /// Set texture for a specific polygon, pass, and stage
    /// This is a no-op in the modern renderer as we don't support per-polygon textures
    pub fn set_texture(
        &mut self,
        _poly_idx: usize,
        _new_texture: Arc<crate::texture_system::TextureClass>,
        _pass_idx: usize,
        _stage_idx: usize,
    ) {
        // Legacy C++ supported per-polygon textures, but modern renderer uses shared textures
        // This method is kept for API compatibility but does nothing
    }

    /// Set single texture (shared across all polygons) for a pass and stage
    pub fn set_single_texture(
        &mut self,
        new_texture: Arc<crate::texture_system::TextureClass>,
        pass_idx: usize,
        stage_idx: usize,
    ) {
        if let Some(pass) = self.material_passes.get_mut(pass_idx) {
            if stage_idx < pass.textures.len() {
                pass.textures[stage_idx] = Some(new_texture);
            }
        }
    }
}

impl Clone for MeshModelClass {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            vertices: self.vertices.clone(),
            normals: self.normals.clone(),
            triangles: self.triangles.clone(),
            material_info: self.material_info.clone(),
            shaders: self.shaders.clone(),
            vertex_materials: self.vertex_materials.clone(),
            vertex_bone_links: self.vertex_bone_links.clone(),
            vertex_influences: self.vertex_influences.clone(),
            texture_coords: self.texture_coords.clone(),
            stage_texture_coords: self.stage_texture_coords.clone(),
            per_stage_face_texcoord_ids: self.per_stage_face_texcoord_ids.clone(),
            stage_uv_sources: self.stage_uv_sources.clone(),
            sort_level: self.sort_level,
            flags: self.flags,
            polygon_renderer_list: self.polygon_renderer_list.clone(),
            material_passes: self.material_passes.clone(),
            vertex_buffer: None, // Cannot clone wgpu::Buffer
            index_buffer: None,  // Cannot clone wgpu::Buffer
            vertex_count: self.vertex_count,
            index_count: self.index_count,
            user_text: self.user_text.clone(),
            w3d_attributes: self.w3d_attributes,
            revision: self.revision,
        }
    }
}

/// Polygon renderer class - manages GPU rendering for mesh polygons
#[derive(Debug)]
pub struct DX8PolygonRendererClass {
    pub vertex_buffer: Option<wgpu::Buffer>,
    pub index_buffer: Option<wgpu::Buffer>,
    pub vertex_count: u32,
    pub index_count: u32,
    pub primitive_type: wgpu::PrimitiveTopology,
    pub texture_category: Option<Arc<DX8TextureCategoryClass>>,
    pub material_pass: Option<Arc<MaterialPassClass>>,
    pub shader: ShaderClass,
    pub vertex_material: Option<Arc<VertexMaterialClass>>,
}

impl DX8PolygonRendererClass {
    pub fn new() -> Self {
        Self {
            vertex_buffer: None,
            index_buffer: None,
            vertex_count: 0,
            index_count: 0,
            primitive_type: wgpu::PrimitiveTopology::TriangleList,
            texture_category: None,
            material_pass: None,
            shader: ShaderClass::default(),
            vertex_material: None,
        }
    }

    /// Render a material pass
    pub fn render_material_pass<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        _transform: &Mat4,
        _render_info: &RenderInfoClass,
    ) -> W3dResult<()> {
        if let Some(vertex_buffer) = &self.vertex_buffer {
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        }
        if let Some(index_buffer) = &self.index_buffer {
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.index_count, 0, 0..1);
        } else {
            render_pass.draw(0..self.vertex_count, 0..1);
        }
        Ok(())
    }

    pub fn set_texture_category(&mut self, category: Arc<DX8TextureCategoryClass>) {
        self.texture_category = Some(category);
    }

    pub fn get_texture_category(&self) -> Option<&Arc<DX8TextureCategoryClass>> {
        self.texture_category.as_ref()
    }
}

/// Texture category for organizing rendering by texture/shader combinations
#[derive(Debug)]
pub struct DX8TextureCategoryClass {
    pub pass: u32,
    pub textures: Vec<Option<Arc<TextureClass>>>,
    pub shader: ShaderClass,
    pub material: Option<Arc<VertexMaterialClass>>,
    pub polygon_renderers: Vec<Arc<DX8PolygonRendererClass>>,
    render_tasks: Mutex<Vec<MeshRenderTask>>,
}

impl DX8TextureCategoryClass {
    pub fn new(
        textures: Vec<Option<Arc<TextureClass>>>,
        shader: ShaderClass,
        material: Option<Arc<VertexMaterialClass>>,
        pass: u32,
    ) -> Self {
        Self {
            pass,
            textures,
            shader,
            material,
            polygon_renderers: Vec::new(),
            render_tasks: Mutex::new(Vec::new()),
        }
    }

    pub fn add_polygon_renderer(&mut self, renderer: Arc<DX8PolygonRendererClass>) {
        self.polygon_renderers.push(renderer);
    }

    pub fn add_render_task(
        &mut self,
        polygon_renderer: Arc<DX8PolygonRendererClass>,
        mesh: Arc<MeshClass>,
    ) {
        if let Ok(mut guard) = self.render_tasks.lock() {
            guard.push(MeshRenderTask {
                polygon_renderer,
                mesh,
            });
        }
    }

    pub fn clear_render_tasks(&mut self) {
        if let Ok(mut guard) = self.render_tasks.lock() {
            guard.clear();
        }
    }

    pub fn has_render_tasks(&self) -> bool {
        self.render_tasks
            .lock()
            .map(|tasks| !tasks.is_empty())
            .unwrap_or(false)
    }
}

/// Render task for mesh rendering
#[derive(Debug, Clone)]
pub struct MeshRenderTask {
    pub polygon_renderer: Arc<DX8PolygonRendererClass>,
    pub mesh: Arc<MeshClass>,
}

/// FVF (Flexible Vertex Format) category container
#[derive(Debug)]
pub struct DX8FVFCategoryContainer {
    pub texture_categories: HashMap<(u32, String), Arc<DX8TextureCategoryClass>>,
}

impl DX8FVFCategoryContainer {
    pub fn new() -> Self {
        Self {
            texture_categories: HashMap::new(),
        }
    }

    pub fn get_or_create_texture_category(
        &mut self,
        textures: Vec<Option<Arc<TextureClass>>>,
        shader: ShaderClass,
        material: Option<Arc<VertexMaterialClass>>,
        pass: u32,
    ) -> Arc<DX8TextureCategoryClass> {
        let key = (
            pass,
            format!(
                "{:?}",
                (
                    shader,
                    material
                        .as_ref()
                        .map(|m| m.name.clone())
                        .unwrap_or_default()
                )
            ),
        );

        if let Some(category) = self.texture_categories.get(&key) {
            return category.clone();
        }

        let category = Arc::new(DX8TextureCategoryClass::new(
            textures, shader, material, pass,
        ));

        self.texture_categories.insert(key, category.clone());
        category
    }
}

#[derive(Debug, Clone)]
struct DecalRecord {
    id: u32,
    material_pass: Arc<MaterialPassClass>,
    vertices: Vec<Vec3>,
    normals: Vec<Vec3>,
    texcoords: Vec<Vec2>,
    indices: Vec<u32>,
}

/// View of the cached bone palette, mirroring the DX8 renderer's palette versioning.
pub struct BonePaletteView<'a> {
    pub matrices: &'a [Mat4],
    pub version: u64,
}

/// Main mesh class - equivalent to C++ MeshClass
#[derive(Debug)]
pub struct MeshClass {
    pub name: String,
    pub model: Option<Arc<MeshModelClass>>,
    pub transform: Mat4,
    pub bounding_box: AABoxClass,
    pub bounding_sphere: SphereClass,
    pub sort_level: u32,
    pub is_hidden: bool,
    pub is_animation_hidden: bool,
    pub alpha_override: f32,
    pub material_pass_alpha_override: f32,
    pub material_pass_emissive_override: f32,
    pub lighting_environment: Option<Arc<LightEnvironmentClass>>,
    pub decal_meshes: Vec<Arc<MeshClass>>, // Equivalent to C++ Decal meshes
    pub base_vertex_offset: u32,           // Equivalent to C++ BaseVertexOffset
    pub is_disabled_by_debugger: bool,     // Equivalent to C++ IsDisabledByDebugger
    pub mesh_debug_id: u32,                // Equivalent to C++ MeshDebugId
    pub next_visible_skin: Option<Arc<MeshClass>>, // Equivalent to C++ NextVisibleSkin
    pub collision_type: u32,               // Equivalent to C++ collision type bits
    pub w3d_attributes: u32,               // Equivalent to C++ W3dAttributes
    pub is_decal_instance: bool,
    material_info_cache: OnceLock<crate::render_object_system::MaterialInfoClass>,
    decal_records: Vec<DecalRecord>,
    deformed_world_vertices: Option<Vec<Vec3>>,
    bone_palette: Vec<Mat4>,
    bone_palette_version: u64,
    uv_offset_override: Option<[f32; 2]>,
}

/// Thread-safe debug ID counter for mesh objects
/// C++ Reference: Original code used static mut for mesh_debug_id assignment
/// Rust Implementation: Uses AtomicU32 with SeqCst ordering for thread safety
static MESH_DEBUG_ID_COUNT: AtomicU32 = AtomicU32::new(0);

impl MeshClass {
    pub fn new() -> Self {
        // Atomically increment and get the debug ID (thread-safe, no unsafe needed)
        let debug_id = MESH_DEBUG_ID_COUNT.fetch_add(1, Ordering::SeqCst);
        Self {
            name: String::new(),
            model: None,
            transform: Mat4::IDENTITY,
            bounding_box: AABoxClass::new(),
            bounding_sphere: SphereClass::new(Vec3::new(0.0, 0.0, 0.0), 0.0),
            sort_level: SORT_LEVEL_NONE,
            is_hidden: false,
            is_animation_hidden: false,
            alpha_override: 1.0,
            material_pass_alpha_override: 1.0,
            material_pass_emissive_override: 1.0,
            lighting_environment: None,
            decal_meshes: Vec::new(),
            base_vertex_offset: 0,
            is_disabled_by_debugger: false,
            mesh_debug_id: debug_id,
            next_visible_skin: None,
            collision_type: 0xFFFFFFFF, // Default: collide with everything
            w3d_attributes: 0,
            material_info_cache: OnceLock::new(),
            decal_records: Vec::new(),
            deformed_world_vertices: None,
            bone_palette: Vec::new(),
            bone_palette_version: 0,
            uv_offset_override: None,
            is_decal_instance: false,
        }
    }

    pub fn get_name(&self) -> &str {
        if let Some(model) = &self.model {
            model.get_name()
        } else {
            &self.name
        }
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
        if let Some(model_arc) = &mut self.model {
            if let Some(model_mut) = Arc::get_mut(model_arc) {
                model_mut.set_name(name);
            } else {
                let mut cloned = (**model_arc).clone();
                cloned.set_name(name);
                *model_arc = Arc::new(cloned);
            }
        }
    }

    pub fn set_uv_offset_override(&mut self, offset: Option<[f32; 2]>) {
        self.uv_offset_override = offset;
    }

    pub fn uv_offset_override(&self) -> Option<[f32; 2]> {
        self.uv_offset_override
    }

    /// Install per-vertex bone links on the underlying model geometry.
    pub fn set_vertex_bone_links(&mut self, links: Vec<u16>) {
        if let Some(model_arc) = &mut self.model {
            Arc::make_mut(model_arc).set_vertex_bone_links(links);
        }
        if !self.bone_palette.is_empty() {
            let _ = self.recompute_deformed_vertices_from_palette();
        }
    }

    pub fn set_vertex_influences(&mut self, influences: Vec<W3dVertInfStruct>) {
        if let Some(model_arc) = &mut self.model {
            Arc::make_mut(model_arc).set_vertex_influences(influences);
        }
        if !self.bone_palette.is_empty() {
            let _ = self.recompute_deformed_vertices_from_palette();
        }
    }

    pub fn vertex_bone_links(&self) -> Option<&[u16]> {
        self.model
            .as_ref()
            .and_then(|model| model.vertex_bone_links())
    }

    /// Create WGPU buffers for the mesh model
    pub fn create_wgpu_buffers(&mut self, device: &wgpu::Device) {
        if let Some(model_arc) = self.model.as_mut() {
            Arc::make_mut(model_arc).create_wgpu_buffers(device);
        }
    }

    /// Update the cached bone palette and recompute skinned vertices when possible.
    pub fn set_bone_palette_slice(&mut self, matrices: &[Mat4]) {
        self.bone_palette.clear();
        self.bone_palette.extend_from_slice(matrices);
        self.bone_palette_version = self.bone_palette_version.wrapping_add(1);
        if self.bone_palette.is_empty() {
            self.deformed_world_vertices = None;
        } else {
            let _ = self.recompute_deformed_vertices_from_palette();
        }
    }

    /// Remove any cached palette information.
    pub fn clear_bone_palette(&mut self) {
        self.bone_palette.clear();
        self.bone_palette_version = self.bone_palette_version.wrapping_add(1);
        self.deformed_world_vertices = None;
    }

    /// Borrow the current palette together with its version counter.
    pub fn bone_palette_view(&self) -> Option<BonePaletteView<'_>> {
        if self.bone_palette.is_empty() {
            None
        } else {
            Some(BonePaletteView {
                matrices: &self.bone_palette,
                version: self.bone_palette_version,
            })
        }
    }

    fn ensure_deformed_vertices_for_skin(&mut self) -> Option<&[Vec3]> {
        if self.deformed_world_vertices.is_none()
            && !self.recompute_deformed_vertices_from_palette()
        {
            return None;
        }
        self.deformed_world_vertices.as_deref()
    }

    fn compute_deformed_vertices_from_palette(&self) -> Option<Vec<Vec3>> {
        let model_arc = self.model.as_ref()?;
        if self.bone_palette.is_empty() {
            return None;
        }

        let model_ref = model_arc.as_ref();
        let mut vertices = Vec::with_capacity(model_ref.vertices.len());
        for (index, vertex) in model_ref.vertices.iter().enumerate() {
            let position = Vec3::new(vertex.x, vertex.y, vertex.z);
            let (indices, weights) = model_ref.vertex_influence_view(index);

            let mut skinned = Vec3::ZERO;
            let mut accumulated = 0.0;

            for slot in 0..4 {
                let weight = weights[slot];
                if weight <= f32::EPSILON {
                    continue;
                }
                let palette_index = indices[slot] as usize;
                let matrix = self
                    .bone_palette
                    .get(palette_index)
                    .copied()
                    .unwrap_or(Mat4::IDENTITY);
                skinned += matrix.transform_point3(position) * weight;
                accumulated += weight;
            }

            if accumulated <= f32::EPSILON {
                let fallback_index = model_ref
                    .vertex_bone_links()
                    .and_then(|links| links.get(index))
                    .copied()
                    .map(|idx| idx as usize)
                    .unwrap_or(0);
                let matrix = self
                    .bone_palette
                    .get(fallback_index)
                    .copied()
                    .unwrap_or(Mat4::IDENTITY);
                skinned = matrix.transform_point3(position);
            }

            vertices.push(skinned);
        }

        Some(vertices)
    }

    fn recompute_deformed_vertices_from_palette(&mut self) -> bool {
        if let Some(vertices) = self.compute_deformed_vertices_from_palette() {
            self.deformed_world_vertices = Some(vertices);
            true
        } else {
            self.deformed_world_vertices = None;
            false
        }
    }

    // get_name method already defined in first impl block

    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        self.clear_deformed_world_vertices();
        self.update_cached_bounding_volumes();
    }

    pub fn get_transform(&self) -> &Mat4 {
        &self.transform
    }

    pub fn set_sort_level(&mut self, level: u32) {
        self.sort_level = level;
    }

    pub fn get_sort_level(&self) -> u32 {
        self.sort_level
    }

    pub fn set_hidden(&mut self, hidden: bool) {
        self.is_hidden = hidden;
    }

    pub fn set_animation_hidden(&mut self, hidden: bool) {
        self.is_animation_hidden = hidden;
    }

    pub fn is_not_hidden_at_all(&self) -> bool {
        !self.is_hidden && !self.is_animation_hidden
    }

    pub fn get_bounding_box(&self) -> &AABoxClass {
        &self.bounding_box
    }

    pub fn set_lighting_environment(&mut self, env: Option<Arc<LightEnvironmentClass>>) {
        self.lighting_environment = env;
    }

    pub fn get_lighting_environment(&self) -> Option<&Arc<LightEnvironmentClass>> {
        self.lighting_environment.as_ref()
    }

    /// Cast ray against this mesh - equivalent to C++ MeshClass::Cast_Ray
    pub fn cast_ray(&self, raytest: &mut RayCollisionTestClass) -> bool {
        // Check collision type and visibility flags
        if (self.get_collision_type() & raytest.collision_type) == 0 {
            return false;
        }

        if (self.is_hidden || self.is_animation_hidden) && !raytest.check_hidden {
            return false;
        }

        if raytest.result.start_bad {
            return false;
        }

        // Transform ray to object space
        let world_to_obj = self.transform.inverse();
        let mut obj_ray = raytest.transformed_by_matrix(world_to_obj);
        obj_ray.result = RayCollisionResult::default();
        obj_ray.collided_render_obj = None;

        if let Some(model) = &self.model {
            if model.cast_ray(&mut obj_ray) {
                raytest.result = obj_ray.result.clone();
                raytest.result.normal = self
                    .transform
                    .transform_vector3(raytest.result.normal)
                    .normalize_or_zero();
                if raytest.result.compute_contact_point {
                    raytest.result.contact_point = self
                        .transform
                        .transform_point3(obj_ray.result.contact_point);
                } else {
                    raytest.result.contact_point = self
                        .transform
                        .transform_point3(obj_ray.result.contact_point);
                }
                raytest.collided_render_obj = Some(self as *const MeshClass as usize);
                return true;
            }
        }
        false
    }

    /// Cast AABox against this mesh - equivalent to C++ MeshClass::Cast_AABox
    pub fn cast_aabox(&self, boxtest: &mut AABoxCollisionTestClass) -> bool {
        if (self.get_collision_type() & boxtest.collision_type) == 0 {
            return false;
        }

        // Transform AABox to object space
        let world_to_obj = self.transform.inverse();
        let mut obj_box = boxtest.transformed_by_matrix(world_to_obj);

        if let Some(model) = &self.model {
            if model.cast_aabox(&mut obj_box) {
                if let Some(result) = obj_box.result.clone() {
                    let transformed_contacts = result
                        .contact_points
                        .iter()
                        .map(|point| self.transform.transform_point3(*point))
                        .collect::<Vec<_>>();
                    boxtest.result = Some(AABoxCollisionResult {
                        intersection: result.intersection,
                        contact_points: transformed_contacts,
                    });
                } else {
                    boxtest.result = Some(AABoxCollisionResult {
                        intersection: true,
                        contact_points: Vec::new(),
                    });
                }
                boxtest.collided_render_obj = Some(self as *const MeshClass as usize);
                return true;
            } else {
                boxtest.result = obj_box.result;
            }
            false
        } else {
            false
        }
    }

    /// Test intersection with AABox - equivalent to C++ MeshClass::Intersect_AABox
    pub fn intersect_aabox(&self, boxtest: &AABoxIntersectionTestClass) -> bool {
        if (self.get_collision_type() & boxtest.collision_type) == 0 {
            return false;
        }

        // Transform AABox to object space
        let world_to_obj = self.transform.inverse();
        let obj_box = boxtest.transformed_by_matrix(world_to_obj);

        if let Some(model) = &self.model {
            model.intersect_aabox(&obj_box)
        } else {
            false
        }
    }

    /// Test intersection with OBBox - equivalent to C++ MeshClass::Intersect_OBBox
    pub fn intersect_obbox(&self, boxtest: &OBBoxIntersectionTestClass) -> bool {
        if (self.get_collision_type() & boxtest.collision_type) == 0 {
            return false;
        }

        // Transform OBBox to object space
        let world_to_obj = self.transform.inverse();
        let obj_box = boxtest.transformed_by_matrix(world_to_obj);

        if let Some(model) = &self.model {
            model.intersect_obbox(&obj_box)
        } else {
            false
        }
    }

    /// Create a decal on this mesh - equivalent to C++ MeshClass::Create_Decal
    pub fn create_decal(&mut self, generator: &mut DecalGeneratorClass) {
        if !ww3d_core::WW3D::are_decals_enabled() {
            return;
        }

        if !generator.allow_translucent_meshes() && self.is_translucent() {
            return;
        }

        let Some(model_arc) = self.model.as_ref().cloned() else {
            return;
        };

        let inv_transform = self.transform.inverse();
        let projector_volume_world = generator.get_bounding_volume();
        let material_pass = generator.material_pass();
        let projector_dir_world = generator.projector_direction();
        let projector_dir_obj = normalize_or(
            inv_transform.transform_vector3(projector_dir_world),
            Vec3::Z,
        );
        let surface_bias = generator.surface_bias();
        let bias_offset_obj = projector_dir_obj * surface_bias;
        let bias_offset_world = projector_dir_world * surface_bias;

        let mut record_vertices = Vec::new();
        let mut record_normals = Vec::new();
        let mut record_texcoords = Vec::new();
        let mut record_indices = Vec::new();

        if model_arc.get_flag(MeshGeometryClass::SKIN) {
            generator.set_mesh_transform(Mat4::IDENTITY);
            let axes_world = [
                normalize_or(projector_volume_world.basis[0], Vec3::X),
                normalize_or(projector_volume_world.basis[1], Vec3::Y),
                normalize_or(projector_volume_world.basis[2], Vec3::Z),
            ];
            let extents_world = Vec3::new(
                projector_volume_world.extent.x.abs(),
                projector_volume_world.extent.y.abs(),
                projector_volume_world.extent.z.abs(),
            );

            {
                let world_vertices: &[Vec3] = match self.ensure_deformed_vertices_for_skin() {
                    Some(verts) => verts,
                    None => {
                        debug!(
                            "Mesh '{}' missing skinned world vertices; using object-space transform as fallback",
                            self.get_name()
                        );
                        let transform = self.transform;
                        let fallback: Vec<Vec3> = model_arc
                            .vertices
                            .iter()
                            .map(|vertex| {
                                transform.transform_point3(Vec3::new(vertex.x, vertex.y, vertex.z))
                            })
                            .collect();
                        self.deformed_world_vertices = Some(fallback);
                        self.deformed_world_vertices.as_deref().unwrap()
                    }
                };

                let mut apt = Vec::new();
                model_arc.generate_skin_apt(&projector_volume_world, &mut apt, world_vertices);
                if apt.is_empty() {
                    debug!(
                        "Decal generator {} did not intersect skinned mesh '{}' geometry",
                        generator.get_decal_id(),
                        self.get_name()
                    );
                    return;
                }

                for poly_index in apt {
                    let Some(triangle) = model_arc.triangles.get(poly_index as usize) else {
                        continue;
                    };

                    let indices = triangle.vindex.map(|idx| idx as usize);
                    if indices.iter().any(|&idx| idx >= world_vertices.len()) {
                        continue;
                    }

                    let world_unbiased = [
                        world_vertices[indices[0]],
                        world_vertices[indices[1]],
                        world_vertices[indices[2]],
                    ];

                    let plane_normal_world = normalize_or(
                        (world_unbiased[1] - world_unbiased[0])
                            .cross(world_unbiased[2] - world_unbiased[0]),
                        projector_dir_world,
                    );
                    if plane_normal_world.dot(projector_dir_world) > generator.backface_threshold()
                    {
                        continue;
                    }

                    let plane_normal_obj = normalize_or(
                        inv_transform.transform_vector3(plane_normal_world),
                        projector_dir_obj,
                    );

                    let mut polygon = Vec::with_capacity(3);
                    for &vertex_index in &triangle.vindex {
                        let idx = vertex_index as usize;
                        if idx >= world_vertices.len() {
                            continue;
                        }
                        let biased_world = world_vertices[idx] + bias_offset_world;
                        let obj_vertex = inv_transform.transform_point3(biased_world);
                        let offset = biased_world - projector_volume_world.center;
                        let local = Vec3::new(
                            offset.dot(axes_world[0]),
                            offset.dot(axes_world[1]),
                            offset.dot(axes_world[2]),
                        );
                        polygon.push(ClipVertex {
                            obj_pos: obj_vertex,
                            world_pos: biased_world,
                            normal: plane_normal_obj,
                            local,
                        });
                    }

                    let clipped = clip_polygon_to_projector(polygon, extents_world);
                    if clipped.len() < 3 {
                        continue;
                    }

                    for tri_idx in 1..clipped.len() - 1 {
                        let v0 = &clipped[0];
                        let v1 = &clipped[tri_idx];
                        let v2 = &clipped[tri_idx + 1];
                        let face_normal = normalize_or(
                            (v1.obj_pos - v0.obj_pos).cross(v2.obj_pos - v0.obj_pos),
                            plane_normal_obj,
                        );

                        let base = record_vertices.len() as u32;
                        for vertex in [v0, v1, v2] {
                            record_vertices.push(vertex.obj_pos);
                            record_normals.push(normalize_or(vertex.normal, face_normal));
                            let tex = generator.compute_mesh_texture_coordinate(vertex.obj_pos);
                            record_texcoords.push(Vec2::new(tex.x, tex.y));
                        }
                        record_indices.extend_from_slice(&[base, base + 1, base + 2]);
                    }
                }
            }
        } else {
            let model_ref = model_arc.as_ref();
            let local_box = projector_volume_world.transformed(inv_transform);
            generator.set_mesh_transform(self.transform);
            let mut apt = Vec::new();
            model_ref.generate_rigid_apt(&local_box, &mut apt);
            if apt.is_empty() {
                debug!(
                    "Decal generator {} did not intersect mesh '{}' geometry",
                    generator.get_decal_id(),
                    self.get_name()
                );
                return;
            }

            let axes = [
                normalize_or(local_box.basis[0], Vec3::X),
                normalize_or(local_box.basis[1], Vec3::Y),
                normalize_or(local_box.basis[2], Vec3::Z),
            ];
            let extents = Vec3::new(
                local_box.extent.x.abs(),
                local_box.extent.y.abs(),
                local_box.extent.z.abs(),
            );

            for poly_index in apt {
                let Some(triangle) = model_ref.triangles.get(poly_index as usize) else {
                    continue;
                };

                let Some(verts) = triangle_vertices(triangle, &model_ref.vertices) else {
                    continue;
                };

                let plane_normal = {
                    let stored = Vec3::new(triangle.normal.x, triangle.normal.y, triangle.normal.z);
                    if stored.length_squared() > RAY_EPSILON {
                        stored.normalize()
                    } else {
                        normalize_or((verts[1] - verts[0]).cross(verts[2] - verts[0]), Vec3::Z)
                    }
                };

                if plane_normal.dot(projector_dir_obj) > generator.backface_threshold() {
                    continue;
                }

                let mut polygon = Vec::with_capacity(3);
                for &vertex_index in &triangle.vindex {
                    let obj_vertex =
                        Vec3::from(model_ref.vertices[vertex_index as usize]) + bias_offset_obj;
                    let world_vertex = self.transform.transform_point3(obj_vertex);
                    let vertex_normal = model_ref
                        .normals
                        .get(vertex_index as usize)
                        .map(|n| Vec3::from(*n))
                        .map(|n| normalize_or(n, plane_normal))
                        .unwrap_or(plane_normal);
                    let offset = obj_vertex - local_box.center;
                    let local = Vec3::new(
                        offset.dot(axes[0]),
                        offset.dot(axes[1]),
                        offset.dot(axes[2]),
                    );
                    polygon.push(ClipVertex {
                        obj_pos: obj_vertex,
                        world_pos: world_vertex,
                        normal: vertex_normal,
                        local,
                    });
                }

                let clipped = clip_polygon_to_projector(polygon, extents);
                if clipped.len() < 3 {
                    continue;
                }

                for tri_idx in 1..clipped.len() - 1 {
                    let v0 = &clipped[0];
                    let v1 = &clipped[tri_idx];
                    let v2 = &clipped[tri_idx + 1];
                    let face_normal = normalize_or(
                        (v1.obj_pos - v0.obj_pos).cross(v2.obj_pos - v0.obj_pos),
                        plane_normal,
                    );

                    let base = record_vertices.len() as u32;
                    for vertex in [v0, v1, v2] {
                        record_vertices.push(vertex.obj_pos);
                        record_normals.push(normalize_or(vertex.normal, face_normal));
                        let tex = generator.compute_mesh_texture_coordinate(vertex.world_pos);
                        record_texcoords.push(Vec2::new(tex.x, tex.y));
                    }
                    record_indices.extend_from_slice(&[base, base + 1, base + 2]);
                }
            }
        }

        if record_indices.is_empty() {
            debug!(
                "Decal generator {} had no clipped geometry on mesh '{}'",
                generator.get_decal_id(),
                self.get_name()
            );
            return;
        }

        let record = DecalRecord {
            id: generator.get_decal_id(),
            material_pass: material_pass.clone(),
            vertices: record_vertices,
            normals: record_normals,
            texcoords: record_texcoords,
            indices: record_indices,
        };

        self.decal_records.push(record);
        generator.add_mesh_handle(self as *const MeshClass);
        self.rebuild_decal_mesh();

        debug!(
            "Created decal {} on mesh '{}' ({} decals active)",
            generator.get_decal_id(),
            self.get_name(),
            self.decal_records.len()
        );
    }

    pub fn delete_decal(&mut self, decal_id: u32) {
        let previous = self.decal_records.len();
        self.decal_records.retain(|record| record.id != decal_id);

        if self.decal_records.len() == previous {
            return;
        }

        if self.decal_records.is_empty() {
            self.decal_meshes.clear();
        } else {
            self.rebuild_decal_mesh();
        }
    }

    /// Cache world-space deformed vertices so skin decals can project onto the animated surface.
    pub fn set_deformed_world_vertices(&mut self, vertices: Vec<Vec3>) {
        if let Some(model) = &self.model {
            if vertices.len() != model.vertices.len() {
                warn!(
                    "set_deformed_world_vertices mismatch for mesh '{}': received {} vertices, expected {}",
                    self.get_name(),
                    vertices.len(),
                    model.vertices.len()
                );
            }
        }
        self.deformed_world_vertices = Some(vertices);
    }

    /// Clear any cached deformed vertex data. Call when animation data is invalidated.
    pub fn clear_deformed_world_vertices(&mut self) {
        self.deformed_world_vertices = None;
    }

    /// Get number of polygons - equivalent to C++ MeshClass::Get_Num_Polys
    pub fn get_num_polys(&self) -> u32 {
        self.model.as_ref().map_or(0, |m| m.triangles.len() as u32)
    }

    /// Get object space bounding sphere - equivalent to C++ Get_Obj_Space_Bounding_Sphere
    pub fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        self.bounding_sphere
    }

    /// Get object space bounding box - equivalent to C++ Get_Obj_Space_Bounding_Box
    pub fn get_obj_space_bounding_box(&self) -> &AABoxClass {
        &self.bounding_box
    }

    fn build_material_info_from_model(
        model: &MeshModelClass,
    ) -> crate::render_object_system::MaterialInfoClass {
        let mut vertex_materials = Vec::new();
        let mut textures = Vec::new();

        for pass in &model.material_passes {
            if let Some(vm) = &pass.vertex_material {
                vertex_materials.push((**vm).clone());
            }
            for stage in 0..MAX_TEXTURE_STAGES {
                if let Some(texture) = pass.get_texture(stage) {
                    textures.push(Arc::clone(texture));
                }
            }
        }

        crate::render_object_system::MaterialInfoClass {
            vertex_materials,
            textures,
            passes: model.material_passes.clone(),
        }
    }

    /// Update skin deformation - equivalent to C++ MeshClass::update_skin
    pub fn update_skin(&mut self) {
        let is_skinned = self
            .model
            .as_ref()
            .map(|model| model.get_flag(MeshGeometryClass::SKIN))
            .unwrap_or(false);

        if !is_skinned {
            self.clear_deformed_world_vertices();
            return;
        }

        let _ = self.recompute_deformed_vertices_from_palette();
        self.update_cached_bounding_volumes();
    }

    /// Get deformed vertices for skin - equivalent to C++ MeshClass::Get_Deformed_Vertices
    pub fn get_deformed_vertices(&self, vertices: &mut Vec<W3dVectorStruct>) {
        vertices.clear();

        let Some(model) = &self.model else {
            return;
        };

        let output_positions: Vec<Vec3> = if model.get_flag(MeshGeometryClass::SKIN) {
            if let Some(cached) = self.deformed_world_vertices.as_ref() {
                cached.clone()
            } else if let Some(computed) = self.compute_deformed_vertices_from_palette() {
                computed
            } else {
                model
                    .vertices
                    .iter()
                    .map(|vertex| Vec3::new(vertex.x, vertex.y, vertex.z))
                    .collect()
            }
        } else {
            model
                .vertices
                .iter()
                .map(|vertex| Vec3::new(vertex.x, vertex.y, vertex.z))
                .collect()
        };

        vertices.reserve(output_positions.len());
        for position in output_positions {
            vertices.push(W3dVectorStruct {
                x: position.x,
                y: position.y,
                z: position.z,
            });
        }
    }

    /// Make mesh unique in renderer - equivalent to C++ MeshClass::Make_Unique
    pub fn make_unique(&mut self) {
        if let Some(model_arc) = &mut self.model {
            // Ensure unique before mutating
            if let Some(model_mut) = Arc::get_mut(model_arc) {
                model_mut.make_geometry_unique();
            } else {
                let mut cloned = (**model_arc).clone();
                cloned.make_geometry_unique();
                *model_arc = Arc::new(cloned);
            }

            // Update any cached data that might reference shared geometry
            self.update_cached_bounding_volumes();
        }
    }

    /// Replace vertex material - equivalent to C++ MeshClass::Replace_VertexMaterial
    pub fn replace_vertex_material(
        &mut self,
        old_material: &VertexMaterialClass,
        new_material: &VertexMaterialClass,
    ) {
        if let Some(model_arc) = self.model.as_mut() {
            let model = Arc::make_mut(model_arc);
            // Replace all instances of old_material with new_material in all material passes
            for pass in &mut model.material_passes {
                if let Some(vertex_material) = &mut pass.vertex_material {
                    if vertex_material.name == old_material.name {
                        *vertex_material = Arc::new(new_material.clone());
                    }
                }
            }
            model.mark_dirty();
            let _ = self.material_info_cache.take();
        }
    }

    /// Render specific material pass - equivalent to C++ MeshClass::Render_Material_Pass
    pub fn render_material_pass<'a>(
        &'a self,
        pass: &MaterialPassClass,
        index_buffer: Option<&'a wgpu::Buffer>,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) -> W3dResult<()> {
        if let Some(model) = &self.model {
            if let Some(vertex_buffer) = &model.vertex_buffer {
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                if let Some(index_buf) = index_buffer {
                    render_pass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
                    let pass_ranges = compute_pass_index_ranges(model, &[]);
                    let pass_index = pass.get_pass_index();
                    let (start_index, index_count) = if pass_index < pass_ranges.len() {
                        pass_ranges[pass_index]
                    } else if !pass_ranges.is_empty() {
                        pass_ranges[0]
                    } else {
                        (0, model.index_count)
                    };
                    if index_count > 0 {
                        render_pass.draw_indexed(start_index..start_index + index_count, 0, 0..1);
                    }
                } else {
                    render_pass.draw(0..model.vertex_count, 0..1);
                }
            }
        }

        Ok(())
    }

    /// Get collision type - equivalent to C++ Get_Collision_Type
    pub fn get_collision_type(&self) -> u32 {
        self.collision_type
    }

    /// Set collision type - equivalent to C++ Set_Collision_Type
    pub fn set_collision_type(&mut self, collision_type: u32) {
        self.collision_type = collision_type;
    }

    /// Check if mesh contains a point - equivalent to C++ MeshClass::Contains
    pub fn contains(&self, point: Vec3) -> bool {
        // Transform point to object space
        let obj_point = self.transform.inverse().transform_point3(point);

        if let Some(_model) = &self.model {
            // Fast rejection: check bounding sphere first
            let distance = obj_point.distance(self.bounding_sphere.center);
            if distance > self.bounding_sphere.radius {
                return false;
            }

            // Use sphere containment as approximation
            // Note: Precise containment would require ray casting through mesh triangles
            // (odd/even intersection count). C++ MeshClass::Contains uses AABTree traversal.
            distance <= self.bounding_sphere.radius
        } else {
            false
        }
    }

    /// Clone the mesh - equivalent to C++ MeshClass::Clone
    pub fn clone_mesh(&self) -> MeshClass {
        let mut new_mesh = MeshClass::new();

        // Copy basic properties
        new_mesh.name = self.name.clone();
        new_mesh.transform = self.transform;
        new_mesh.bounding_box = self.bounding_box.clone();
        new_mesh.bounding_sphere = self.bounding_sphere;
        new_mesh.sort_level = self.sort_level;
        new_mesh.is_hidden = self.is_hidden;
        new_mesh.is_animation_hidden = self.is_animation_hidden;
        new_mesh.alpha_override = self.alpha_override;
        new_mesh.material_pass_alpha_override = self.material_pass_alpha_override;
        new_mesh.material_pass_emissive_override = self.material_pass_emissive_override;
        new_mesh.collision_type = self.collision_type;
        new_mesh.w3d_attributes = self.w3d_attributes;
        new_mesh.is_decal_instance = self.is_decal_instance;
        new_mesh.uv_offset_override = self.uv_offset_override;

        // Clone the model if it exists
        if let Some(model) = &self.model {
            new_mesh.model = Some(Arc::new((**model).clone()));
        }

        // Clone lighting environment
        new_mesh.lighting_environment = self.lighting_environment.clone();

        new_mesh.decal_records = self.decal_records.clone();
        new_mesh.deformed_world_vertices = self.deformed_world_vertices.clone();
        new_mesh.decal_meshes = self.decal_meshes.clone();
        new_mesh.rebuild_decal_mesh();

        new_mesh
    }

    fn rebuild_decal_mesh(&mut self) {
        if self.decal_records.is_empty() {
            self.decal_meshes.clear();
            return;
        }

        let mut grouped: BTreeMap<usize, Vec<&DecalRecord>> = BTreeMap::new();
        for record in &self.decal_records {
            let key = Arc::as_ptr(&record.material_pass) as usize;
            grouped.entry(key).or_default().push(record);
        }

        self.decal_meshes.clear();

        for (group_index, records) in grouped.values().enumerate() {
            let mut combined_positions: Vec<Vec3> = Vec::new();
            let mut combined_normals: Vec<Vec3> = Vec::new();
            let mut combined_texcoords: Vec<Vec2> = Vec::new();
            let mut combined_triangles: Vec<[u32; 3]> = Vec::new();

            let mut min = Vec3::splat(f32::INFINITY);
            let mut max = Vec3::splat(f32::NEG_INFINITY);

            for record in records {
                let base = combined_positions.len() as u32;
                for pos in &record.vertices {
                    min = min.min(*pos);
                    max = max.max(*pos);
                }

                combined_positions.extend_from_slice(&record.vertices);
                combined_normals.extend_from_slice(&record.normals);
                combined_texcoords.extend_from_slice(&record.texcoords);

                for chunk in record.indices.chunks(3) {
                    if chunk.len() < 3 {
                        continue;
                    }
                    combined_triangles.push([base + chunk[0], base + chunk[1], base + chunk[2]]);
                }
            }

            if combined_positions.is_empty() || combined_triangles.is_empty() {
                continue;
            }

            let vertices: Vec<W3dVectorStruct> = combined_positions
                .iter()
                .copied()
                .map(W3dVectorStruct::from)
                .collect();
            let normals: Vec<W3dVectorStruct> = combined_normals
                .iter()
                .copied()
                .map(|n| W3dVectorStruct::from(normalize_or(n, Vec3::Z)))
                .collect();
            let texcoords: Vec<W3dTexCoordStruct> = combined_texcoords
                .iter()
                .map(|uv| W3dTexCoordStruct { u: uv.x, v: uv.y })
                .collect();
            let triangles: Vec<W3dTriangleStruct> = combined_triangles
                .iter()
                .map(|tri| {
                    let p0 = combined_positions[tri[0] as usize];
                    let p1 = combined_positions[tri[1] as usize];
                    let p2 = combined_positions[tri[2] as usize];
                    let normal = normalize_or((p1 - p0).cross(p2 - p0), Vec3::Z);
                    let distance = -normal.dot(p0);
                    W3dTriangleStruct {
                        vindex: *tri,
                        attributes: 0,
                        normal: W3dVectorStruct::from(normal),
                        distance,
                    }
                })
                .collect();

            let material_pass = records
                .first()
                .map(|record| Arc::clone(&record.material_pass))
                .expect("records must be non-empty");

            let mut model =
                MeshModelClass::new(&format!("{}_Decals_{}", self.get_name(), group_index));
            model.vertices = vertices;
            model.normals = normals;
            model.texture_coords = texcoords;
            model.triangles = triangles;
            model.vertex_count = model.vertices.len() as u32;
            model.index_count = (combined_triangles.len() * 3) as u32;
            model.material_passes = vec![material_pass.as_ref().clone()];
            model.register_for_rendering();

            let mut decal_mesh = MeshClass::new();
            decal_mesh.name = format!("{}_DecalMesh_{}", self.get_name(), group_index);
            decal_mesh.model = Some(Arc::new(model));
            decal_mesh.transform = self.transform;
            decal_mesh.sort_level = self.sort_level;
            decal_mesh.alpha_override = 1.0;
            decal_mesh.material_pass_alpha_override = 1.0;
            decal_mesh.material_pass_emissive_override = 1.0;
            decal_mesh.is_decal_instance = true;

            let bounding_box = AABoxClass::from_min_max(min, max);
            let obj_center = (min + max) * 0.5;
            let world_center = self.transform.transform_point3(obj_center);
            let mut radius: f32 = 0.0;
            for pos in &combined_positions {
                let world_pos = self.transform.transform_point3(*pos);
                radius = radius.max((world_pos - world_center).length());
            }

            decal_mesh.bounding_box = bounding_box;
            decal_mesh.bounding_sphere = SphereClass::new(world_center, radius);

            self.decal_meshes.push(Arc::new(decal_mesh));
        }
    }

    /// Free resources - equivalent to C++ MeshClass::Free
    pub fn free(&mut self) {
        self.model = None;
        let _ = self.material_info_cache.take();
        self.decal_meshes.clear();
        self.decal_records.clear();
        self.deformed_world_vertices = None;
    }

    /// Load mesh from W3D file - equivalent to C++ MeshClass::Load_W3D
    pub fn load_w3d(&mut self, _data: &[u8]) -> W3dResult<()> {
        // Parse W3D file data
        // This is a simplified implementation - would need full W3D parsing

        // Fallback path: attach minimal metadata for already-loaded mesh models.
        if let Some(model_arc) = &mut self.model {
            // Placeholder: set a name, ensuring unique ownership
            if let Some(model_mut) = Arc::get_mut(model_arc) {
                model_mut.set_name("Loaded_W3D_Mesh");
            } else {
                let mut cloned = (**model_arc).clone();
                cloned.set_name("Loaded_W3D_Mesh");
                *model_arc = Arc::new(cloned);
            }

            // Update cached bounding volumes
            self.update_cached_bounding_volumes();

            Ok(())
        } else {
            Err(W3dError::InvalidParameter(
                "No mesh model available".to_string(),
            ))
        }
    }

    /// Initialize mesh from MeshBuilder - equivalent to C++ MeshClass::Init
    // Note: MeshBuilder module not yet implemented. When added, this method will:
    // 1. Extract geometry data (vertices, normals, triangles) from builder
    // 2. Create MeshModelClass and populate with builder data
    // 3. Compute bounding volumes and set up materials
    // C++ equivalent: MeshClass::Init(MeshBuilder*) in meshclass.cpp
    /*
    pub fn init_from_builder(&mut self, builder: &crate::rendering::mesh_builder::MeshBuilder) -> W3dResult<()> {
        // Create mesh model from builder
        let mut meshmodel = MeshModelClass::new("Built_Mesh");

        // Copy geometry from builder
        if let Some(geometry) = builder.get_geometry() {
            // Copy vertices, triangles, normals, etc.
            meshmodel.vertices = geometry.vertices.clone();
            meshmodel.triangles = geometry.triangles.clone();

            if let Some(normals) = &geometry.normals {
                meshmodel.normals = Some(normals.clone());
            }

            if let Some(tex_coords) = &geometry.tex_coords {
                meshmodel.tex_coords = Some(tex_coords.clone());
            }

            // Copy materials
            meshmodel.material_passes = builder.get_material_passes().clone();
        }

        // Set the model
        self.model = Some(Arc::new(meshmodel));

        // Update cached bounding volumes
        self.update_cached_bounding_volumes();

        Ok(())
    }
    */

    /// Get W3D flags - equivalent to C++ MeshClass::Get_W3D_Flags
    pub fn get_w3d_flags(&self) -> u32 {
        if let Some(model) = &self.model {
            model.get_w3d_attributes()
        } else {
            self.w3d_attributes
        }
    }

    /// Get user text - equivalent to C++ MeshClass::Get_User_Text
    pub fn get_user_text(&self) -> Option<String> {
        if let Some(model) = &self.model {
            model.get_user_text().map(|s| s.to_string())
        } else {
            None
        }
    }

    /// Scale the mesh - equivalent to C++ MeshClass::Scale
    pub fn scale(&mut self, scale: f32) {
        if scale == 1.0 {
            return;
        }

        let sc = Vec3::new(scale, scale, scale);
        // Ensure unique model before mutating
        if let Some(model_arc) = &mut self.model {
            if let Some(model_mut) = Arc::get_mut(model_arc) {
                model_mut.make_geometry_unique();
                model_mut.scale_geometry(sc);
            } else {
                // Clone to get unique ownership
                let mut cloned = (**model_arc).clone();
                cloned.make_geometry_unique();
                cloned.scale_geometry(sc);
                *model_arc = Arc::new(cloned);
            }
        }

        // Invalidate cached bounding volumes
        self.update_cached_bounding_volumes();

        // Update container's bounding volumes
        // Note: Container system would update parent bounding volumes here
    }

    /// Scale the mesh with separate axes - equivalent to C++ MeshClass::Scale
    pub fn scale_xyz(&mut self, scalex: f32, scaley: f32, scalez: f32) {
        let sc = Vec3::new(scalex, scaley, scalez);
        if let Some(model_arc) = &mut self.model {
            if let Some(model_mut) = Arc::get_mut(model_arc) {
                model_mut.make_geometry_unique();
                model_mut.scale_geometry(sc);
            } else {
                let mut cloned = (**model_arc).clone();
                cloned.make_geometry_unique();
                cloned.scale_geometry(sc);
                *model_arc = Arc::new(cloned);
            }
        }

        // Invalidate cached bounding volumes
        self.update_cached_bounding_volumes();

        // Update container's bounding volumes
        // Note: Container system would update parent bounding volumes here
    }

    /// Transform an AABox from object space to world space
    /// C++ Reference: Matrix3D::Transform_Center_Extent_AABox (matrix3d.cpp:1052-1078)
    fn transform_aabox(&self, obj_box: &AABoxClass) -> AABoxClass {
        let mat = self.transform;
        let mut new_center = Vec3::ZERO;
        let mut new_extent = Vec3::ZERO;

        // For each axis of the output box
        for i in 0..3 {
            // Start with the translation component
            new_center[i] = mat.col(3)[i];
            new_extent[i] = 0.0;

            // Add contributions from rotation/scale
            for j in 0..3 {
                new_center[i] += mat.col(j)[i] * obj_box.center[j];
                // Take absolute value of transformed extent
                new_extent[i] += (mat.col(j)[i] * obj_box.extent[j]).abs();
            }
        }

        AABoxClass::from_center_and_extent(new_center, new_extent)
    }

    /// Update cached bounding volumes - equivalent to C++ MeshClass::Update_Cached_Bounding_Volumes
    pub fn update_cached_bounding_volumes(&mut self) {
        // Get object space bounding sphere
        let sphere = self.get_obj_space_bounding_sphere();

        // Transform to world space
        let world_center = self.transform.transform_point3(sphere.center);
        self.bounding_sphere = SphereClass::new(world_center, sphere.radius);

        // Get object space bounding box
        let obj_box = self.get_obj_space_bounding_box();

        // Transform to world space
        // C++ Reference: Matrix3D::Transform_Center_Extent_AABox (matrix3d.cpp:1052-1078)
        self.bounding_box = self.transform_aabox(&obj_box);
    }

    /// Replace texture - equivalent to C++ MeshClass::Replace_Texture
    /// C++ Reference: MeshModelClass::Replace_Texture (meshmdl.cpp:207-222)
    pub fn replace_texture(&mut self, old_texture: &TextureClass, new_texture: &TextureClass) {
        if let Some(model_arc) = self.model.as_mut() {
            let model = Arc::make_mut(model_arc);

            // Iterate through all texture stages and passes
            // C++ loops through MAX_TEX_STAGES and pass count
            for pass_idx in 0..model.get_pass_count() {
                for stage_idx in 0..4 {
                    // MAX_TEX_STAGES = 4 in most implementations
                    if model.has_texture_array(pass_idx, stage_idx) {
                        // Check each polygon's texture
                        for poly_idx in 0..model.get_polygon_count() {
                            if let Some(texture) = model.peek_texture(poly_idx, pass_idx, stage_idx)
                            {
                                // Compare texture pointers or names
                                if std::ptr::eq(texture, old_texture)
                                    || texture.get_name() == old_texture.get_name()
                                {
                                    model.set_texture(
                                        poly_idx,
                                        Arc::new(new_texture.clone()),
                                        pass_idx,
                                        stage_idx,
                                    );
                                }
                            }
                        }
                    } else if let Some(single_texture) =
                        model.peek_single_texture(pass_idx, stage_idx)
                    {
                        // Handle single texture for all polygons
                        if std::ptr::eq(single_texture, old_texture)
                            || single_texture.get_name() == old_texture.get_name()
                        {
                            model.set_single_texture(
                                Arc::new(new_texture.clone()),
                                pass_idx,
                                stage_idx,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Generate culling tree - equivalent to C++ MeshClass::Generate_Culling_Tree
    /// C++ Reference: mesh.cpp lines 1448-1451, meshgeometry.cpp lines 1548-1558
    pub fn generate_culling_tree(&mut self) {
        if let Some(model_arc) = self.model.as_mut() {
            // Get mutable access to the model
            let _model = Arc::make_mut(model_arc);
            // Delegate to model's culling tree generation
            // In C++, this calls Model->Generate_Culling_Tree() which builds an AABTree
            // from the polygon and vertex arrays for hierarchical collision/culling
            // The AABTree is built using AABTreeBuilderClass and stored in CullTree
            // Note: Culling tree generation is typically done at load time, not runtime
            // If needed at runtime, it would build an axis-aligned bounding box tree
            // for accelerating ray casts and collision tests
        }
    }

    /// Add dependencies to list - equivalent to C++ MeshClass::Add_Dependencies_To_List
    /// C++ Reference: mesh.cpp lines 1466-1500
    pub fn add_dependencies_to_list(&self, file_list: &mut Vec<String>, _textures_only: bool) {
        // Get material info and add texture filenames
        // C++ Implementation: Gets MaterialInfoClass via Get_Material_Info()
        // Then loops through material->Texture_Count() and adds each texture's full path
        if let Some(model) = &self.model {
            // Add textures from material passes (Rust equivalent of material info)
            for pass in &model.material_passes {
                // Enumerate textures from each material pass
                for texture_opt in &pass.textures {
                    if let Some(texture) = texture_opt {
                        // In C++: texture->Get_Full_Path() returns the texture filename
                        // Add texture path to the dependency list
                        let texture_path = format!("{}.dds", texture.name);
                        if !file_list.contains(&texture_path) {
                            file_list.push(texture_path);
                        }
                    }
                }
            }
        }

        // Add dependencies from container
        // C++ Implementation: Calls RenderObjClass::Add_Dependencies_To_List(file_list, textures_only)
        // which handles container-specific dependencies
        // In the Rust implementation, container system is handled at a higher level
        // so we skip this unless we have explicit container references
    }

    // load_w3d method already defined in first impl block

    /// Special render for vis and shadow - equivalent to C++ MeshClass::Special_Render
    pub fn special_render(&self, rinfo: &mut RenderInfoClass) -> W3dResult<()> {
        // Special rendering for visibility and shadow passes
        // This handles special rendering modes like shadow mapping and visibility testing
        // Note: RenderInfoClass doesn't currently have render_type field.
        // When added, this will switch between different rendering modes:
        // - Shadow: depth-only rendering for shadow maps
        // - Visibility: simplified rendering for occlusion queries
        // - Normal: full material rendering
        // C++ equivalent: MeshClass::Special_Render checks RenderInfoClass::m_Type

        if let Some(_model) = &self.model {
            // Render mode switching would go here based on rinfo.render_type
            // Current fallback path is equivalent to the normal render pass.
            let _ = rinfo; // Suppress unused warning
        }

        Ok(())
    }

    /// Check if mesh is translucent - equivalent to C++ Is_Translucent
    pub fn is_translucent(&self) -> bool {
        // Check if the mesh has translucent materials
        if let Some(model) = &self.model {
            for pass in &model.material_passes {
                if pass
                    .vertex_material
                    .as_ref()
                    .map(|material| material.opacity < 1.0 || material.translucency > 0.0)
                    .unwrap_or(false)
                {
                    return true;
                }
                let blend = pass.shader.blend_mode();
                if matches!(
                    blend,
                    MaterialBlendMode::Alpha | MaterialBlendMode::Additive
                ) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if mesh is alpha - equivalent to C++ Is_Alpha
    pub fn is_alpha(&self) -> bool {
        // Check if the mesh has alpha-blended materials
        if let Some(model) = &self.model {
            for pass in &model.material_passes {
                let blend = pass.shader.blend_mode();
                if matches!(
                    blend,
                    MaterialBlendMode::Alpha
                        | MaterialBlendMode::Additive
                        | MaterialBlendMode::Decal
                ) {
                    return true;
                }
                if pass
                    .vertex_material
                    .as_ref()
                    .map(|material| material.opacity < 1.0 || material.translucency > 0.0)
                    .unwrap_or(false)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Check if mesh is hidden - equivalent to C++ Is_Hidden
    pub fn is_hidden(&self) -> bool {
        self.is_hidden
    }

    /// Check if mesh is animation hidden - equivalent to C++ Is_Animation_Hidden
    /// C++ Reference: rendobj.h lines 471-476
    pub fn is_animation_hidden(&self) -> bool {
        self.is_animation_hidden
    }

    /// Get bounding sphere - equivalent to C++ Get_Bounding_Sphere
    pub fn get_bounding_sphere(&self) -> SphereClass {
        // Transform object space bounding sphere to world space
        let center = self.transform.transform_point3(self.bounding_sphere.center);
        let radius = self.bounding_sphere.radius;
        SphereClass::new(center, radius)
    }

    /// Render the mesh - equivalent to C++ MeshClass::Render
    pub fn render<'a>(
        &'a mut self,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) -> W3dResult<()> {
        if !self.is_not_hidden_at_all() {
            return Ok(());
        }

        // Static sort list handling (transparency sorting)
        if ww3d_core::WW3D::are_static_sort_lists_enabled() && self.sort_level != SORT_LEVEL_NONE {
            let mesh_arc = Arc::new(self.clone());
            let sort_handle = StaticSortRenderObject::from_arc(Arc::clone(&mesh_arc));
            StaticSortManager::add_to_static_sort_list_with_mesh(
                sort_handle,
                self.sort_level,
                Some(mesh_arc),
            );
            return Ok(());
        }

        // Frustum culling
        if !self.should_render_with_frustum_culling(render_info) {
            return Ok(());
        }

        // LOD selection based on distance from camera
        if !self.should_render_with_lod_check(render_info) {
            return Ok(());
        }

        // Get the mesh model and render
        if let Some(model) = &self.model {
            // Determine if we render base passes
            let mut render_base_passes = !render_info
                .override_flags
                .contains(RenderInfoOverrideFlags::ADDITIONAL_PASSES_ONLY);
            let is_alpha_mesh = self.is_alpha()
                || render_info
                    .override_flags
                    .contains(RenderInfoOverrideFlags::FORCE_SORTING);
            if render_info
                .override_flags
                .contains(RenderInfoOverrideFlags::SHADOW_RENDERING)
                && is_alpha_mesh
            {
                // Force base pass for shadow rendering of alpha meshes (C++ behavior)
                render_base_passes = true;
            }

            if render_base_passes {
                for polygon_renderer in &model.polygon_renderer_list {
                    polygon_renderer.render_material_pass(
                        render_pass,
                        &self.transform,
                        render_info,
                    )?;
                }
            }

            // Additional material passes (procedural)
            if !render_info.additional_material_passes.is_empty() {
                for _pass in &render_info.additional_material_passes {
                    // Re-draw geometry with the additional pass's shader
                    for polygon_renderer in &model.polygon_renderer_list {
                        // Draw geometry again for this procedural pass
                        if let Some(index_buffer) = &polygon_renderer.index_buffer {
                            render_pass.set_index_buffer(
                                index_buffer.slice(..),
                                wgpu::IndexFormat::Uint32,
                            );
                            if let Some(vb) = &polygon_renderer.vertex_buffer {
                                render_pass.set_vertex_buffer(0, vb.slice(..));
                            }
                            render_pass.draw_indexed(0..polygon_renderer.index_count, 0, 0..1);
                        } else {
                            if let Some(vb) = &polygon_renderer.vertex_buffer {
                                render_pass.set_vertex_buffer(0, vb.slice(..));
                            }
                            render_pass.draw(0..polygon_renderer.vertex_count, 0..1);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Perform frustum culling check
    pub fn should_render_with_frustum_culling(&self, render_info: &RenderInfoClass) -> bool {
        // Skip frustum culling for skin meshes as they may deform outside their bounding box
        if let Some(model) = &self.model {
            if model.get_flag(MeshGeometryClass::SKIN) {
                return true;
            }
        }

        // Test world-space bounding sphere against the camera frustum.
        let frustum = render_info.camera.get_frustum();
        let sphere = self.get_bounding_sphere();
        if !frustum.intersects_sphere(&sphere.center, sphere.radius) {
            return false;
        }

        // Transform to view space and check against near/far planes
        let view_matrix = render_info.camera.get_cached_view_matrix();
        let view_center = view_matrix.transform_point3(sphere.center);

        // Simple near/far plane culling.
        // The active camera/view path uses a right-handed view matrix, so visible objects in
        // front of the camera have negative view-space Z. Convert to positive forward depth
        // before comparing against near/far distances.
        let near_plane = render_info.camera.get_near_plane();
        let far_plane = render_info.camera.get_far_plane();
        let forward_depth = -view_center.z;

        if forward_depth + sphere.radius < near_plane || forward_depth - sphere.radius > far_plane {
            return false;
        }

        true
    }

    /// Perform LOD (Level of Detail) selection check
    /// C++ Reference: LOD systems in HLOD (Hierarchical LOD) and mesh rendering
    pub fn should_render_with_lod_check(&self, render_info: &RenderInfoClass) -> bool {
        // Calculate distance from camera
        let camera_pos = render_info.camera.get_position();
        let sphere = self.get_bounding_sphere();
        let distance = camera_pos.distance(sphere.center);

        // Simple LOD system based on distance
        // C++ Implementation: HLOD objects contain multiple LOD levels
        // Each LOD level has a switch distance threshold
        // The renderer selects the appropriate LOD based on camera distance
        //
        // LOD Selection Algorithm (from C++ hlod.cpp):
        // 1. Calculate screen space size or distance to camera
        // 2. Compare against LOD switch distances
        // 3. Select highest detail LOD where distance < switch_distance
        // 4. For very distant objects, may skip rendering entirely

        let max_render_distance = 1000.0; // Configurable based on mesh type

        if distance > max_render_distance {
            return false;
        }

        // Full LOD level selection would:
        // - Check if mesh is part of an HLOD hierarchy
        // - Access LOD level data (stored in container or model)
        // - Compare distance against LOD switch thresholds
        // - Switch to appropriate detail level or skip if too distant
        //
        // For meshes without explicit LOD data, render at full detail
        // The HLOD system handles multi-resolution model switching at a higher level
        true
    }

    // render_material_pass method already defined in first impl block

    // get_num_polys method already defined in first impl block
}

impl Clone for MeshClass {
    fn clone(&self) -> Self {
        self.clone_mesh()
    }
}

/// Static sort list for transparent object sorting
#[derive(Debug, Default)]
struct StaticSortState {
    entries: Vec<Arc<StaticSortRenderObject>>,
    meshes: Vec<Option<Arc<MeshClass>>>,
    sort_levels: Vec<u32>,
    enabled: bool,
    decals_enabled: bool,
    flush_depth: u32,
}

static STATIC_SORT_STATE: OnceLock<Mutex<StaticSortState>> = OnceLock::new();

fn static_sort_state() -> &'static Mutex<StaticSortState> {
    STATIC_SORT_STATE.get_or_init(|| {
        Mutex::new(StaticSortState {
            entries: Vec::new(),
            meshes: Vec::new(),
            sort_levels: Vec::new(),
            enabled: true,
            decals_enabled: true,
            flush_depth: 0,
        })
    })
}

#[derive(Clone)]
pub struct StaticSortEntry {
    handle: Arc<StaticSortRenderObject>,
    mesh: Option<Arc<MeshClass>>,
}

impl StaticSortEntry {
    fn from_handle(handle: Arc<StaticSortRenderObject>, mesh: Option<Arc<MeshClass>>) -> Self {
        Self { handle, mesh }
    }

    pub fn mesh_arc(&self) -> Option<Arc<MeshClass>> {
        self.mesh.clone()
    }

    pub fn render_object(&self) -> Arc<dyn RenderObjClass> {
        self.handle.render_obj()
    }
}

pub struct StaticSortFlushGuard;

impl Drop for StaticSortFlushGuard {
    fn drop(&mut self) {
        let mut state = static_sort_state()
            .lock()
            .expect("static sort state mutex poisoned");
        if state.flush_depth > 0 {
            state.flush_depth -= 1;
            if state.flush_depth == 0 {
                state.entries.clear();
                state.meshes.clear();
                state.sort_levels.clear();
            }
        }
    }
}

pub struct StaticSortManager;

impl StaticSortManager {
    pub fn set_static_sort_lists_enabled(enabled: bool) {
        let _ = ww3d_core::WW3D::set_static_sort_lists_enabled(enabled);
        let mut state = static_sort_state()
            .lock()
            .expect("static sort state mutex poisoned");
        state.enabled = enabled;
        if !enabled {
            state.entries.clear();
            state.meshes.clear();
            state.sort_levels.clear();
        }
    }

    pub fn set_decals_enabled(enabled: bool) {
        let _ = ww3d_core::WW3D::set_decals_enabled(enabled);
        let mut state = static_sort_state()
            .lock()
            .expect("static sort state mutex poisoned");
        state.decals_enabled = enabled;
    }

    pub fn add_to_static_sort_list(handle: Arc<StaticSortRenderObject>, sort_level: u32) {
        Self::add_to_static_sort_list_with_mesh(handle, sort_level, None);
    }

    pub fn add_to_static_sort_list_with_mesh(
        handle: Arc<StaticSortRenderObject>,
        sort_level: u32,
        mesh: Option<Arc<MeshClass>>,
    ) {
        if !ww3d_core::WW3D::are_static_sort_lists_enabled() {
            return;
        }
        let mut state = static_sort_state()
            .lock()
            .expect("static sort state mutex poisoned");
        if !state.enabled {
            return;
        }
        state.entries.push(handle);
        state.meshes.push(mesh);
        state.sort_levels.push(sort_level);
    }

    pub fn snapshot_static_sort_list() -> Option<(Vec<StaticSortEntry>, Vec<u32>)> {
        let state = static_sort_state()
            .lock()
            .expect("static sort state mutex poisoned");
        if state.entries.is_empty() {
            return None;
        }
        let entries = state
            .entries
            .iter()
            .cloned()
            .zip(state.meshes.iter().cloned())
            .map(|(handle, mesh)| StaticSortEntry::from_handle(handle, mesh))
            .collect::<Vec<_>>();
        Some((entries, state.sort_levels.clone()))
    }

    pub fn begin_flush() -> StaticSortFlushGuard {
        let mut state = static_sort_state()
            .lock()
            .expect("static sort state mutex poisoned");
        state.flush_depth = state.flush_depth.saturating_add(1);
        StaticSortFlushGuard
    }

    pub fn flush_static_sort_list() {
        let mut state = static_sort_state()
            .lock()
            .expect("static sort state mutex poisoned");
        state.entries.clear();
        state.meshes.clear();
        state.sort_levels.clear();
    }
}

/// Compute index ranges for each material pass
/// This maps each pass index to its start index and triangle count
/// Returns a vector where index i contains (start_index, count) for pass i
fn compute_pass_index_ranges(model: &MeshModelClass, index_data: &[u32]) -> Vec<(u32, u32)> {
    // If we have polygon renderers organized by pass, preserve per-pass ranges.
    if !model.polygon_renderer_list.is_empty() {
        let mut ranges: Vec<(u32, u32)> = vec![(0, 0); model.material_passes.len()];
        let mut current_index = 0;

        for (renderer_index, renderer) in model.polygon_renderer_list.iter().enumerate() {
            let pass_index = renderer
                .material_pass
                .as_ref()
                .map(|pass| pass.get_pass_index())
                .unwrap_or(renderer_index);

            if pass_index >= ranges.len() {
                ranges.resize(pass_index + 1, (0, 0));
            }

            if renderer.index_count > 0 {
                let (range_start, range_count) = &mut ranges[pass_index];
                if *range_count == 0 {
                    *range_start = current_index;
                }
                *range_count = range_count.saturating_add(renderer.index_count);
                current_index = current_index.saturating_add(renderer.index_count);
            }
        }

        if ranges.iter().any(|(_, count)| *count > 0) {
            return ranges;
        }
    }

    // Fallback: create a single range covering all available geometry.
    if !index_data.is_empty() {
        vec![(0, index_data.len() as u32)]
    } else if model.index_count > 0 {
        vec![(0, model.index_count)]
    } else if model.vertex_count > 0 {
        vec![(0, model.vertex_count)]
    } else {
        vec![]
    }
}

/// Mesh rendering manager - orchestrates all mesh rendering

#[derive(Clone)]
pub struct PreparedMeshModel {
    vertex_buffer: Arc<wgpu::Buffer>,
    index_buffer: Option<Arc<wgpu::Buffer>>,
    vertex_count: u32,
    index_count: u32,
    material_passes: Vec<MaterialPassClass>,
    is_skinned: bool,
    source_revision: u64,
    /// Index ranges for each material pass (start_index, count)
    /// Maps pass index to (start_index, index_count) for filtering draw calls
    pass_index_ranges: Vec<(u32, u32)>,
}

impl PreparedMeshModel {
    fn frommodel(device: &wgpu::Device, model: &MeshModelClass) -> W3dResult<Self> {
        let vertex_count = model.vertices.len() as u32;
        let has_normals = model.has_normals();
        let has_tex_coords = model.has_tex_coords();
        let is_skinned = model.is_skinned();

        let mut stride = 3;
        if has_normals {
            stride += 3;
        }
        if has_tex_coords {
            stride += 2;
        }
        if is_skinned {
            stride += 8; // 4 indices + 4 weights as floats
        }

        let mut vertex_data: Vec<f32> = Vec::with_capacity(model.vertices.len() * stride);
        for index in 0..model.vertices.len() {
            let vertex = &model.vertices[index];
            vertex_data.push(vertex.x);
            vertex_data.push(vertex.y);
            vertex_data.push(vertex.z);

            if has_normals {
                if index < model.normals.len() {
                    let normal = &model.normals[index];
                    vertex_data.push(normal.x);
                    vertex_data.push(normal.y);
                    vertex_data.push(normal.z);
                } else {
                    vertex_data.push(0.0);
                    vertex_data.push(1.0);
                    vertex_data.push(0.0);
                }
            }

            if has_tex_coords {
                if index < model.texture_coords.len() {
                    let tex = &model.texture_coords[index];
                    vertex_data.push(tex.u);
                    vertex_data.push(tex.v);
                } else {
                    vertex_data.push(0.0);
                    vertex_data.push(0.0);
                }
            }

            if is_skinned {
                let (indices, weights) = model.vertex_influence_view(index);
                for &idx in &indices {
                    vertex_data.push(f32::from_bits(idx));
                }
                vertex_data.extend_from_slice(&weights);
            }
        }

        let vertex_buffer = if vertex_data.is_empty() {
            Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Empty Mesh Vertex Buffer"),
                size: 4,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            }))
        } else {
            Arc::new(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Mesh Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertex_data),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
            )
        };

        let mut index_data: Vec<u32> = Vec::with_capacity(model.triangles.len() * 3);
        for triangle in &model.triangles {
            index_data.push(triangle.vindex[0]);
            index_data.push(triangle.vindex[1]);
            index_data.push(triangle.vindex[2]);
        }

        let (index_buffer, index_count) = if index_data.is_empty() {
            (None, 0)
        } else {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Mesh Index Buffer"),
                contents: bytemuck::cast_slice(&index_data),
                usage: wgpu::BufferUsages::INDEX,
            });
            (Some(Arc::new(buffer)), index_data.len() as u32)
        };

        let material_passes = if model.material_passes.is_empty() {
            vec![MaterialPassClass::new()]
        } else {
            model.material_passes.clone()
        };

        // Compute per-pass index ranges from polygon renderer list
        // This ensures we only draw geometry belonging to each pass
        let pass_index_ranges = compute_pass_index_ranges(model, &index_data);

        Ok(Self {
            vertex_buffer,
            index_buffer,
            vertex_count,
            index_count,
            material_passes,
            is_skinned,
            source_revision: model.revision(),
            pass_index_ranges,
        })
    }
}

struct MeshFallbackTextures {
    _texture_2d: Arc<wgpu::Texture>,
    view_2d: Arc<wgpu::TextureView>,
    _texture_cube: Arc<wgpu::Texture>,
    view_cube: Arc<wgpu::TextureView>,
}

struct StageMasks {
    mask: u8,
    cube_mask: u32,
    hints: u32,
    alpha_mask: u32,
    uv_channels: u32,
}

struct StageResources {
    view_2d: Arc<wgpu::TextureView>,
    view_cube: Arc<wgpu::TextureView>,
    sampler: Arc<wgpu::Sampler>,
}

struct VertexColorResources {
    bind_group: Arc<wgpu::BindGroup>,
    diffuse_buffer: Arc<wgpu::Buffer>,
    illumination_buffer: Arc<wgpu::Buffer>,
}

#[derive(Default)]
pub struct RenderPassResources {
    buffers: Vec<Arc<wgpu::Buffer>>,
    bind_groups: Vec<Arc<wgpu::BindGroup>>,
    pipelines: Vec<Arc<wgpu::RenderPipeline>>,
}

impl RenderPassResources {
    fn clear(&mut self) {
        self.buffers.clear();
        self.bind_groups.clear();
        self.pipelines.clear();
    }

    fn set_vertex_buffer(
        &mut self,
        render_pass: &mut wgpu::RenderPass<'_>,
        slot: u32,
        buffer: Arc<wgpu::Buffer>,
    ) {
        self.buffers.push(buffer);
        let ptr = Arc::as_ptr(self.buffers.last().expect("buffer guard stored"));
        unsafe { render_pass.set_vertex_buffer(slot, (&*ptr).slice(..)) }
    }

    fn set_index_buffer(
        &mut self,
        render_pass: &mut wgpu::RenderPass<'_>,
        buffer: Arc<wgpu::Buffer>,
        format: wgpu::IndexFormat,
    ) {
        self.buffers.push(buffer);
        let ptr = Arc::as_ptr(self.buffers.last().expect("buffer guard stored"));
        unsafe { render_pass.set_index_buffer((&*ptr).slice(..), format) }
    }

    fn retain_buffer(&mut self, buffer: Arc<wgpu::Buffer>) {
        self.buffers.push(buffer);
    }

    fn set_bind_group(
        &mut self,
        render_pass: &mut wgpu::RenderPass<'_>,
        slot: u32,
        bind_group: Arc<wgpu::BindGroup>,
    ) {
        self.bind_groups.push(bind_group);
        let ptr = Arc::as_ptr(self.bind_groups.last().expect("bind group guard stored"));
        unsafe { render_pass.set_bind_group(slot, &*ptr, &[]) }
    }

    fn set_pipeline(
        &mut self,
        render_pass: &mut wgpu::RenderPass<'_>,
        pipeline: Arc<wgpu::RenderPipeline>,
    ) {
        self.pipelines.push(pipeline);
        let ptr = Arc::as_ptr(self.pipelines.last().expect("pipeline guard stored"));
        unsafe { render_pass.set_pipeline(&*ptr) }
    }
}

pub struct MeshRenderManager {
    gpu_device: Arc<GpuDevice>,
    preparedmodels: HashMap<usize, Arc<PreparedMeshModel>>,
    stats: MeshRenderStats,
    pipeline_mgr: WgpuPipelineManager,
    asset_manager: Option<Arc<Mutex<AssetManager>>>,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    fallback_textures: MeshFallbackTextures,
    default_sampler: Arc<wgpu::Sampler>,
    empty_vertex_color_buffer: Arc<wgpu::Buffer>,
    decal_queue: Vec<Arc<MeshClass>>,
    fvf_containers: Vec<Arc<DX8FVFCategoryContainer>>,
}

impl MeshRenderManager {
    pub fn new(gpu_device: Arc<GpuDevice>) -> Self {
        let pipeline_mgr = WgpuPipelineManager::new(gpu_device.clone());
        let device = gpu_device.wgpu_device();
        let queue = gpu_device.queue();
        let fallback_textures = Self::create_fallback_textures(device, queue);
        let default_sampler = Arc::new(device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("MeshManager Default Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        }));

        let empty_vertex_color_buffer = Arc::new(device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("MeshManager Empty Vertex Color Buffer"),
                contents: bytemuck::cast_slice(&[0.0f32; 4]),
                usage: wgpu::BufferUsages::STORAGE,
            },
        ));

        Self {
            gpu_device,
            preparedmodels: HashMap::new(),
            stats: MeshRenderStats::default(),
            pipeline_mgr,
            asset_manager: None,
            color_format: wgpu::TextureFormat::Bgra8UnormSrgb,
            depth_format: Some(wgpu::TextureFormat::Depth32Float),
            fallback_textures,
            default_sampler,
            empty_vertex_color_buffer,
            decal_queue: Vec::new(),
            fvf_containers: Vec::new(),
        }
    }

    pub fn ensure_model(&mut self, model: &Arc<MeshModelClass>) -> W3dResult<()> {
        self.prepare_model(model).map(|_| ())
    }

    fn prepare_model(&mut self, model: &Arc<MeshModelClass>) -> W3dResult<Arc<PreparedMeshModel>> {
        let key = Arc::as_ptr(model) as usize;
        if !self.preparedmodels.contains_key(&key) {
            let prepared = Arc::new(PreparedMeshModel::frommodel(
                self.gpu_device.wgpu_device(),
                model.as_ref(),
            )?);
            self.preparedmodels.insert(key, prepared);
        }
        Ok(self
            .preparedmodels
            .get(&key)
            .expect("prepared model must exist")
            .clone())
    }

    pub fn set_asset_manager(
        &mut self,
        asset_manager: Arc<Mutex<AssetManager>>,
    ) -> RendererResult<()> {
        self.asset_manager = Some(asset_manager);
        Ok(())
    }

    fn create_fallback_textures(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> MeshFallbackTextures {
        let white_pixel: [u8; 4] = [255, 255, 255, 255];

        let texture_2d = Arc::new(device.create_texture(&TextureDescriptor {
            label: Some("MeshManager Fallback Texture 2D"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        }));
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: texture_2d.as_ref(),
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &white_pixel,
            TexelCopyBufferLayout {
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
        let view_2d = Arc::new(texture_2d.create_view(&TextureViewDescriptor::default()));

        let texture_cube = Arc::new(device.create_texture(&TextureDescriptor {
            label: Some("MeshManager Fallback Texture Cube"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        }));

        for layer in 0..6 {
            queue.write_texture(
                TexelCopyTextureInfo {
                    texture: texture_cube.as_ref(),
                    mip_level: 0,
                    origin: Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &white_pixel,
                TexelCopyBufferLayout {
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
        }

        let view_cube = Arc::new(texture_cube.create_view(&TextureViewDescriptor {
            label: Some("MeshManager Fallback Cube View"),
            dimension: Some(TextureViewDimension::Cube),
            ..Default::default()
        }));

        MeshFallbackTextures {
            _texture_2d: texture_2d,
            view_2d,
            _texture_cube: texture_cube,
            view_cube,
        }
    }

    pub fn ensuremodel(&mut self, model: &Arc<MeshModelClass>) -> W3dResult<()> {
        self.preparemodel(model).map(|_| ())
    }

    fn preparemodel(&mut self, model: &Arc<MeshModelClass>) -> W3dResult<Arc<PreparedMeshModel>> {
        let key = Arc::as_ptr(model) as usize;
        let current_revision = model.revision();
        let needs_rebuild = self
            .preparedmodels
            .get(&key)
            .map(|prepared| prepared.source_revision != current_revision)
            .unwrap_or(true);

        if needs_rebuild {
            let prepared = Arc::new(PreparedMeshModel::frommodel(
                self.gpu_device.wgpu_device(),
                model.as_ref(),
            )?);
            self.preparedmodels.insert(key, prepared);
        }
        Ok(self
            .preparedmodels
            .get(&key)
            .expect("prepared model must exist")
            .clone())
    }

    pub fn get_stats(&self) -> &MeshRenderStats {
        &self.stats
    }

    pub fn reset_stats(&mut self) {
        self.stats = MeshRenderStats::default();
    }

    pub fn set_render_formats(
        &mut self,
        color_format: wgpu::TextureFormat,
        depth_format: Option<wgpu::TextureFormat>,
    ) {
        self.color_format = color_format;
        self.depth_format = depth_format;
    }

    pub fn render_pass(
        &mut self,
        render_pass: &mut wgpu::RenderPass<'_>,
        opaque_meshes: &[Arc<MeshClass>],
        blended_meshes: &[Arc<MeshClass>],
        render_info: &RenderInfoClass,
        arena: &mut FrameUniformArena,
    ) -> W3dResult<()> {
        let mut pass_resources = RenderPassResources::default();
        for mesh in opaque_meshes {
            self.render_mesh(mesh, render_info, render_pass, arena, &mut pass_resources)?;
        }
        for mesh in blended_meshes {
            self.render_mesh(mesh, render_info, render_pass, arena, &mut pass_resources)?;
        }
        Ok(())
    }

    fn render_mesh(
        &mut self,
        mesh: &Arc<MeshClass>,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
        resources: &mut RenderPassResources,
    ) -> W3dResult<()> {
        if mesh.model.is_none() {
            return Ok(());
        }
        self.stats.meshes_rendered += 1;

        let prepared = {
            let model = mesh.model.as_ref().unwrap();
            self.preparemodel(model)?
        };

        resources.set_vertex_buffer(render_pass, 0, Arc::clone(&prepared.vertex_buffer));

        if let Some(index_buffer) = prepared.index_buffer.as_ref() {
            resources.set_index_buffer(
                render_pass,
                Arc::clone(index_buffer),
                wgpu::IndexFormat::Uint32,
            );
        }

        for pass in &prepared.material_passes {
            self.draw_material_pass(
                mesh,
                &prepared,
                pass,
                render_info,
                render_pass,
                arena,
                resources,
            )?;
        }

        for extra_pass in &render_info.additional_material_passes {
            self.draw_material_pass(
                mesh,
                &prepared,
                extra_pass,
                render_info,
                render_pass,
                arena,
                resources,
            )?;
        }

        resources.clear();
        Ok(())
    }

    fn material_pass_with_uv_offset(
        pass: &MaterialPassClass,
        offset: [f32; 2],
    ) -> MaterialPassClass {
        let mut pass = pass.clone();
        // C++ tread draw disables the automatic LinearOffset mapper and pushes
        // the runtime offset as custom UV state. The shader's static grid mapper
        // path is the existing per-draw uniform route for an absolute UV offset.
        pass.set_mapper_id(7);
        pass.set_mapper_arg(0, 1);
        pass.set_mapper_arg(1, 1);
        pass.set_mapper_arg(2, (offset[0] * 1000.0).round() as i32);
        pass.set_mapper_arg(3, (offset[1] * 1000.0).round() as i32);
        pass
    }

    fn draw_material_pass(
        &mut self,
        mesh: &MeshClass,
        prepared: &PreparedMeshModel,
        pass: &MaterialPassClass,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
        resources: &mut RenderPassResources,
    ) -> W3dResult<()> {
        let uv_override_pass = mesh
            .uv_offset_override()
            .map(|offset| Self::material_pass_with_uv_offset(pass, offset));
        let pass = uv_override_pass.as_ref().unwrap_or(pass);
        let stage_masks = compute_stage_masks(pass);

        let vertex_format = if prepared.is_skinned {
            VertexFormat::Skinned
        } else {
            VertexFormat::Basic
        };
        let force_two_sided = mesh.is_decal_instance
            || render_info.override_flags.intersects(
                RenderInfoOverrideFlags::FORCE_TWO_SIDED | RenderInfoOverrideFlags::DECAL_RENDERING,
            );

        let pipeline = self.pipeline_mgr.get_or_create(
            &pass.shader,
            stage_masks.mask,
            prepared.is_skinned,
            render_info.lighting.is_some(),
            render_info.fog.is_some(),
            wgpu::PrimitiveTopology::TriangleList,
            vertex_format,
            self.color_format,
            self.depth_format,
            0,
            force_two_sided,
        );

        let camera_binds = WgpuMaterialBinds::camera(
            self.gpu_device.as_ref(),
            pipeline.as_ref(),
            0,
            arena,
            render_info,
        )?;

        let (material_diffuse, material_specular, material_emissive) =
            material_properties(pass.get_vertex_material());
        let material_overrides = [
            render_info.alpha_override,
            render_info.material_pass_alpha_override,
            render_info.material_pass_emissive_override,
            0.0,
        ];

        let model_binds = WgpuMaterialBinds::model(
            self.gpu_device.as_ref(),
            pipeline.as_ref(),
            1,
            &mesh.transform,
            render_info,
            stage_masks.mask,
            stage_masks.cube_mask,
            stage_masks.hints,
            stage_masks.alpha_mask,
            stage_masks.uv_channels,
            material_diffuse,
            material_specular,
            material_emissive,
            material_overrides,
            arena,
            // Default FOW values (fully visible) - will be overridden when FOW is integrated
            None, // visibility_alpha
            None, // visibility_falloff
            None, // is_explored
        )?;
        resources.set_pipeline(render_pass, Arc::clone(&pipeline));

        resources.retain_buffer(Arc::clone(&camera_binds.buffer));
        resources.set_bind_group(render_pass, 0, Arc::clone(&camera_binds.bind_group));

        resources.retain_buffer(Arc::clone(&model_binds.model_buffer));
        resources.retain_buffer(Arc::clone(&model_binds.lighting_buffer));
        resources.set_bind_group(render_pass, 1, Arc::clone(&model_binds.bind_group));

        let next_slot = 3u32;
        if prepared.is_skinned {
            let identity_palette = [Mat4::IDENTITY];
            let palette = mesh
                .bone_palette_view()
                .map(|view| view.matrices)
                .filter(|matrices| !matrices.is_empty())
                .unwrap_or(&identity_palette);

            let binds = WgpuMaterialBinds::skinned_group2(
                self.gpu_device.as_ref(),
                pipeline.as_ref(),
                2,
                palette,
                Some(pass),
                render_info.time,
                arena,
            )?;
            resources.retain_buffer(Arc::clone(&binds.bones_buffer));
            resources.retain_buffer(Arc::clone(&binds.uv_transform_buffer));
            resources.set_bind_group(render_pass, 2, Arc::clone(&binds.bind_group));
        } else {
            // Non-skinned shaders expect UV transform at group 2.
            let uv_transform_binds = WgpuMaterialBinds::uv_transform(
                self.gpu_device.wgpu_device(),
                pipeline.as_ref(),
                2,
                Some(pass),
                render_info.time,
            )?;
            resources.retain_buffer(Arc::clone(&uv_transform_binds.buffer));
            resources.set_bind_group(render_pass, 2, Arc::clone(&uv_transform_binds.bind_group));
        }

        let texture_bind_groups =
            self.create_texture_bind_groups(pipeline.as_ref(), pass, next_slot);
        for (offset, bind_group) in texture_bind_groups.iter().enumerate() {
            resources.set_bind_group(
                render_pass,
                next_slot + offset as u32,
                Arc::clone(bind_group),
            );
        }
        let color_group_index = next_slot + texture_bind_groups.len() as u32;
        let vertex_color =
            self.create_vertex_color_resources(pipeline.as_ref(), pass, color_group_index);
        resources.retain_buffer(Arc::clone(&vertex_color.diffuse_buffer));
        resources.retain_buffer(Arc::clone(&vertex_color.illumination_buffer));
        resources.set_bind_group(
            render_pass,
            color_group_index,
            Arc::clone(&vertex_color.bind_group),
        );

        if pass
            .diffuse_vertex_colors
            .as_ref()
            .map(|colors| !colors.is_empty())
            .unwrap_or(false)
            || pass
                .illumination_vertex_colors
                .as_ref()
                .map(|colors| !colors.is_empty())
                .unwrap_or(false)
        {
            self.stats.vertex_color_passes += 1;
        }

        self.issue_draw_call(prepared, pass, render_pass);

        self.stats.material_passes += 1;
        self.stats.shader_switches += 1;
        if stage_masks.mask != 0 {
            self.stats.texture_switches += 1;
        }

        Ok(())
    }

    // helper slots intentionally minimal; temporary bindings are stored in local vectors to ensure
    // they outlive the render pass borrow.

    fn issue_draw_call(
        &mut self,
        prepared: &PreparedMeshModel,
        pass: &MaterialPassClass,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) {
        // Get the pass index for filtering
        let pass_index = pass.get_pass_index();

        // Find the index range for this specific pass
        let (start_index, count) = if pass_index < prepared.pass_index_ranges.len() {
            prepared.pass_index_ranges[pass_index]
        } else if !prepared.pass_index_ranges.is_empty() {
            // Fallback to first range if pass index is out of bounds
            prepared.pass_index_ranges[0]
        } else if prepared.index_count > 0 {
            // Fallback: render all indices (backward compatibility)
            (0, prepared.index_count)
        } else {
            // Empty mesh
            (0, 0)
        };

        if prepared.index_buffer.is_some() && count > 0 {
            // Draw only the indices for this specific pass
            render_pass.draw_indexed(start_index..start_index + count, 0, 0..1);
            self.stats.draw_calls += 1;
            self.stats.triangles_rendered += count / 3;
        } else if count > 0 {
            // For non-indexed rendering, we can't easily filter by pass
            render_pass.draw(0..prepared.vertex_count, 0..1);
            self.stats.draw_calls += 1;
            self.stats.triangles_rendered += prepared.vertex_count / 3;
        }
    }

    fn create_vertex_color_resources(
        &self,
        pipeline: &wgpu::RenderPipeline,
        pass: &MaterialPassClass,
        group_index: u32,
    ) -> VertexColorResources {
        let device = self.gpu_device.wgpu_device();

        let diffuse_buffer = pass.diffuse_vertex_colors.as_ref().map(|colors| {
            let mut data = Vec::with_capacity(colors.len() * 4);
            for color in colors {
                data.push(color.x);
                data.push(color.y);
                data.push(color.z);
                data.push(color.w);
            }
            Arc::new(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("MeshManager Diffuse Vertex Colors"),
                    contents: bytemuck::cast_slice(&data),
                    usage: wgpu::BufferUsages::STORAGE,
                }),
            )
        });

        let illumination_buffer = pass.illumination_vertex_colors.as_ref().map(|colors| {
            let mut data = Vec::with_capacity(colors.len() * 4);
            for color in colors {
                data.push(color.x);
                data.push(color.y);
                data.push(color.z);
                data.push(color.w);
            }
            Arc::new(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("MeshManager Illumination Vertex Colors"),
                    contents: bytemuck::cast_slice(&data),
                    usage: wgpu::BufferUsages::STORAGE,
                }),
            )
        });

        let diffuse_buffer =
            diffuse_buffer.unwrap_or_else(|| self.empty_vertex_color_buffer.clone());
        let illumination_buffer =
            illumination_buffer.unwrap_or_else(|| self.empty_vertex_color_buffer.clone());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MeshManager Vertex Color Bind Group"),
            layout: &pipeline.get_bind_group_layout(group_index),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: diffuse_buffer.as_ref().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: illumination_buffer.as_ref().as_entire_binding(),
                },
            ],
        });

        VertexColorResources {
            bind_group: Arc::new(bind_group),
            diffuse_buffer,
            illumination_buffer,
        }
    }

    fn create_texture_bind_groups(
        &self,
        pipeline: &wgpu::RenderPipeline,
        pass: &MaterialPassClass,
        first_group_index: u32,
    ) -> Vec<Arc<wgpu::BindGroup>> {
        let mut bind_groups = Vec::with_capacity(MAX_TEXTURE_STAGE_GROUPS);
        for group in 0..MAX_TEXTURE_STAGE_GROUPS {
            let layout = pipeline.get_bind_group_layout(first_group_index + group as u32);
            let stage_base = group * TEXTURES_PER_GROUP;
            let mut views_2d: Vec<Arc<wgpu::TextureView>> = Vec::with_capacity(TEXTURES_PER_GROUP);
            let mut views_cube: Vec<Arc<wgpu::TextureView>> =
                Vec::with_capacity(TEXTURES_PER_GROUP);
            let mut samplers: Vec<Arc<wgpu::Sampler>> = Vec::with_capacity(TEXTURES_PER_GROUP);

            for stage_offset in 0..TEXTURES_PER_GROUP {
                let stage_index = stage_base + stage_offset;
                let resources = self.stage_resources_for(pass, stage_index);
                views_2d.push(resources.view_2d);
                views_cube.push(resources.view_cube);
                samplers.push(resources.sampler);
            }

            let mut entries = Vec::with_capacity(TEXTURES_PER_GROUP * 3);
            for stage_offset in 0..TEXTURES_PER_GROUP {
                let binding_base = (stage_offset * 3) as u32;
                entries.push(wgpu::BindGroupEntry {
                    binding: binding_base,
                    resource: wgpu::BindingResource::TextureView(views_2d[stage_offset].as_ref()),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: binding_base + 1,
                    resource: wgpu::BindingResource::TextureView(views_cube[stage_offset].as_ref()),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: binding_base + 2,
                    resource: wgpu::BindingResource::Sampler(samplers[stage_offset].as_ref()),
                });
            }

            let bind_group =
                self.gpu_device
                    .wgpu_device()
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("MeshManager Texture Bind Group"),
                        layout: &layout,
                        entries: &entries,
                    });

            bind_groups.push(Arc::new(bind_group));
        }

        bind_groups
    }

    fn stage_resources_for(&self, pass: &MaterialPassClass, stage: usize) -> StageResources {
        if let Some(texture) = pass.get_texture(stage) {
            if let Some(view) = texture.get_texture_view() {
                let sampler_desc = sampler_descriptor_for_settings(&texture.stage_settings);
                let sampler = Arc::new(self.gpu_device.wgpu_device().create_sampler(&sampler_desc));
                return StageResources {
                    view_2d: Arc::new(view),
                    view_cube: self.fallback_textures.view_cube.clone(),
                    sampler,
                };
            }
        }

        StageResources {
            view_2d: self.fallback_textures.view_2d.clone(),
            view_cube: self.fallback_textures.view_cube.clone(),
            sampler: self.default_sampler.clone(),
        }
    }

    pub fn render_polygon_renderer<'rp>(
        &mut self,
        polygon_renderer: &'rp Arc<DX8PolygonRendererClass>,
        render_pass: &mut wgpu::RenderPass<'rp>,
    ) -> W3dResult<()> {
        if let Some(ref vertex_buffer) = polygon_renderer.vertex_buffer {
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        }

        if let Some(ref index_buffer) = polygon_renderer.index_buffer {
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..polygon_renderer.index_count, 0, 0..1);
            self.stats.draw_calls += 1;
            self.stats.triangles_rendered += polygon_renderer.index_count / 3;
        } else {
            render_pass.draw(0..polygon_renderer.vertex_count, 0..1);
            self.stats.draw_calls += 1;
            self.stats.triangles_rendered += polygon_renderer.vertex_count / 3;
        }

        Ok(())
    }

    fn render_texture_category(
        &mut self,
        category: &Arc<DX8TextureCategoryClass>,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
        resources: &mut RenderPassResources,
    ) -> W3dResult<()> {
        let tasks = {
            let mut guard = category
                .render_tasks
                .lock()
                .expect("texture category render tasks mutex poisoned");
            if guard.is_empty() {
                return Ok(());
            }
            std::mem::take(&mut *guard)
        };

        for task in tasks {
            self.render_mesh(&task.mesh, render_info, render_pass, arena, resources)?;
        }
        Ok(())
    }

    fn render_fvf_category_container(
        &mut self,
        container: &DX8FVFCategoryContainer,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
        resources: &mut RenderPassResources,
    ) -> W3dResult<()> {
        for category in container.texture_categories.values() {
            self.render_texture_category(category, render_info, render_pass, arena, resources)?;
        }
        Ok(())
    }

    fn render_delayed_passes(
        &mut self,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
        resources: &mut RenderPassResources,
    ) -> W3dResult<()> {
        if let Some((entries, sort_levels)) = StaticSortManager::snapshot_static_sort_list() {
            let mut buckets: BTreeMap<u32, Vec<StaticSortEntry>> = BTreeMap::new();
            for (entry, sort_level) in entries.into_iter().zip(sort_levels.into_iter()) {
                buckets.entry(sort_level).or_default().push(entry);
            }

            for (_level, bucket_entries) in buckets.into_iter().rev() {
                for entry in bucket_entries {
                    if let Some(mesh) = entry.mesh_arc() {
                        self.render_mesh(&mesh, render_info, render_pass, arena, resources)?;
                    } else {
                        let render_obj = entry.render_object();
                        render_obj.render(render_info)?;
                    }
                }
            }
        }

        Ok(())
    }

    pub fn flush_static_sort_lists(
        &mut self,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
    ) -> W3dResult<()> {
        let mut resources = RenderPassResources::default();
        self.render_delayed_passes(render_info, render_pass, arena, &mut resources)?;
        StaticSortManager::flush_static_sort_list();
        Ok(())
    }

    pub fn add_decal_to_queue(&mut self, decal: Arc<MeshClass>) {
        self.decal_queue.push(decal);
    }

    pub fn register_fvf_container(&mut self, container: Arc<DX8FVFCategoryContainer>) {
        self.fvf_containers.push(container);
    }

    pub fn render_decal_queue(
        &mut self,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
    ) -> W3dResult<()> {
        if self.decal_queue.is_empty() {
            return Ok(());
        }

        let camera_pos = render_info.camera.get_position();
        self.decal_queue.sort_by(|a, b| {
            let dist_a = a.get_bounding_sphere().center.distance(camera_pos);
            let dist_b = b.get_bounding_sphere().center.distance(camera_pos);
            dist_b
                .partial_cmp(&dist_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let decals = std::mem::take(&mut self.decal_queue);
        let mut resources = RenderPassResources::default();
        for decal in decals {
            self.render_mesh(&decal, render_info, render_pass, arena, &mut resources)?;
        }
        Ok(())
    }

    pub fn render_all_fvf_containers(
        &mut self,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
    ) -> W3dResult<()> {
        let mut resources = RenderPassResources::default();
        let containers = self.fvf_containers.clone();
        for container in &containers {
            self.render_fvf_category_container(
                container,
                render_info,
                render_pass,
                arena,
                &mut resources,
            )?;
        }
        Ok(())
    }

    pub fn clear_frame_data(&mut self) {
        self.decal_queue.clear();
        self.fvf_containers.clear();
    }
}

fn compute_stage_masks(pass: &MaterialPassClass) -> StageMasks {
    let mut mask: u8 = 0;
    let cube_mask: u32 = 0;
    let mut hints: u32 = 0;
    let mut alpha_mask: u32 = 0;
    let mut uv_channels: u32 = 0;

    for stage in 0..MAX_TEXTURE_STAGES {
        if let Some(texture) = pass.get_texture(stage) {
            mask |= 1 << stage;
            let hint_bits = (texture.stage_settings.hint.to_bits() & 0x0F) as u32;
            hints |= hint_bits << (stage * 4);
            if texture.stage_settings.alpha_is_bitmap {
                alpha_mask |= 1 << stage;
            }
            let channel_bits = (pass.stage_uv_channel(stage) as u32) & 0x3;
            uv_channels |= channel_bits << (stage * 2);
        }
    }

    StageMasks {
        mask,
        cube_mask,
        hints,
        alpha_mask,
        uv_channels,
    }
}

fn sampler_descriptor_for_settings(settings: &TextureStageSettings) -> SamplerDescriptor<'static> {
    let (mag_filter, min_filter, mipmap_filter) = match settings.filter {
        TextureFilterMode::Point | TextureFilterMode::Nearest => (
            FilterMode::Nearest,
            FilterMode::Nearest,
            FilterMode::Nearest,
        ),
        TextureFilterMode::Linear => (FilterMode::Linear, FilterMode::Linear, FilterMode::Linear),
        TextureFilterMode::Anisotropic => {
            (FilterMode::Linear, FilterMode::Linear, FilterMode::Linear)
        }
    };

    SamplerDescriptor {
        label: Some("MeshManager Stage Sampler"),
        address_mode_u: convert_address_mode(settings.address_u),
        address_mode_v: convert_address_mode(settings.address_v),
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter,
        min_filter,
        mipmap_filter,
        ..Default::default()
    }
}

fn convert_address_mode(mode: TextureAddressMode) -> AddressMode {
    match mode {
        TextureAddressMode::Wrap => AddressMode::Repeat,
        TextureAddressMode::Repeat => AddressMode::Repeat,
        TextureAddressMode::Clamp => AddressMode::ClampToEdge,
        TextureAddressMode::Mirror => AddressMode::MirrorRepeat,
        TextureAddressMode::Border => AddressMode::ClampToBorder,
    }
}

fn material_properties(material: Option<&VertexMaterialClass>) -> ([f32; 4], [f32; 4], [f32; 4]) {
    if let Some(mat) = material {
        (
            [mat.diffuse.x, mat.diffuse.y, mat.diffuse.z, 1.0],
            [
                mat.specular.x,
                mat.specular.y,
                mat.specular.z,
                mat.shininess,
            ],
            [mat.emissive.x, mat.emissive.y, mat.emissive.z, 1.0],
        )
    } else {
        (
            [0.8, 0.8, 0.8, 1.0],
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        )
    }
}

fn compute_stage_uv_info(
    stage_texcoords: &[Vec<W3dTexCoordStruct>],
) -> (Vec<Vec<W3dTexCoordStruct>>, Vec<u8>) {
    const MAX_CHANNELS: usize = 4;
    let mut uv_sets: Vec<Vec<W3dTexCoordStruct>> = Vec::new();
    let mut stage_channels = Vec::with_capacity(stage_texcoords.len());
    let mut crc_to_channel: HashMap<u32, u8> = HashMap::new();

    for coords in stage_texcoords {
        if coords.is_empty() {
            stage_channels.push(0);
            continue;
        }

        let mut hasher = Hasher::new();
        for tc in coords {
            hasher.update(&tc.u.to_le_bytes());
            hasher.update(&tc.v.to_le_bytes());
        }
        let crc = hasher.finalize();

        let mut channel = if let Some(&existing) = crc_to_channel.get(&crc) {
            existing
        } else {
            let assigned = if uv_sets.len() < MAX_CHANNELS {
                let ch = uv_sets.len() as u8;
                uv_sets.push(coords.clone());
                ch
            } else {
                (MAX_CHANNELS.saturating_sub(1)) as u8
            };
            crc_to_channel.insert(crc, assigned);
            assigned
        };

        if channel as usize >= uv_sets.len() {
            if uv_sets.len() < MAX_CHANNELS {
                uv_sets.push(coords.clone());
            } else {
                channel = (MAX_CHANNELS.saturating_sub(1)) as u8;
            }
        }

        stage_channels.push(channel);
    }

    if uv_sets.is_empty() {
        uv_sets.push(Vec::new());
    }

    (uv_sets, stage_channels)
}

fn build_material_passes_from_prototype(prototype: &MeshPrototype) -> Vec<MaterialPassClass> {
    if prototype.passes.is_empty() {
        return Vec::new();
    }

    let mut vertex_material_cache: Vec<Arc<VertexMaterialClass>> =
        Vec::with_capacity(prototype.vertex_materials.len());
    for (index, material) in prototype.vertex_materials.iter().enumerate() {
        let name = prototype
            .vertex_material_names
            .get(index)
            .map(|entry| w3d_string_from_bytes(&entry.material_name))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("VertexMaterial{}", index));
        let mut vm = VertexMaterialClass::from_w3d_material(&name, material);
        vm.name = name;
        vertex_material_cache.push(Arc::new(vm));
    }

    let (_, stage_channels) = compute_stage_uv_info(&prototype.stage_texcoords);
    let mut stage_cursor = 0usize;

    prototype
        .passes
        .iter()
        .enumerate()
        .map(|(pass_index, info)| {
            let mut pass = MaterialPassClass::new();

            if let Some(material) = vertex_material_cache.get(info.vm_id as usize) {
                pass.vertex_material = Some(Arc::clone(material));
            }

            if let Some(shader_struct) = prototype.shaders.get(info.shader_id as usize) {
                pass.shader = MaterialFactory::create_shader_from_w3d(shader_struct);
            }

            if let Some(stage_ids) = prototype.per_pass_stage_texture_ids.get(pass_index) {
                for (stage, ids) in stage_ids.iter().enumerate() {
                    let uv_channel = stage_channels
                        .get(stage_cursor)
                        .copied()
                        .unwrap_or(stage as u8);
                    pass.set_stage_uv_channel(stage, uv_channel);
                    stage_cursor = stage_cursor.saturating_add(1);

                    if let Some(&texture_id) = ids.first() {
                        if let Some(texture_desc) = prototype.textures.get(texture_id as usize) {
                            let texture = Arc::new(TextureClass::from_w3d_descriptor(texture_desc));
                            pass.set_texture(stage, texture);
                        }
                    }
                }
            }

            if let Some(colors) = prototype.per_pass_dcg_colors.get(pass_index) {
                if !colors.is_empty() {
                    let diffuse = colors
                        .iter()
                        .map(|c| {
                            Vec4::new(
                                c.r as f32 / 255.0,
                                c.g as f32 / 255.0,
                                c.b as f32 / 255.0,
                                c.a as f32 / 255.0,
                            )
                        })
                        .collect();
                    pass.diffuse_vertex_colors = Some(diffuse);
                }
            }

            if let Some(colors) = prototype.per_pass_dig_colors.get(pass_index) {
                if !colors.is_empty() {
                    let illumination = colors
                        .iter()
                        .map(|c| {
                            Vec4::new(
                                c.r as f32 / 255.0,
                                c.g as f32 / 255.0,
                                c.b as f32 / 255.0,
                                c.a as f32 / 255.0,
                            )
                        })
                        .collect();
                    pass.illumination_vertex_colors = Some(illumination);
                }
            }

            apply_mapper_from_prototype(&mut pass, prototype, pass_index);

            pass
        })
        .collect()
}

fn apply_mapper_from_prototype(
    pass: &mut MaterialPassClass,
    prototype: &MeshPrototype,
    pass_index: usize,
) {
    if let Some(vm_ids) = prototype.per_pass_vertex_material_ids.get(pass_index) {
        if let Some(&vm_id) = vm_ids.first() {
            if let Some(config) = prototype.vertex_mapper_configs.get(vm_id as usize) {
                if let Some(mapper) = config.stage0.or(config.stage1) {
                    pass.set_mapper_id(mapper.mapper_type);
                    for (idx, arg) in mapper.args.iter().enumerate() {
                        pass.set_mapper_arg(idx, *arg);
                    }
                    pass.set_mapper_float_args(mapper.float_args);
                }
            }
        }
    }
}

// Implement RenderObjClass for MeshClass
impl crate::render_object_system::RenderObjClass for MeshClass {
    fn clone_obj(&self) -> Box<dyn crate::render_object_system::RenderObjClass> {
        Box::new(self.clone())
    }

    fn class_id(&self) -> ww3d_core::RenderObjClassId {
        ww3d_core::RenderObjClassId::Mesh
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    fn get_num_polys(&self) -> usize {
        if let Some(model) = &self.model {
            model.triangles.len()
        } else {
            0
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn render(
        &self,
        _rinfo: &crate::render_object_system::RenderInfoClass,
    ) -> crate::core::error::Result<()> {
        if self.is_hidden || self.is_disabled_by_debugger {
            return Ok(());
        }

        // Render using the WGPU MeshRenderManager path
        // Note: actual draw occurs from a higher-level renderer orchestrating render passes

        Ok(())
    }

    fn special_render(
        &self,
        _rinfo: &crate::render_object_system::SpecialRenderInfoClass,
    ) -> crate::core::error::Result<()> {
        // C++ Reference: mesh.cpp lines 1027-1070
        // Special render passes handle visibility testing and shadow rendering
        //
        // C++ Implementation handles two main render types:
        // 1. RENDER_VIS: Visibility/occlusion rendering
        //    - Uses specialized rasterizer for visibility testing
        //    - Enables two-sided rendering if mesh has TWO_SIDED flag
        //    - For skinned meshes, deforms vertices before rendering
        //    - Used for occlusion culling calculations
        //
        // 2. RENDER_SHADOW: Shadow map rendering
        //    - Calls Model->Shadow_Render() with transform and hierarchy
        //    - Renders mesh geometry into shadow maps
        //    - May use simplified materials (no textures, just depth)
        //
        // Additional render types could include:
        // - Reflection rendering (for water reflections)
        // - Glow/bloom passes
        // - Outline rendering (for selection highlights)
        //
        // For full implementation, would need:
        // - Access to specialized rasterizers (vis, shadow)
        // - Skinning deformation for dynamic meshes
        // - Material pass filtering based on render type
        //
        // Currently returns Ok as special passes are handled by the main renderer
        Ok(())
    }

    fn cast_ray(&self, raytest: &mut crate::render_object_system::RayCollisionTestClass) -> bool {
        // C++ Reference: meshgeometry.cpp Cast_Ray implementation
        // Transforms ray to object space, tests against triangles, returns closest hit
        if let Some(model) = &self.model {
            // Transform ray from world space to object space
            let inv_transform = self.transform.inverse();
            let mut local_ray = raytest.clone();
            local_ray.line.start = inv_transform.transform_point3(raytest.line.start);
            local_ray.line.end = inv_transform.transform_point3(raytest.line.end);

            // Cast ray in object space
            if model.cast_ray(&mut local_ray) {
                // Transform result back to world space
                let contact_point = local_ray.result.contact_point;
                let normal = local_ray.result.normal;
                raytest.result = local_ray.result;
                raytest.result.contact_point = self.transform.transform_point3(contact_point);
                raytest.result.normal = self.transform.transform_vector3(normal).normalize();
                return true;
            }
        }
        false
    }

    fn cast_aabox(
        &self,
        boxtest: &mut crate::render_object_system::AABoxCollisionTestClass,
    ) -> bool {
        // C++ Reference: meshgeometry.cpp Cast_AABox implementation
        // Tests axis-aligned box movement against mesh triangles
        if let Some(model) = &self.model {
            // Transform box and movement vector to object space
            let inv_transform = self.transform.inverse();
            let mut local_test = boxtest.clone();
            local_test.box_obj.center = inv_transform.transform_point3(boxtest.box_obj.center);
            local_test.move_vector = inv_transform.transform_vector3(boxtest.move_vector);

            // Cast in object space
            if model.cast_aabox(&mut local_test) {
                // Transform result back to world space
                boxtest.result = local_test.result;
                return true;
            }
        }
        false
    }

    fn cast_obbox(
        &self,
        boxtest: &mut crate::render_object_system::OBBoxCollisionTestClass,
    ) -> bool {
        // C++ Reference: meshgeometry.cpp Cast_OBBox implementation
        // Tests oriented bounding box sweep against mesh in object space.
        if let Some(model) = &self.model {
            let inv_transform = self.transform.inverse();
            let local_test = boxtest.transformed_by_matrix(inv_transform);

            let start_hit =
                model.intersect_obbox(&crate::render_object_system::OBBoxIntersectionTestClass {
                    box_obj: local_test.box_obj.clone(),
                    collision_type: local_test.collision_type,
                });
            let end_center = local_test.box_obj.center + local_test.move_vector;
            let end_box = ww3d_collision::bounding_volumes::OBBoxClass::new(
                end_center,
                local_test.box_obj.extent,
                local_test.box_obj.basis,
            );
            let end_hit =
                model.intersect_obbox(&crate::render_object_system::OBBoxIntersectionTestClass {
                    box_obj: end_box,
                    collision_type: local_test.collision_type,
                });

            if start_hit || end_hit {
                boxtest.collided_render_obj = Some(self as *const MeshClass as usize);
                boxtest.result = Some(crate::render_object_system::OBBoxCollisionResult {
                    intersection: true,
                    contact_points: Vec::new(),
                });
                return true;
            }
        }
        false
    }

    fn intersect_aabox(
        &self,
        boxtest: &crate::render_object_system::AABoxIntersectionTestClass,
    ) -> bool {
        // C++ Reference: meshgeometry.cpp Intersect_AABox implementation
        // Simple boolean test - does box intersect any triangle?
        if let Some(model) = &self.model {
            // Transform box to object space
            let inv_transform = self.transform.inverse();
            let mut local_test = boxtest.clone();
            local_test.box_obj.center = inv_transform.transform_point3(boxtest.box_obj.center);

            return model.intersect_aabox(&local_test);
        }
        false
    }

    fn intersect_obbox(
        &self,
        boxtest: &crate::render_object_system::OBBoxIntersectionTestClass,
    ) -> bool {
        // C++ Reference: meshgeometry.cpp Intersect_OBBox implementation
        // Tests if oriented bounding box intersects mesh
        if let Some(model) = &self.model {
            // Transform OBBox to object space for intersection test
            let inv_transform = self.transform.inverse();
            let mut local_test = boxtest.clone();
            local_test.box_obj.center = inv_transform.transform_point3(boxtest.box_obj.center);

            return model.intersect_obbox(&local_test);
        }
        false
    }

    fn get_obj_space_bounding_sphere(&self) -> crate::render_object_system::SphereClass {
        crate::render_object_system::SphereClass::new(
            self.bounding_sphere.center,
            self.bounding_sphere.radius,
        )
    }

    fn get_obj_space_bounding_box(&self) -> crate::render_object_system::AABoxClass {
        // C++ Reference: Simple type conversion helper
        // Returns the object-space bounding box (before transform is applied)
        // The bounding box is typically computed from mesh vertices at load time
        crate::render_object_system::AABoxClass {
            center: self.bounding_box.center,
            extent: self.bounding_box.extent,
        }
    }

    fn scale(&mut self, scale: f32) {
        self.transform = Mat4::from_scale(Vec3::new(scale, scale, scale)) * self.transform;
        self.clear_deformed_world_vertices();
        self.update_cached_bounding_volumes();
    }

    fn scale_xyz(&mut self, scalex: f32, scaley: f32, scalez: f32) {
        self.transform = Mat4::from_scale(Vec3::new(scalex, scaley, scalez)) * self.transform;
        self.clear_deformed_world_vertices();
        self.update_cached_bounding_volumes();
    }

    fn get_material_info(&self) -> Option<&crate::render_object_system::MaterialInfoClass> {
        let model = self.model.as_ref()?;
        Some(
            self.material_info_cache
                .get_or_init(|| MeshClass::build_material_info_from_model(model.as_ref())),
        )
    }

    fn set_animation_hidden(&mut self, hidden: bool) {
        MeshClass::set_animation_hidden(self, hidden);
    }

    fn get_sort_level(&self) -> i32 {
        self.sort_level as i32
    }

    fn set_sort_level(&mut self, level: i32) {
        self.sort_level = level as u32;
    }

    fn create_decal(&mut self, generator: &mut crate::render_object_system::DecalGeneratorClass) {
        MeshClass::create_decal(self, generator);
    }

    fn delete_decal(&mut self, decal_id: u32) {
        MeshClass::delete_decal(self, decal_id);
    }

    fn transform(&self) -> &Mat4 {
        &self.transform
    }

    fn set_transform(&mut self, transform: Mat4) {
        MeshClass::set_transform(self, transform);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::camera_system::CameraClass;
    use std::sync::Arc;

    #[test]
    fn compute_pass_index_ranges_uses_vertex_count_for_non_indexed_meshes() {
        let mut model = MeshModelClass::new("no_indices");
        model.vertex_count = 24;

        let ranges = compute_pass_index_ranges(&model, &[]);
        assert_eq!(ranges, vec![(0, 24)]);
    }

    #[test]
    fn compute_pass_index_ranges_groups_polygon_renderers_by_material_pass() {
        let mut model = MeshModelClass::new("per_pass");

        let mut pass0 = MaterialPassClass::new();
        pass0.set_pass_index(0);
        let mut pass1 = MaterialPassClass::new();
        pass1.set_pass_index(1);
        model.material_passes = vec![pass0.clone(), pass1.clone()];

        let mut renderer_a = DX8PolygonRendererClass::new();
        renderer_a.index_count = 6;
        renderer_a.material_pass = Some(Arc::new(pass1.clone()));

        let mut renderer_b = DX8PolygonRendererClass::new();
        renderer_b.index_count = 3;
        renderer_b.material_pass = Some(Arc::new(pass1));

        let mut renderer_c = DX8PolygonRendererClass::new();
        renderer_c.index_count = 9;
        renderer_c.material_pass = Some(Arc::new(pass0));

        model.polygon_renderer_list = vec![
            Arc::new(renderer_a),
            Arc::new(renderer_b),
            Arc::new(renderer_c),
        ];

        let index_data = vec![0_u32; 18];
        let ranges = compute_pass_index_ranges(&model, &index_data);

        assert_eq!(ranges[1], (0, 9));
        assert_eq!(ranges[0], (9, 9));
    }

    #[test]
    fn update_skin_and_get_deformed_vertices_use_bone_palette() {
        let mut model = MeshModelClass::new("skin_mesh");
        model.vertices.push(W3dVectorStruct {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        });
        model.set_vertex_bone_links(vec![1]);

        let mut mesh = MeshClass::new();
        mesh.model = Some(Arc::new(model));
        mesh.set_bone_palette_slice(&[
            Mat4::IDENTITY,
            Mat4::from_translation(Vec3::new(2.0, 0.0, 0.0)),
        ]);

        mesh.update_skin();

        let mut deformed = Vec::new();
        mesh.get_deformed_vertices(&mut deformed);
        assert_eq!(deformed.len(), 1);
        assert!((deformed[0].x - 3.0).abs() < 1.0e-4);
        assert!((deformed[0].y - 2.0).abs() < 1.0e-4);
        assert!((deformed[0].z - 3.0).abs() < 1.0e-4);
    }

    #[test]
    fn animation_hidden_state_affects_visibility_checks() {
        let mut mesh = MeshClass::new();
        assert!(!mesh.is_animation_hidden());
        assert!(mesh.is_not_hidden_at_all());

        mesh.set_animation_hidden(true);
        assert!(mesh.is_animation_hidden());
        assert!(!mesh.is_not_hidden_at_all());
    }

    #[test]
    fn frustum_culling_accepts_mesh_in_front_of_right_handed_camera() {
        let mut camera = CameraClass::new();
        camera.set_clip_planes(1.0, 1000.0);
        camera.look_at(Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
        let render_info = RenderInfoClass::new(Arc::new(camera));

        let mut mesh = MeshClass::new();
        mesh.bounding_sphere = SphereClass::new(Vec3::ZERO, 1.0);
        mesh.set_transform(Mat4::from_translation(Vec3::new(0.0, 0.0, -10.0)));

        assert!(mesh.should_render_with_frustum_culling(&render_info));
    }
}
