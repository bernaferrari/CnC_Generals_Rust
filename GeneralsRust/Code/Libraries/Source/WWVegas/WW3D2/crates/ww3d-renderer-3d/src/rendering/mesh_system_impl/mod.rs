//! Mesh Rendering System - Complete implementation matching C++ WW3D2
//!
//! Live `mesh_system` module via `#[path = "mesh_system_impl/mod.rs"]`.
#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    non_snake_case,
    unused_mut,
    unused_assignments,
    clippy::all
)]

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
    OBBoxCollisionResult, OBBoxCollisionTestClass, OBBoxIntersectionTestClass, RayCollisionResult,
    RayCollisionTestClass, RenderInfoClass, RenderInfoOverrideFlags, RenderObjClass,
    StaticSortRenderObject,
};
use crate::rendering::frame_uniform_arena::FrameUniformArena;
use crate::rendering::lighting_system::LightEnvironmentClass;
use crate::rendering::texture_system::texture_base::{TextureAddressMode, TextureFilterMode};
use crate::rendering::wgpu_renderer::wgpu_material_binds::WgpuMaterialBinds;
use crate::rendering::wgpu_renderer::wgpu_pipeline_manager::{
    MAX_TEXTURE_STAGE_GROUPS, MAX_TEXTURE_STAGES, TEXTURES_PER_GROUP, VertexFormat,
    WgpuPipelineManager,
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
use std::sync::{Arc, Mutex, OnceLock, atomic::AtomicU32};
use wgpu::util::DeviceExt;
use wgpu::{
    AddressMode, FilterMode, Origin3d, SamplerDescriptor, TexelCopyBufferLayout,
    TexelCopyTextureInfo, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureViewDescriptor, TextureViewDimension,
};
use ww3d_assets::{
    AssetManager,
    prototypes::{HierarchyPrototype, MeshPrototype},
};
use ww3d_collision::bounding_volumes::obbox::OBBoxClass;
use ww3d_core::w3d_format::{
    W3dTexCoordStruct, W3dTriangleStruct, W3dVectorStruct, W3dVertInfStruct,
};
use ww3d_core::w3d_string_from_bytes;
use ww3d_core::*;
use ww3d_gpu::device::GpuDevice;

mod cast_ray_aligned;
mod collect_billboard_xform;
mod dx8;
mod helpers;
mod materials;
mod mesh;
mod mesh_camera_align;
mod mesh_ops;
mod model;
mod model_collision;
mod render_manager;
mod render_obj;
mod skin_deform;
mod static_sort;

#[cfg(test)]
mod tests;

// Restricted re-exports so impl submodules can `use super::*;`
pub(in crate::rendering::mesh_system) use helpers::*;
pub(in crate::rendering::mesh_system) use materials::*;

pub use dx8::{
    DX8FVFCategoryContainer, DX8PolygonRendererClass, DX8TextureCategoryClass, MeshRenderTask,
};
pub use render_manager::{
    MeshPassTextureProvider, MeshRenderManager, PreparedMeshModel, RenderPassResources,
};
pub use static_sort::{StaticSortEntry, StaticSortFlushGuard, StaticSortManager};

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
    ALIGNED = 8,
    ORIENTED = 16,
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

#[derive(Debug, Clone)]
pub(super) struct DecalRecord {
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

/// Immutable per-mesh fog-of-war scalar state captured by presentation code.
///
/// The renderer receives this with a mesh instance and must not replace it
/// with a live game/FOW query. `visibility_falloff` is deliberately carried
/// unchanged for the model-uniform layout; the current scalar WGSL path does
/// not derive an additional curve from it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrozenFowVisibility {
    pub visibility_alpha: f32,
    pub visibility_falloff: f32,
    pub is_explored: f32,
}

impl FrozenFowVisibility {
    /// Clear/default visibility for standalone WW3D meshes with no game-owned
    /// render-item snapshot.
    pub const CLEAR: Self = Self {
        visibility_alpha: 1.0,
        visibility_falloff: 1.0,
        is_explored: 1.0,
    };

    pub const fn new(visibility_alpha: f32, visibility_falloff: f32, is_explored: f32) -> Self {
        Self {
            visibility_alpha,
            visibility_falloff,
            is_explored,
        }
    }

    /// Field order expected by `WgpuMaterialBinds::model`.
    #[inline]
    pub const fn model_uniform_values(self) -> (f32, f32, f32) {
        (
            self.visibility_alpha,
            self.visibility_falloff,
            self.is_explored,
        )
    }
}

impl Default for FrozenFowVisibility {
    fn default() -> Self {
        Self::CLEAR
    }
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
    /// Presentation-owned instance opacity (C++ Drawable stealth look).
    /// Kept separate from FOW alpha so friendly stealth does not alter the
    /// frozen shroud channel.
    pub presentation_opacity: f32,
    pub material_pass_alpha_override: f32,
    pub material_pass_emissive_override: f32,
    frozen_fow_visibility: FrozenFowVisibility,
    /// Exact presentation-owned `ObjectShroudStatus > Clear` decision. This
    /// is intentionally independent from scalar FOW alpha.
    projected_shroud_eligible: bool,
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

/// Concatenated live sources for residual `include_str!` scans.
pub const MESH_SYSTEM_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("helpers.rs"),
    include_str!("model_collision.rs"),
    include_str!("model.rs"),
    include_str!("dx8.rs"),
    include_str!("mesh.rs"),
    include_str!("mesh_ops.rs"),
    include_str!("static_sort.rs"),
    include_str!("render_manager.rs"),
    include_str!("materials.rs"),
    include_str!("render_obj.rs"),
);
