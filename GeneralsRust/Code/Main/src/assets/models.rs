////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// W3D model loading system for real C&C 3D assets

use crate::assets::archive::ArchiveFileSystem;
use anyhow::{anyhow, Result};
use crc32fast::Hasher;
use glam::{Mat4, Vec3};
use log::{debug, info, warn};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use ww3d_assets::{
    prototypes::{
        AnimationPrototype, HModelPrototype, HierarchyPrototype, HlodPrototype, MaterialPassInfo,
        MeshPrototype, VertexMapperConfig,
    },
    AssetManagerExt as Ww3dAssetManager, W3DLoader as RawW3DLoader,
};
use ww3d_core::w3d_format::{
    w3d_string_from_bytes, W3dMeshHeader3Struct, W3dRGBAStruct, W3dShaderStruct, W3dVertInfStruct,
    W3dVertexMaterialStruct,
};
use ww3d_renderer_3d::rendering::mesh_system::MeshModelClass;

#[allow(dead_code)]
const _MAX_TEXTURE_STAGES: usize = 8;
#[allow(dead_code)]
const _MAX_UV_CHANNELS: usize = 4;

/// W3D file format constants based on C++ w3d_file.h
const W3D_CHUNK_MESH: u32 = 0x00000000;
const W3D_CHUNK_MESH_HEADER: u32 = 0x0000001F; // W3dMeshHeader3Struct
const W3D_CHUNK_VERTICES: u32 = 0x00000002;
const W3D_CHUNK_VERTEX_NORMALS: u32 = 0x00000003;
const W3D_CHUNK_MESH_USER_TEXT: u32 = 0x0000000C;
const W3D_CHUNK_VERTEX_INFLUENCES: u32 = 0x0000000E;
const W3D_CHUNK_TRIANGLES: u32 = 0x00000020;
const W3D_CHUNK_VERTEX_SHADE_INDICES: u32 = 0x00000022;
const W3D_CHUNK_MATERIAL_INFO: u32 = 0x00000028;
const W3D_CHUNK_SHADERS: u32 = 0x00000029;
const W3D_CHUNK_VERTEX_MATERIALS: u32 = 0x0000002A;
const W3D_CHUNK_MATERIALS3: u32 = 0x0000002B;
const W3D_CHUNK_MATERIAL3: u32 = 0x0000002C;
const W3D_CHUNK_MATERIAL3_NAME: u32 = 0x0000002D;
const W3D_CHUNK_MATERIAL3_INFO: u32 = 0x0000002E;
const W3D_CHUNK_MATERIAL3_DC_MAP: u32 = 0x0000002F;
const W3D_CHUNK_MAP3_FILENAME: u32 = 0x0000001A; // FIXED: Was 0x30
const W3D_CHUNK_MAP3_INFO: u32 = 0x0000001B; // FIXED: Was 0x31
const W3D_CHUNK_TEXTURES: u32 = 0x00000030; // FIXED: Was 0x32
const W3D_CHUNK_TEXTURE: u32 = 0x00000031; // FIXED: Was 0x33
const W3D_CHUNK_TEXTURE_NAME: u32 = 0x00000032; // FIXED: Was 0x34
const W3D_CHUNK_TEXTURE_INFO: u32 = 0x00000033; // FIXED: Was 0x35
const W3D_CHUNK_MATERIAL_PASS: u32 = 0x00000038;
const W3D_CHUNK_TEXTURE_STAGE: u32 = 0x00000048;
const W3D_CHUNK_TEXTURE_IDS: u32 = 0x00000049; // NEW: Texture index array

// Additional W3D chunks
const W3D_CHUNK_VERTEX_COLORS: u32 = 0x00000008;
const W3D_CHUNK_TEXCOORDS: u32 = 0x00000005;
const W3D_CHUNK_MATERIALS: u32 = 0x00000028;
const W3D_CHUNK_HIERARCHY: u32 = 0x00000100;
const W3D_CHUNK_ANIMATION: u32 = 0x00000200;
const W3D_CHUNK_HMODEL: u32 = 0x00000300;
const W3D_CHUNK_LODMODEL: u32 = 0x00000400;
const W3D_CHUNK_HLOD: u32 = 0x00000700; // NEW: Hierarchical LOD model

// Mesh types
const W3D_MESH_FLAG_NONE: u32 = 0;
const W3D_MESH_FLAG_HIDDEN: u32 = 0x00000001;
const W3D_MESH_FLAG_TWO_SIDED: u32 = 0x00000002;
const W3D_MESH_FLAG_CAST_SHADOW: u32 = 0x00000004;
const W3D_MESH_FLAG_GEOMETRY_TYPE_MASK: u32 = 0x00FF0000;
const W3D_MESH_FLAG_GEOMETRY_TYPE_NORMAL: u32 = 0x00000000;
const W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ALIGNED: u32 = 0x00010000;
const W3D_MESH_FLAG_GEOMETRY_TYPE_SKIN: u32 = 0x00020000;

/// C++ SAGE engine compatible vertex data - internal format for W3D loading
/// This gets converted to VertexXYZNDUV2 for rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct W3DVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl W3DVertex {
    /// Convert to C++ SAGE VertexFormatXYZNDUV2 format for rendering
    pub fn to_sage_vertex(&self, material_color: Vec3) -> crate::cnc_game_engine::VertexXYZNDUV2 {
        // Pack diffuse color as RGBA bytes (D3D8 style)
        let r = ((self.color[0] * material_color.x * 255.0) as u32).min(255);
        let g = ((self.color[1] * material_color.y * 255.0) as u32).min(255);
        let b = ((self.color[2] * material_color.z * 255.0) as u32).min(255);
        let a = ((self.color[3] * 255.0) as u32).min(255);
        let diffuse_packed = (a << 24) | (r << 16) | (g << 8) | b;

        crate::cnc_game_engine::VertexXYZNDUV2 {
            position: self.position,
            normal: self.normal,
            diffuse: diffuse_packed,
            tex_coords0: self.uv,    // Primary texture coordinates
            tex_coords1: [0.0, 0.0], // Secondary UV for multi-texturing
        }
    }
}

fn w3d_position_to_world(position: [f32; 3]) -> [f32; 3] {
    // Legacy W3D content is authored in X/Y ground with Z-up. The active Rust world
    // uses X/Z ground with Y-up, so swap the vertical and depth axes on import.
    [position[0], position[2], position[1]]
}

fn w3d_normal_to_world(normal: [f32; 3]) -> [f32; 3] {
    [normal[0], normal[2], normal[1]]
}

fn push_world_space_triangle(indices: &mut Vec<u32>, a: u32, b: u32, c: u32) {
    // Swapping Y/Z to move legacy W3D content into Rust's Y-up world flips handedness.
    // Mirror the C++ visible winding by reversing triangle order at import time so
    // backface culling in the WW3D renderer keeps the same observable result.
    indices.push(a);
    indices.push(c);
    indices.push(b);
}

/// W3D material information - matches C++ VertexMaterialClass exactly
#[derive(Debug, Clone)]
pub struct W3DMaterial {
    pub name: String,
    pub diffuse_color: Vec3,  // Color reflected when illuminated by lighting
    pub specular_color: Vec3, // Sharp, concentrated reflective highlights
    pub emissive_color: Vec3, // Self-illumination color (glow)
    pub shininess: f32,       // Specular power (higher = sharper highlights)
    pub opacity: f32,         // Transparency: 1.0 = opaque, 0.0 = transparent
    pub texture_name: Option<String>,

    // C++ VertexMaterialClass multi-stage texture mapping properties
    pub stage0_mapping: TextureStageMapping,
    pub stage1_mapping: Option<TextureStageMapping>,
    pub stage2_mapping: Option<TextureStageMapping>,
    pub stage3_mapping: Option<TextureStageMapping>,

    // BumpEnv vertex material mapping (for normal/bump mapping)
    pub bump_rotation: f32, // Bump texture rotation
    pub bump_scale: f32,    // Bump effect intensity
    pub u_per_sec: f32,     // U coordinate animation speed
    pub v_per_sec: f32,     // V coordinate animation speed
    pub u_scale: f32,       // U coordinate scaling
    pub v_scale: f32,       // V coordinate scaling

    // Shader blending modes for transparency and alpha testing
    pub blend_mode: BlendMode,
    pub alpha_test_enabled: bool,
    pub alpha_reference: f32,
}

/// Texture stage mapping - matches C++ texture stage system
#[derive(Debug, Clone)]
pub struct TextureStageMapping {
    pub texture_name: Option<String>,
    pub uv_source: UVSource, // Which UV set to use
    pub blend_mode: TextureBlendMode,
    pub address_u: TextureAddressMode,
    pub address_v: TextureAddressMode,
    pub min_filter: TextureFilter,
    pub mag_filter: TextureFilter,
    pub mip_filter: TextureFilter,
}

/// UV coordinate source for multi-UV models
#[derive(Debug, Clone, Copy)]
pub enum UVSource {
    UV0, // Primary texture coordinates
    UV1, // Secondary texture coordinates
    UV2, // Tertiary texture coordinates
    UV3, // Quaternary texture coordinates
}

/// Texture blending modes - matches C++ shader blending
#[derive(Debug, Clone, Copy)]
pub enum TextureBlendMode {
    Replace,  // Replace previous stage
    Modulate, // Multiply with previous stage
    Add,      // Add to previous stage
    Subtract, // Subtract from previous stage
    Blend,    // Alpha blend with previous stage
}

/// Material blending modes for transparency
#[derive(Debug, Clone, Copy)]
pub enum BlendMode {
    Opaque,   // No blending (solid)
    Alpha,    // Standard alpha blending
    Additive, // Additive blending (for effects)
    Modulate, // Multiplicative blending
}

/// Texture addressing modes
#[derive(Debug, Clone, Copy)]
pub enum TextureAddressMode {
    Wrap,   // Repeat texture
    Clamp,  // Clamp to edge
    Mirror, // Mirror texture
}

/// Texture filtering modes
#[derive(Debug, Clone, Copy)]
pub enum TextureFilter {
    Point,       // Nearest neighbor
    Linear,      // Linear interpolation
    Anisotropic, // Anisotropic filtering
}

impl Default for W3DMaterial {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            diffuse_color: Vec3::new(1.0, 1.0, 1.0), // Pure white like C++ original
            specular_color: Vec3::new(0.0, 0.0, 0.0), // Black specular like C++ original
            emissive_color: Vec3::ZERO,
            shininess: 0.0, // C++ default shininess
            opacity: 1.0,
            texture_name: None,

            // Default texture stage 0 mapping
            stage0_mapping: TextureStageMapping::default(),
            stage1_mapping: None,
            stage2_mapping: None,
            stage3_mapping: None,

            // Default BumpEnv properties
            bump_rotation: 0.0,
            bump_scale: 1.0,
            u_per_sec: 0.0,
            v_per_sec: 0.0,
            u_scale: 1.0,
            v_scale: 1.0,

            // Default blending
            blend_mode: BlendMode::Opaque,
            alpha_test_enabled: false,
            alpha_reference: 0.5,
        }
    }
}

impl Default for TextureStageMapping {
    fn default() -> Self {
        Self {
            texture_name: None,
            uv_source: UVSource::UV0,
            blend_mode: TextureBlendMode::Replace,
            address_u: TextureAddressMode::Wrap,
            address_v: TextureAddressMode::Wrap,
            min_filter: TextureFilter::Linear,
            mag_filter: TextureFilter::Linear,
            mip_filter: TextureFilter::Linear,
        }
    }
}

/// W3D mesh data
#[derive(Debug, Clone)]
pub struct W3DMesh {
    pub name: String,
    pub vertices: Vec<W3DVertex>,
    pub indices: Vec<u32>,
    pub material: W3DMaterial,
    pub transform: Mat4,
    pub header: Option<W3dMeshHeader3Struct>,
    pub stage_texcoords: Vec<Vec<[f32; 2]>>,
    pub passes: Vec<MaterialPassInfo>,
    pub per_pass_stage_texture_ids: Vec<Vec<Vec<u32>>>,
    pub per_pass_stage_texture_names: Vec<Vec<Vec<String>>>,
    pub per_pass_vertex_material_ids: Vec<Vec<u32>>,
    pub per_pass_shader_ids: Vec<Vec<u32>>,
    pub per_pass_dcg_colors: Vec<Vec<W3dRGBAStruct>>,
    pub per_pass_dig_colors: Vec<Vec<W3dRGBAStruct>>,
    pub vertex_materials: Vec<W3dVertexMaterialStruct>,
    pub shaders: Vec<W3dShaderStruct>,
    pub vertex_influences: Option<Vec<W3dVertInfStruct>>,
    pub vertex_shade_indices: Option<Vec<u32>>,
    pub per_stage_face_texcoord_ids: Vec<Vec<[u32; 3]>>,
    pub stage_uv_channels: Vec<u8>,
    pub texture_library: Vec<String>,
    pub vertex_mappers: Vec<VertexMapperConfig>,
    pub vertices_in_render_space: bool,
    pub has_explicit_vertex_colors: bool,
}

impl W3DMesh {
    pub fn new(name: String) -> Self {
        Self {
            name,
            vertices: Vec::new(),
            indices: Vec::new(),
            material: W3DMaterial::default(),
            transform: Mat4::IDENTITY,
            header: None,
            stage_texcoords: Vec::new(),
            passes: Vec::new(),
            per_pass_stage_texture_ids: Vec::new(),
            per_pass_stage_texture_names: Vec::new(),
            per_pass_vertex_material_ids: Vec::new(),
            per_pass_shader_ids: Vec::new(),
            per_pass_dcg_colors: Vec::new(),
            per_pass_dig_colors: Vec::new(),
            vertex_materials: Vec::new(),
            shaders: Vec::new(),
            vertex_influences: None,
            vertex_shade_indices: None,
            per_stage_face_texcoord_ids: Vec::new(),
            stage_uv_channels: Vec::new(),
            texture_library: Vec::new(),
            vertex_mappers: Vec::new(),
            vertices_in_render_space: false,
            has_explicit_vertex_colors: false,
        }
    }

    pub fn texture_name_from_library(&self, texture_id: u32) -> Option<&str> {
        if texture_id == u32::MAX {
            return None;
        }
        self.texture_library
            .get(texture_id as usize)
            .map(|name| name.as_str())
            .filter(|name| !name.is_empty())
    }

    pub fn stage_texture_names_from_ids(
        &self,
        pass_index: usize,
        stage_index: usize,
    ) -> Vec<String> {
        self.per_pass_stage_texture_ids
            .get(pass_index)
            .and_then(|stages| stages.get(stage_index))
            .map(|ids| {
                ids.iter()
                    .filter_map(|tex_id| self.texture_name_from_library(*tex_id))
                    .map(|name| name.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Complete W3D model
#[derive(Debug, Clone)]
pub struct W3DModel {
    pub name: String,
    pub meshes: Vec<W3DMesh>,
    pub materials: HashMap<String, W3DMaterial>,
    pub texture_names: Vec<String>, // W3D texture definitions loaded from W3D_CHUNK_TEXTURES
    pub ww3d_mesh_models: HashMap<String, Arc<MeshModelClass>>,
    pub bounding_box_min: Vec3,
    pub bounding_box_max: Vec3,
}

impl W3DModel {
    pub fn new(name: String) -> Self {
        Self {
            name,
            meshes: Vec::new(),
            materials: HashMap::new(),
            texture_names: Vec::new(),
            ww3d_mesh_models: HashMap::new(),
            bounding_box_min: Vec3::splat(f32::MAX),
            bounding_box_max: Vec3::splat(f32::MIN),
        }
    }

    pub fn calculate_bounding_box(&mut self) {
        self.bounding_box_min = Vec3::splat(f32::MAX);
        self.bounding_box_max = Vec3::splat(f32::MIN);

        for mesh in &self.meshes {
            for vertex in &mesh.vertices {
                let pos = Vec3::from_array(vertex.position);
                self.bounding_box_min = self.bounding_box_min.min(pos);
                self.bounding_box_max = self.bounding_box_max.max(pos);
            }
        }
    }
}

/// W3D model loader
pub struct W3DLoader;

impl Default for W3DLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DLoader {
    /// Create new W3D loader
    pub fn new() -> Self {
        Self
    }

    /// Load W3D model from BIG archive
    pub async fn load_model(
        &self,
        archive_system: &mut ArchiveFileSystem,
        model_name: &str,
    ) -> Result<W3DModel> {
        debug!("Loading W3D model: {}", model_name);

        // Try to find the W3D file in archives - handle full paths vs. just filenames
        let w3d_filename = if model_name.to_ascii_lowercase().ends_with(".w3d") {
            model_name.to_string()
        } else {
            format!("{}.w3d", model_name)
        };
        let filename_variants = Self::w3d_filename_variants(&w3d_filename);

        // Try multiple path variations since files are stored with full paths in BIG files.
        // Prefer `art/w3d` first because that's where the vast majority of shipped assets live.
        let mut path_variations = Vec::new();
        let mut seen = HashSet::new();
        for filename in filename_variants {
            for candidate in [
                format!("art/w3d/{}", filename),
                format!("Art/W3D/{}", filename),
                format!("ART/W3D/{}", filename),
                filename.clone(),
                format!("data/w3d/{}", filename),
                format!("Data/W3D/{}", filename),
                format!("models/{}", filename),
                format!("Models/{}", filename),
            ] {
                let key = candidate.to_ascii_lowercase();
                if seen.insert(key) {
                    path_variations.push(candidate);
                }
            }
        }

        let mut last_error = None;
        for path_variant in &path_variations {
            debug!("Trying W3D path: {}", path_variant);

            // Add timeout to file loading to prevent hangs
            let file_timeout = tokio::time::Duration::from_secs(5);
            match tokio::time::timeout(file_timeout, archive_system.open_file(path_variant)).await {
                Ok(Ok(model_data)) => {
                    debug!("Found W3D file at path: {}", path_variant);
                    debug!("Loaded W3D file data: {} bytes", model_data.len());
                    return self
                        .parse_w3d_data_with_companions(
                            archive_system,
                            path_variant,
                            &model_data,
                            model_name.to_string(),
                        )
                        .await;
                }
                Ok(Err(e)) => {
                    debug!("❌ Failed to find W3D at {}: {}", path_variant, e);
                    last_error = Some(e);
                }
                Err(_) => {
                    warn!("⏰ File loading timeout for path: {}", path_variant);
                    last_error = Some(anyhow!("File loading timeout"));
                }
            }
        }

        // If all variations failed, return the last error
        Err(anyhow!(
            "Failed to load W3D file {} from any path variation: {}",
            w3d_filename,
            last_error.unwrap_or_else(|| anyhow!("No paths tried"))
        ))
    }

    /// Parse W3D binary data using the preferred ww3d-assets pipeline, with legacy fallback.
    fn parse_w3d_data(&self, data: &[u8], model_name: String) -> Result<W3DModel> {
        match self.parse_with_ww3d_assets(data, &model_name) {
            Ok(model) => Ok(model),
            Err(err) => {
                warn!(
                    "ww3d-assets parser failed for '{}': {} - falling back to legacy parser",
                    model_name, err
                );
                self.parse_w3d_data_legacy(data, model_name)
            }
        }
    }

    async fn parse_w3d_data_with_companions(
        &self,
        archive_system: &mut ArchiveFileSystem,
        resolved_path: &str,
        data: &[u8],
        model_name: String,
    ) -> Result<W3DModel> {
        let primary_result = self.parse_with_ww3d_assets(data, &model_name);
        let companion_sources = self
            .load_companion_w3d_sources(archive_system, resolved_path)
            .await;

        if !companion_sources.is_empty() {
            let mut sources: Vec<(&[u8], &str)> = Vec::with_capacity(companion_sources.len() + 1);
            sources.push((data, model_name.as_str()));
            for (bytes, stem) in &companion_sources {
                sources.push((bytes.as_slice(), stem.as_str()));
            }
            match self.parse_with_ww3d_assets_sources(&sources, &model_name) {
                Ok(family_model) => {
                    let family_score = Self::model_family_richness_score(&family_model);
                    if let Ok(primary_model) = primary_result {
                        let primary_score = Self::model_family_richness_score(&primary_model);
                        if family_score > primary_score {
                            let mut family_model = family_model;
                            self.enrich_model_textures_from_legacy(
                                data,
                                &model_name,
                                &mut family_model,
                            );
                            let mut family_model = family_model;
                            self.enrich_model_textures_from_legacy(
                                data,
                                &model_name,
                                &mut family_model,
                            );
                            return Ok(family_model);
                        }
                        let mut primary_model = primary_model;
                        self.enrich_model_textures_from_legacy(
                            data,
                            &model_name,
                            &mut primary_model,
                        );
                        return Ok(primary_model);
                    }
                    return Ok(family_model);
                }
                Err(companion_err) => {
                    warn!(
                        "ww3d-assets family parse failed for '{}': {}",
                        model_name, companion_err
                    );

                    let mut best_companion: Option<W3DModel> = None;
                    let mut best_score = 0usize;
                    for (bytes, stem) in &companion_sources {
                        let parsed =
                            self.parse_with_ww3d_assets(bytes.as_slice(), stem)
                                .or_else(|_| {
                                    self.parse_w3d_data_legacy(bytes.as_slice(), stem.clone())
                                });
                        let Ok(mut parsed) = parsed else {
                            continue;
                        };
                        self.enrich_model_textures_from_legacy(bytes.as_slice(), stem, &mut parsed);
                        let score = Self::model_family_richness_score(&parsed);
                        if score > best_score {
                            best_score = score;
                            best_companion = Some(parsed);
                        }
                    }

                    if let Some(mut companion_model) = best_companion {
                        companion_model.name = model_name.clone();
                        return Ok(companion_model);
                    }
                }
            }
        }

        match primary_result {
            Ok(mut model) => {
                self.enrich_model_textures_from_legacy(data, &model_name, &mut model);
                Ok(model)
            }
            Err(primary_err) => {
                warn!(
                    "ww3d-assets parser failed for '{}': {} - falling back to legacy parser",
                    model_name, primary_err
                );
                self.parse_w3d_data_legacy(data, model_name)
            }
        }
    }

    fn model_texture_reference_score(model: &W3DModel) -> usize {
        let stage_refs = model
            .meshes
            .iter()
            .flat_map(|mesh| mesh.per_pass_stage_texture_names.iter())
            .flat_map(|stages| stages.iter())
            .flat_map(|names| names.iter())
            .filter(|name| !name.is_empty())
            .count();
        let material_refs = model
            .meshes
            .iter()
            .filter(|mesh| {
                mesh.material
                    .texture_name
                    .as_ref()
                    .is_some_and(|name| !name.is_empty())
            })
            .count();
        stage_refs + material_refs + model.texture_names.len()
    }

    fn enrich_model_textures_from_legacy(
        &self,
        data: &[u8],
        model_name: &str,
        model: &mut W3DModel,
    ) {
        if Self::model_texture_reference_score(model) > 0 {
            return;
        }
        let Ok(legacy) = self.parse_w3d_data_legacy(data, model_name.to_string()) else {
            return;
        };
        if Self::model_texture_reference_score(&legacy) == 0 {
            return;
        }

        for texture_name in legacy.texture_names {
            if !model
                .texture_names
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&texture_name))
            {
                model.texture_names.push(texture_name);
            }
        }

        let mut legacy_meshes = std::collections::HashMap::new();
        for mesh in legacy.meshes {
            legacy_meshes.insert(mesh.name.to_ascii_lowercase(), mesh);
        }

        for mesh in &mut model.meshes {
            let Some(legacy_mesh) = legacy_meshes.get(&mesh.name.to_ascii_lowercase()) else {
                continue;
            };
            if mesh.material.texture_name.is_none()
                && legacy_mesh
                    .material
                    .texture_name
                    .as_ref()
                    .is_some_and(|name| !name.is_empty())
            {
                mesh.material.texture_name = legacy_mesh.material.texture_name.clone();
                if mesh.material.stage0_mapping.texture_name.is_none() {
                    mesh.material.stage0_mapping.texture_name =
                        legacy_mesh.material.texture_name.clone();
                }
            }
            if mesh.texture_library.is_empty() && !legacy_mesh.texture_library.is_empty() {
                mesh.texture_library = legacy_mesh.texture_library.clone();
            }
            if mesh.per_pass_stage_texture_names.is_empty()
                && !legacy_mesh.per_pass_stage_texture_names.is_empty()
            {
                mesh.per_pass_stage_texture_names =
                    legacy_mesh.per_pass_stage_texture_names.clone();
            }
        }

        if Self::model_texture_reference_score(model) == 0 {
            self.enrich_model_textures_from_embedded_names(data, model_name, model);
        }
    }

    fn model_family_richness_score(model: &W3DModel) -> usize {
        let textured_meshes = model
            .meshes
            .iter()
            .filter(|mesh| {
                mesh.material.texture_name.is_some()
                    || mesh
                        .per_pass_stage_texture_names
                        .iter()
                        .flat_map(|stages| stages.iter())
                        .flat_map(|names| names.iter())
                        .any(|name| !name.is_empty())
            })
            .count();
        model.meshes.len() * 1000 + textured_meshes * 10 + model.ww3d_mesh_models.len()
    }

    fn enrich_model_textures_from_embedded_names(
        &self,
        data: &[u8],
        model_name: &str,
        model: &mut W3DModel,
    ) {
        let embedded_names = Self::extract_embedded_texture_names(data);
        if embedded_names.is_empty() {
            return;
        }

        for texture_name in &embedded_names {
            if !model
                .texture_names
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(texture_name))
            {
                model.texture_names.push(texture_name.clone());
            }
        }

        for mesh in &mut model.meshes {
            if mesh.texture_library.is_empty() {
                mesh.texture_library = embedded_names.clone();
            }
            if mesh.per_pass_stage_texture_names.is_empty() && !embedded_names.is_empty() {
                mesh.per_pass_stage_texture_names = vec![vec![embedded_names.clone()]];
            }
            if mesh.material.texture_name.is_none() {
                if let Some(texture_name) =
                    Self::select_embedded_texture_for_mesh(model_name, &mesh.name, &embedded_names)
                {
                    mesh.material.texture_name = Some(texture_name.clone());
                    if mesh.material.stage0_mapping.texture_name.is_none() {
                        mesh.material.stage0_mapping.texture_name = Some(texture_name);
                    }
                }
            }
        }
    }

    fn extract_embedded_texture_names(data: &[u8]) -> Vec<String> {
        fn is_name_byte(byte: u8) -> bool {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'/' | b'\\' | b'-')
        }

        fn normalize_candidate(candidate: &str) -> Option<String> {
            let trimmed = candidate.trim_matches(char::from(0)).trim();
            if trimmed.len() < 5 {
                return None;
            }
            let lower = trimmed.to_ascii_lowercase();
            if !(lower.ends_with(".tga") || lower.ends_with(".dds")) {
                return None;
            }
            if !trimmed
                .bytes()
                .all(|byte| byte.is_ascii_graphic() || byte == b' ')
            {
                return None;
            }
            Some(trimmed.replace('\\', "/"))
        }

        let mut names = Vec::new();
        let mut seen = HashSet::new();
        let mut start = 0usize;

        while start < data.len() {
            while start < data.len() && !is_name_byte(data[start]) {
                start += 1;
            }
            let mut end = start;
            while end < data.len() && is_name_byte(data[end]) {
                end += 1;
            }
            if end > start {
                if let Ok(candidate) = std::str::from_utf8(&data[start..end]) {
                    if let Some(name) = normalize_candidate(candidate) {
                        let key = name.to_ascii_lowercase();
                        if seen.insert(key) {
                            names.push(name);
                        }
                    }
                }
            }
            start = end.saturating_add(1);
        }

        names
    }

    fn select_embedded_texture_for_mesh(
        model_name: &str,
        mesh_name: &str,
        texture_names: &[String],
    ) -> Option<String> {
        if texture_names.is_empty() {
            return None;
        }

        let mesh_lower = mesh_name.to_ascii_lowercase();
        let model_lower = model_name.to_ascii_lowercase();
        let find_match = |needle: &str| {
            texture_names
                .iter()
                .find(|name| name.to_ascii_lowercase().contains(needle))
                .cloned()
        };

        if mesh_lower.contains("housecolor") {
            if let Some(name) = find_match("housecolor") {
                return Some(name);
            }
        }
        if mesh_lower.contains("light") || mesh_lower.contains("radar") {
            if let Some(name) = find_match("light") {
                return Some(name);
            }
        }
        for token in [
            "chassis",
            "turret",
            "barrel",
            "props",
            "propeller",
            "bunker",
        ] {
            if mesh_lower.contains(token) {
                if let Some(name) = find_match(&model_lower) {
                    return Some(name);
                }
            }
        }

        find_match(&model_lower).or_else(|| texture_names.first().cloned())
    }

    fn w3d_filename_variants(w3d_filename: &str) -> Vec<String> {
        let mut variants = Vec::new();
        let mut seen = HashSet::new();
        let mut push_variant = |candidate: String| {
            let key = candidate.to_ascii_lowercase();
            if seen.insert(key) {
                variants.push(candidate);
            }
        };

        push_variant(w3d_filename.to_string());

        if let Some(base) = w3d_filename.strip_suffix(".w3d") {
            push_variant(format!("{base}.W3D"));
        } else if let Some(base) = w3d_filename.strip_suffix(".W3D") {
            push_variant(format!("{base}.w3d"));
        }

        push_variant(w3d_filename.to_ascii_lowercase());
        push_variant(w3d_filename.to_ascii_uppercase());

        variants
    }

    async fn load_companion_w3d_sources(
        &self,
        archive_system: &mut ArchiveFileSystem,
        resolved_path: &str,
    ) -> Vec<(Vec<u8>, String)> {
        let mut sources = Vec::new();
        let mut seen = HashSet::new();
        let path = Path::new(resolved_path);
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            return sources;
        };
        let Some(stem) = file_name
            .strip_suffix(".w3d")
            .or_else(|| file_name.strip_suffix(".W3D"))
        else {
            return sources;
        };

        let parent = path
            .parent()
            .map(|parent| parent.to_string_lossy().to_string());
        for companion_stem in companion_stem_variants(stem) {
            if !seen.insert(companion_stem.to_ascii_lowercase()) {
                continue;
            }
            let companion_file = format!("{companion_stem}.w3d");
            for candidate in Self::w3d_filename_variants(&companion_file) {
                let path = if let Some(parent) = &parent {
                    format!("{parent}/{candidate}")
                } else {
                    candidate
                };
                if let Ok(bytes) = archive_system.open_file(&path).await {
                    sources.push((bytes, companion_stem.clone()));
                    break;
                }
                if let Some(bytes) = Self::load_loose_companion_w3d_bytes(&path) {
                    sources.push((bytes, companion_stem.clone()));
                    break;
                }
            }
        }

        sources
    }

    fn load_loose_companion_w3d_bytes(resolved_path: &str) -> Option<Vec<u8>> {
        let normalized = resolved_path.replace('\\', "/");
        let lowered = normalized.to_ascii_lowercase();
        let candidate_suffixes = [
            lowered
                .strip_prefix("art/w3d/")
                .map(|tail| PathBuf::from("Art/W3D").join(tail)),
            lowered
                .strip_prefix("data/english/art/w3d/")
                .map(|tail| PathBuf::from("Data/English/Art/W3D").join(tail)),
            lowered
                .strip_prefix("data/w3d/")
                .map(|tail| PathBuf::from("Data/W3D").join(tail)),
        ];

        let roots = [
            PathBuf::from("."),
            PathBuf::from(".."),
            PathBuf::from("../windows_game/extracted_big_files_v2/W3DZH"),
            PathBuf::from("../windows_game/extracted_big_files/W3DZH"),
            PathBuf::from("../windows_game/extracted_big_files_v2/W3DEnglishZH"),
            PathBuf::from("../windows_game/extracted_big_files/W3DEnglishZH"),
            PathBuf::from("../windows_game/Command & Conquer Generals Zero Hour"),
            PathBuf::from("/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main/windows_game/extracted_big_files_v2/W3DZH"),
            PathBuf::from("/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main/windows_game/extracted_big_files/W3DZH"),
            PathBuf::from("/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main/windows_game/extracted_big_files_v2/W3DEnglishZH"),
            PathBuf::from("/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main/windows_game/extracted_big_files/W3DEnglishZH"),
        ];

        for suffix in candidate_suffixes.into_iter().flatten() {
            for root in &roots {
                let candidate = root.join(&suffix);
                if let Ok(bytes) = std::fs::read(&candidate) {
                    return Some(bytes);
                }
            }
        }

        None
    }

    fn parse_with_ww3d_assets(&self, data: &[u8], model_name: &str) -> Result<W3DModel> {
        self.parse_with_ww3d_assets_sources(&[(data, model_name)], model_name)
    }

    fn parse_with_ww3d_assets_sources(
        &self,
        sources: &[(&[u8], &str)],
        model_name: &str,
    ) -> Result<W3DModel> {
        let mut raw_hlod_names = Vec::new();
        let mut raw_hierarchy_names = Vec::new();
        let mut raw_model = W3DModel::new(model_name.to_string());
        let mut raw_mesh_count = 0usize;
        for (source_data, _) in sources {
            if let Ok(parsed) = RawW3DLoader::load_from_bytes(source_data) {
                raw_hlod_names.extend(parsed.hlods.iter().map(|hlod| hlod.name.clone()));
                raw_hierarchy_names.extend(
                    parsed
                        .hierarchies
                        .iter()
                        .map(|hier| hier.header.name.clone()),
                );
                for mesh in &parsed.meshes {
                    self.append_mesh_from_raw_loader(mesh, &mut raw_model)?;
                    raw_mesh_count += 1;
                }
            }
        }
        if raw_mesh_count > 0 {
            raw_model.calculate_bounding_box();
            return Ok(raw_model);
        }

        let mut manager = Ww3dAssetManager::new();
        for (bytes, source_name) in sources {
            manager
                .load_w3d_from_bytes(bytes, source_name)
                .map_err(|e| anyhow!("ww3d-assets parse error in '{}': {e:?}", source_name))?;
        }

        let mut model = W3DModel::new(model_name.to_string());
        let mut mesh_count = 0usize;
        let prototype_names: Vec<String> = manager.prototype_names().cloned().collect();
        for name in prototype_names {
            if let Some(proto) = manager.find_prototype(&name) {
                if let Some(mesh_proto) = proto.as_any().downcast_ref::<MeshPrototype>() {
                    self.append_mesh_from_prototype(mesh_proto, &mut model)?;
                    mesh_count += 1;
                }
            }
        }

        if mesh_count == 0 {
            let prototype_names: Vec<String> = manager.prototype_names().cloned().collect();
            warn!(
                "ww3d-assets prototype flattening found no direct meshes for '{}'; raw_hlods={:?} raw_hierarchies={:?} prototypes={:?}",
                model_name,
                raw_hlod_names,
                raw_hierarchy_names,
                prototype_names
            );
            let maybe_proto = manager.find_prototype(model_name).or_else(|| {
                companion_root_proto_name(&prototype_names, model_name)
                    .and_then(|candidate| manager.find_prototype(&candidate))
            });
            warn!(
                "ww3d-assets top-level prototype lookup for '{}' found={}",
                model_name,
                maybe_proto.is_some()
            );
            if let Some(proto) = maybe_proto {
                let mut recursion_stack = HashSet::new();
                mesh_count = self.append_render_obj_prototype(
                    proto.as_ref(),
                    &manager,
                    Mat4::IDENTITY,
                    &mut recursion_stack,
                    &mut model,
                )?;
            }
        }

        if mesh_count == 0 {
            return Err(anyhow!(
                "ww3d-assets: no mesh prototypes found in '{}'",
                model_name
            ));
        }

        model.calculate_bounding_box();
        Ok(model)
    }

    fn append_render_obj_prototype(
        &self,
        proto: &dyn ww3d_assets::assets::Prototype,
        manager: &Ww3dAssetManager,
        transform: Mat4,
        recursion_stack: &mut HashSet<String>,
        model: &mut W3DModel,
    ) -> Result<usize> {
        let proto_name = proto.name().to_ascii_lowercase();
        if !recursion_stack.insert(proto_name.clone()) {
            return Ok(0);
        }

        let appended = if let Some(mesh_proto) = proto.as_any().downcast_ref::<MeshPrototype>() {
            if mesh_proto.vertices.is_empty() {
                warn!(
                    "flattening MeshPrototype '{}' has no vertices; triangles={} stage_texcoords={} textures={}",
                    mesh_proto.name,
                    mesh_proto.triangles.len(),
                    mesh_proto.stage_texcoords.len(),
                    mesh_proto.textures.len()
                );
            }
            let before = model.meshes.len();
            self.append_mesh_from_prototype(mesh_proto, model)?;
            for mesh in &mut model.meshes[before..] {
                mesh.transform = transform * mesh.transform;
            }
            model.meshes.len().saturating_sub(before)
        } else if let Some(hlod_proto) = proto.as_any().downcast_ref::<HlodPrototype>() {
            warn!(
                "flattening HLOD '{}' hierarchy='{}' lods={:?} aggregates={:?}",
                hlod_proto.name,
                hlod_proto.hierarchy_name,
                hlod_proto
                    .lods
                    .iter()
                    .map(|lod| {
                        (
                            lod.max_screen_size,
                            lod.models
                                .iter()
                                .map(|m| format!("{}@{}", m.name, m.bone_index))
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>(),
                hlod_proto
                    .aggregates
                    .iter()
                    .map(|agg| {
                        (
                            agg.max_screen_size,
                            agg.models
                                .iter()
                                .map(|m| format!("{}@{}", m.name, m.bone_index))
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>()
            );
            self.append_hlod_prototype(hlod_proto, manager, transform, recursion_stack, model)?
        } else if let Some(hmodel_proto) = proto.as_any().downcast_ref::<HModelPrototype>() {
            warn!(
                "flattening HModel '{}' hierarchy='{}' nodes={:?}",
                hmodel_proto.name,
                hmodel_proto.hierarchy_name,
                hmodel_proto
                    .nodes
                    .iter()
                    .map(|node| node.render_obj_name.clone())
                    .collect::<Vec<_>>()
            );
            self.append_hmodel_prototype(hmodel_proto, manager, transform, recursion_stack, model)?
        } else if let Some(anim_proto) = proto.as_any().downcast_ref::<AnimationPrototype>() {
            self.append_animation_family_prototype(
                anim_proto,
                manager,
                transform,
                recursion_stack,
                model,
            )?
        } else {
            warn!(
                "flattening unsupported prototype '{}' class_id={:?} debug={:?}",
                proto.name(),
                proto.class_id(),
                proto
            );
            0
        };

        recursion_stack.remove(&proto_name);
        Ok(appended)
    }

    fn append_animation_family_prototype(
        &self,
        anim_proto: &AnimationPrototype,
        manager: &Ww3dAssetManager,
        transform: Mat4,
        recursion_stack: &mut HashSet<String>,
        model: &mut W3DModel,
    ) -> Result<usize> {
        let mut candidates = Vec::new();
        let mut seen = HashSet::new();
        let prototype_names: Vec<String> = manager.prototype_names().cloned().collect();
        let push_candidate =
            |name: &str, candidates: &mut Vec<String>, seen: &mut HashSet<String>| {
                if name.is_empty() {
                    return;
                }
                let lowered = name.to_ascii_lowercase();
                if seen.insert(lowered) {
                    candidates.push(name.to_string());
                }
            };

        push_candidate(&anim_proto.hierarchy_name, &mut candidates, &mut seen);
        push_candidate(&anim_proto.name, &mut candidates, &mut seen);

        for name in [&anim_proto.hierarchy_name, &anim_proto.name] {
            if let Some(candidate) = companion_root_proto_name(&prototype_names, name) {
                push_candidate(&candidate, &mut candidates, &mut seen);
            }
            for variant in companion_stem_variants(name) {
                push_candidate(&variant, &mut candidates, &mut seen);
            }
        }

        let companion_prefixed = collect_companion_prefixed_prototypes(
            &prototype_names,
            &[anim_proto.hierarchy_name.as_str(), anim_proto.name.as_str()],
        );

        let mut appended = 0usize;
        for candidate in candidates {
            let Some(proto) = manager.find_prototype(&candidate) else {
                continue;
            };
            if proto
                .as_any()
                .downcast_ref::<AnimationPrototype>()
                .is_some()
                && proto.name().eq_ignore_ascii_case(&anim_proto.name)
            {
                continue;
            }
            appended += self.append_render_obj_prototype(
                proto.as_ref(),
                manager,
                transform,
                recursion_stack,
                model,
            )?;
            if appended > 0 {
                break;
            }
        }

        if appended == 0 {
            for candidate in companion_prefixed {
                let Some(proto) = manager.find_prototype(&candidate) else {
                    continue;
                };
                if proto
                    .as_any()
                    .downcast_ref::<AnimationPrototype>()
                    .is_some()
                {
                    continue;
                }
                appended += self.append_render_obj_prototype(
                    proto.as_ref(),
                    manager,
                    transform,
                    recursion_stack,
                    model,
                )?;
            }
        }

        if appended == 0 {
            warn!(
                "flattening animation prototype '{}' hierarchy='{}' found no renderable companion",
                anim_proto.name, anim_proto.hierarchy_name
            );
        }

        Ok(appended)
    }

    fn append_hlod_prototype(
        &self,
        hlod_proto: &HlodPrototype,
        manager: &Ww3dAssetManager,
        transform: Mat4,
        recursion_stack: &mut HashSet<String>,
        model: &mut W3DModel,
    ) -> Result<usize> {
        let hierarchy_transforms =
            self.lookup_hierarchy_bind_transforms(manager, &hlod_proto.hierarchy_name);

        let mut appended = 0usize;

        if let Some(primary_lod) = hlod_proto.lods.first() {
            for sub_object in &primary_lod.models {
                appended += self.append_named_sub_object(
                    &sub_object.name,
                    sub_object.bone_index as usize,
                    hierarchy_transforms.as_deref(),
                    manager,
                    transform,
                    recursion_stack,
                    model,
                )?;
            }
        }

        for aggregate in &hlod_proto.aggregates {
            for sub_object in &aggregate.models {
                appended += self.append_named_sub_object(
                    &sub_object.name,
                    sub_object.bone_index as usize,
                    hierarchy_transforms.as_deref(),
                    manager,
                    transform,
                    recursion_stack,
                    model,
                )?;
            }
        }

        Ok(appended)
    }

    fn append_hmodel_prototype(
        &self,
        hmodel_proto: &HModelPrototype,
        manager: &Ww3dAssetManager,
        transform: Mat4,
        recursion_stack: &mut HashSet<String>,
        model: &mut W3DModel,
    ) -> Result<usize> {
        let hierarchy_transforms =
            self.lookup_hierarchy_bind_transforms(manager, &hmodel_proto.hierarchy_name);

        let mut appended = 0usize;
        for node in &hmodel_proto.nodes {
            appended += self.append_named_sub_object(
                &node.render_obj_name,
                node.pivot_idx as usize,
                hierarchy_transforms.as_deref(),
                manager,
                transform,
                recursion_stack,
                model,
            )?;
        }

        Ok(appended)
    }

    fn append_named_sub_object(
        &self,
        sub_object_name: &str,
        bone_index: usize,
        hierarchy_transforms: Option<&[Mat4]>,
        manager: &Ww3dAssetManager,
        parent_transform: Mat4,
        recursion_stack: &mut HashSet<String>,
        model: &mut W3DModel,
    ) -> Result<usize> {
        let child_transform =
            parent_transform * Self::hierarchy_transform_for_bone(hierarchy_transforms, bone_index);
        let Some(proto) = manager.find_prototype(sub_object_name) else {
            return Ok(0);
        };
        self.append_render_obj_prototype(
            proto.as_ref(),
            manager,
            child_transform,
            recursion_stack,
            model,
        )
    }

    fn lookup_hierarchy_bind_transforms(
        &self,
        manager: &Ww3dAssetManager,
        hierarchy_name: &str,
    ) -> Option<Vec<Mat4>> {
        if hierarchy_name.is_empty() {
            return None;
        }
        let proto = manager.find_prototype(hierarchy_name)?;
        let hierarchy = proto.as_any().downcast_ref::<HierarchyPrototype>()?;
        if hierarchy.bind_transforms.is_empty() {
            None
        } else {
            Some(hierarchy.bind_transforms.clone())
        }
    }

    fn hierarchy_transform_for_bone(
        hierarchy_transforms: Option<&[Mat4]>,
        bone_index: usize,
    ) -> Mat4 {
        let Some(hierarchy_transforms) = hierarchy_transforms else {
            return Mat4::IDENTITY;
        };

        hierarchy_transforms
            .get(bone_index)
            .copied()
            .unwrap_or(Mat4::IDENTITY)
    }

    fn append_mesh_from_raw_loader(
        &self,
        raw_mesh: &ww3d_assets::loaders::mesh_loader::W3DMesh,
        model: &mut W3DModel,
    ) -> Result<()> {
        if raw_mesh.vertices.is_empty() {
            return Ok(());
        }

        let mut mesh = W3DMesh::new(if raw_mesh.header.mesh_name.is_empty() {
            "unnamed_mesh".to_string()
        } else {
            raw_mesh.header.mesh_name.clone()
        });

        mesh.stage_uv_channels = vec![0];
        if !raw_mesh.tex_coords.is_empty() {
            mesh.stage_texcoords.push(
                raw_mesh
                    .tex_coords
                    .iter()
                    .map(|tc| [tc.x, tc.y])
                    .collect::<Vec<_>>(),
            );
        }

        mesh.texture_library = raw_mesh
            .textures
            .iter()
            .map(|tex| tex.name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect();
        if mesh.texture_library.is_empty() && !model.texture_names.is_empty() {
            mesh.texture_library = model.texture_names.clone();
        }

        for (index, position) in raw_mesh.vertices.iter().enumerate() {
            let position = w3d_position_to_world([position.x, position.y, position.z]);
            let normal = raw_mesh
                .normals
                .get(index)
                .map(|n| w3d_normal_to_world([n.x, n.y, n.z]))
                .unwrap_or([0.0, 1.0, 0.0]);
            let uv = raw_mesh
                .tex_coords
                .get(index)
                .map(|tc| [tc.x, tc.y])
                .unwrap_or([0.0, 0.0]);

            mesh.vertices.push(W3DVertex {
                position,
                normal,
                uv,
                color: [1.0, 1.0, 1.0, 1.0],
            });
        }
        mesh.vertices_in_render_space = true;
        mesh.has_explicit_vertex_colors = false;

        for tri in &raw_mesh.triangles {
            push_world_space_triangle(&mut mesh.indices, tri[0], tri[1], tri[2]);
        }

        mesh.per_pass_shader_ids = raw_mesh
            .material_passes
            .iter()
            .map(|pass| pass.shader_ids.clone())
            .collect();
        mesh.per_pass_stage_texture_names = raw_mesh
            .material_passes
            .iter()
            .map(|pass| {
                pass.texture_stages
                    .iter()
                    .map(|stage| {
                        stage
                            .texture_ids
                            .iter()
                            .filter_map(|texture_id| {
                                raw_mesh
                                    .textures
                                    .get(*texture_id as usize)
                                    .map(|tex| tex.name.clone())
                            })
                            .filter(|name| !name.is_empty())
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        let mut material = W3DMaterial::default();
        material.name = mesh.name.clone();
        if let Some(texture_name) = mesh
            .per_pass_stage_texture_names
            .iter()
            .flat_map(|stages| stages.iter())
            .flat_map(|names| names.iter())
            .find(|name| !name.is_empty())
            .cloned()
            .or_else(|| mesh.texture_library.first().cloned())
            .or_else(|| model.texture_names.first().cloned())
        {
            material.texture_name = Some(texture_name.clone());
            material.stage0_mapping.texture_name = Some(texture_name);
        } else if let Some(stage0) = raw_mesh
            .vertex_materials
            .first()
            .and_then(|vm| vm.stage0.clone())
        {
            material.texture_name = Some(stage0.clone());
            material.stage0_mapping.texture_name = Some(stage0);
        }

        Self::apply_material_stage_mappings(&mut material, &mesh);
        mesh.material = material.clone();

        if !material.name.is_empty() {
            model
                .materials
                .entry(material.name.clone())
                .or_insert_with(|| material.clone());
        }

        if let Some(tex_name) = mesh.material.texture_name.clone() {
            if !model
                .texture_names
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&tex_name))
            {
                model.texture_names.push(tex_name);
            }
        }

        model.meshes.push(mesh);
        Ok(())
    }

    fn append_mesh_from_prototype(
        &self,
        proto: &MeshPrototype,
        model: &mut W3DModel,
    ) -> Result<()> {
        if proto.vertices.is_empty() {
            return Ok(());
        }

        let mut mesh = W3DMesh::new(proto.name.clone());
        mesh.header = proto.header.clone();
        mesh.vertex_materials = proto.vertex_materials.clone();
        mesh.shaders = proto.shaders.clone();
        let raw_stage_uvs: Vec<Vec<[f32; 2]>> = proto
            .stage_texcoords
            .iter()
            .map(|coords| coords.iter().map(|tc| [tc.u, tc.v]).collect())
            .collect();
        let (uv_layers, stage_channels) = deduplicate_stage_uv_layers(raw_stage_uvs);
        mesh.stage_texcoords = uv_layers;
        mesh.stage_uv_channels = stage_channels;
        mesh.texture_library = proto
            .textures
            .iter()
            .map(|tex| w3d_string_from_bytes(&tex.name).trim().to_string())
            .filter(|name| !name.is_empty())
            .collect();
        if mesh.texture_library.is_empty() && !model.texture_names.is_empty() {
            mesh.texture_library = model.texture_names.clone();
        }
        mesh.passes = proto.passes.clone();
        mesh.per_pass_stage_texture_ids = proto.per_pass_stage_texture_ids.clone();
        mesh.per_pass_stage_texture_names = Self::resolve_stage_texture_names(proto);
        mesh.per_pass_vertex_material_ids = proto.per_pass_vertex_material_ids.clone();
        mesh.per_pass_shader_ids = proto.per_pass_shader_ids.clone();
        mesh.per_pass_dcg_colors = proto.per_pass_dcg_colors.clone();
        mesh.per_pass_dig_colors = proto.per_pass_dig_colors.clone();
        mesh.vertex_mappers = proto.vertex_mapper_configs.clone();
        mesh.per_stage_face_texcoord_ids = proto.per_face_texcoord_ids.clone();
        mesh.vertex_influences = proto.vertex_influences.clone();
        mesh.vertex_shade_indices = proto.vertex_shade_indices.clone();
        mesh.has_explicit_vertex_colors = false;
        let default_normal = Vec3::Y;
        let texcoords = proto.stage_texcoords.get(0);

        for (index, position) in proto.vertices.iter().enumerate() {
            let normal_vec = proto
                .normals
                .get(index)
                .map(|n| Vec3::new(n.x, n.y, n.z))
                .unwrap_or(default_normal);
            let uv = texcoords
                .and_then(|coords| coords.get(index))
                .map(|tc| [tc.u, tc.v])
                .unwrap_or([0.0, 0.0]);

            let vertex = W3DVertex {
                position: [position.x, position.y, position.z],
                normal: [normal_vec.x, normal_vec.y, normal_vec.z],
                uv,
                color: [1.0, 1.0, 1.0, 1.0],
            };
            mesh.vertices.push(vertex);
        }

        for tri in &proto.triangles {
            push_world_space_triangle(
                &mut mesh.indices,
                tri.vindex[0],
                tri.vindex[1],
                tri.vindex[2],
            );
        }

        let mut material = self.material_from_prototype(proto);
        if material.texture_name.is_none() {
            if let Some(texture_name) = mesh
                .texture_library
                .first()
                .cloned()
                .or_else(|| model.texture_names.first().cloned())
            {
                material.texture_name = Some(texture_name.clone());
                material.stage0_mapping.texture_name = Some(texture_name);
            }
        }
        Self::apply_material_stage_mappings(&mut material, &mesh);
        mesh.material = material.clone();
        mesh.transform = Mat4::IDENTITY;

        if !material.name.is_empty() {
            model
                .materials
                .entry(material.name.clone())
                .or_insert_with(|| material.clone());
        }

        if let Some(tex_name) = material.texture_name.clone() {
            if !model
                .texture_names
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&tex_name))
            {
                model.texture_names.push(tex_name);
            }
        }

        model.meshes.push(mesh);
        match MeshModelClass::from_mesh_prototype(proto, None) {
            Ok(mesh_model) => {
                model
                    .ww3d_mesh_models
                    .insert(proto.name.clone(), Arc::new(mesh_model));
            }
            Err(err) => {
                warn!(
                    "Failed to convert mesh '{}' to WW3D MeshModelClass: {err:?}",
                    proto.name
                );
            }
        }
        Ok(())
    }

    fn stage_channel_to_uv_source(channel: u8) -> UVSource {
        match channel {
            0 => UVSource::UV0,
            1 => UVSource::UV1,
            2 => UVSource::UV2,
            _ => UVSource::UV3,
        }
    }

    fn stage_mapping_mut<'a>(
        material: &'a mut W3DMaterial,
        stage: usize,
        create: bool,
    ) -> Option<&'a mut TextureStageMapping> {
        match stage {
            0 => Some(&mut material.stage0_mapping),
            1 => {
                if material.stage1_mapping.is_none() && create {
                    material.stage1_mapping = Some(TextureStageMapping::default());
                }
                material.stage1_mapping.as_mut()
            }
            2 => {
                if material.stage2_mapping.is_none() && create {
                    material.stage2_mapping = Some(TextureStageMapping::default());
                }
                material.stage2_mapping.as_mut()
            }
            3 => {
                if material.stage3_mapping.is_none() && create {
                    material.stage3_mapping = Some(TextureStageMapping::default());
                }
                material.stage3_mapping.as_mut()
            }
            _ => None,
        }
    }

    fn apply_material_stage_mappings(material: &mut W3DMaterial, mesh: &W3DMesh) {
        for stage_idx in 0..4 {
            let create = stage_idx == 0
                || mesh.stage_uv_channels.get(stage_idx).is_some()
                || Self::stage_texture_from_mesh(mesh, 0, stage_idx).is_some();

            if let Some(mapping) = Self::stage_mapping_mut(material, stage_idx, create) {
                if mapping.texture_name.is_none() {
                    if let Some(name) = Self::stage_texture_from_mesh(mesh, 0, stage_idx) {
                        mapping.texture_name = Some(name);
                    }
                }
            }
        }

        for (stage_idx, &channel) in mesh.stage_uv_channels.iter().enumerate().take(4) {
            if let Some(mapping) = Self::stage_mapping_mut(material, stage_idx, true) {
                mapping.uv_source = Self::stage_channel_to_uv_source(channel);
            }
        }

        // Match the common C++ material path more closely: once stage 0 resolves to a texture,
        // expose that as the primary material texture too so caches/debugging/legacy consumers
        // don't diverge from the active pass state.
        if material.texture_name.is_none() {
            material.texture_name = material.stage0_mapping.texture_name.clone();
        }
    }

    fn stage_texture_from_mesh(
        mesh: &W3DMesh,
        pass_index: usize,
        stage_index: usize,
    ) -> Option<String> {
        if let Some(stage_sets) = mesh.per_pass_stage_texture_names.get(pass_index) {
            if let Some(names) = stage_sets.get(stage_index) {
                if let Some(name) = names.iter().find(|n| !n.is_empty()) {
                    return Some(name.clone());
                }
            }
        }

        mesh.stage_texture_names_from_ids(pass_index, stage_index)
            .into_iter()
            .find(|name| !name.is_empty())
    }

    fn material_from_prototype(&self, proto: &MeshPrototype) -> W3DMaterial {
        let mut material = W3DMaterial::default();
        material.name = proto.name.clone();

        if let Some(vm) = proto.vertex_materials.first() {
            material.diffuse_color = Self::rgba_to_vec3(&vm.diffuse);
            material.specular_color = Self::rgba_to_vec3(&vm.specular);
            material.emissive_color = Self::rgba_to_vec3(&vm.emissive);
            material.opacity = vm.opacity;
            material.shininess = vm.shininess.max(1.0);
        }

        if let Some(texture_name) = Self::primary_texture_name(proto) {
            material.texture_name = Some(texture_name.clone());
            material.stage0_mapping.texture_name = Some(texture_name);
        }

        material
    }

    fn rgba_to_vec3(color: &ww3d_core::w3d_format::W3dRGBAStruct) -> Vec3 {
        Vec3::new(
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
        )
    }

    fn primary_texture_name(proto: &MeshPrototype) -> Option<String> {
        // Try per-pass stage texture assignments first.
        for stage_sets in &proto.per_pass_stage_texture_ids {
            for stage in stage_sets {
                for tex_id in stage {
                    if let Some(tex) = proto.textures.get(*tex_id as usize) {
                        let name = w3d_string_from_bytes(&tex.name).trim().to_string();
                        if !name.is_empty() {
                            return Some(name);
                        }
                    }
                }
            }
        }

        proto
            .textures
            .first()
            .map(|tex| w3d_string_from_bytes(&tex.name).trim().to_string())
            .filter(|name| !name.is_empty())
    }

    fn texture_name_from_proto(proto: &MeshPrototype, texture_id: u32) -> Option<String> {
        proto
            .textures
            .get(texture_id as usize)
            .map(|tex| w3d_string_from_bytes(&tex.name).trim().to_string())
            .filter(|name| !name.is_empty())
    }

    fn resolve_stage_texture_names(proto: &MeshPrototype) -> Vec<Vec<Vec<String>>> {
        proto
            .per_pass_stage_texture_ids
            .iter()
            .map(|stage_sets| {
                stage_sets
                    .iter()
                    .map(|ids| {
                        ids.iter()
                            .filter_map(|tex_id| Self::texture_name_from_proto(proto, *tex_id))
                            .collect()
                    })
                    .collect()
            })
            .collect()
    }

    /// Parse W3D binary data using the legacy chunk parser (fallback path)
    fn parse_w3d_data_legacy(&self, data: &[u8], model_name: String) -> Result<W3DModel> {
        if data.len() < 8 {
            return Err(anyhow!("W3D file too small: {} bytes", data.len()));
        }

        let mut model = W3DModel::new(model_name);
        let mut offset = 0;

        // Check for W3D file header that needs to be skipped
        // Some W3D files have a header with name section that needs to be skipped
        if data.len() >= 16 {
            // Check if first 4 bytes look like a chunk type or a file header
            let first_u32 = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

            // If it's not a known chunk type, might be a file header
            if first_u32 != W3D_CHUNK_MESH
                && first_u32 != W3D_CHUNK_HIERARCHY
                && first_u32 != W3D_CHUNK_ANIMATION
                && first_u32 != W3D_CHUNK_HMODEL
                && first_u32 != W3D_CHUNK_LODMODEL
            {
                // Check for name section size at offset 12
                let name_section_size =
                    u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;
                if name_section_size > 0
                    && name_section_size < 1024
                    && 16 + name_section_size < data.len()
                {
                    debug!(
                        "Detected W3D file header with name section of {} bytes",
                        name_section_size
                    );
                    offset = 16 + name_section_size;
                }
            }
        }

        // Parse W3D chunks with safety counter to prevent infinite loops
        let mut chunk_counter = 0;
        const MAX_CHUNKS: usize = 10000; // Safety limit to prevent infinite loops

        while offset + 8 <= data.len() && chunk_counter < MAX_CHUNKS {
            chunk_counter += 1;
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            // Handle W3D chunk size format: MSB indicates container chunk
            let is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize; // Clear MSB to get actual size

            debug!(
                "W3D chunk: type=0x{:08X}, raw_size=0x{:08X}, size={}, container={}",
                chunk_type, raw_chunk_size, chunk_size, is_container_chunk
            );

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "W3D chunk extends beyond file: type 0x{:08X}, size {} (raw: 0x{:08X})",
                    chunk_type, chunk_size, raw_chunk_size
                );
                break;
            }

            // Additional safety checks to prevent infinite loops
            if chunk_size == 0 {
                warn!(
                    "Zero-sized chunk detected (type 0x{:08X}) - skipping",
                    chunk_type
                );
                offset += 8; // Skip just the header
                continue;
            }

            if chunk_size > data.len() {
                warn!(
                    "Chunk size {} exceeds total file size {} - aborting parsing",
                    chunk_size,
                    data.len()
                );
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MESH => {
                    debug!("Parsing W3D mesh chunk, size: {}", chunk_size);
                    if let Ok(mut mesh) = self.parse_mesh_chunk(chunk_data) {
                        if mesh.texture_library.is_empty() && !model.texture_names.is_empty() {
                            mesh.texture_library = model.texture_names.clone();
                        }
                        model.meshes.push(mesh);
                    } else {
                        warn!("Failed to parse W3D mesh chunk");
                    }
                }
                W3D_CHUNK_HIERARCHY => {
                    debug!(
                        "Parsing W3D hierarchy chunk (container), size: {}",
                        chunk_size
                    );
                    // Parse hierarchy container - it may contain mesh chunks
                    if is_container_chunk {
                        self.parse_container_chunk(chunk_data, &mut model)?;
                    }
                }
                W3D_CHUNK_MATERIALS3 => {
                    debug!("Parsing W3D materials3 container, size: {}", chunk_size);
                    // Parse materials3 container - contains material definitions with texture names
                    if is_container_chunk {
                        self.parse_materials3_chunk(chunk_data, &mut model)?;
                    }
                }
                W3D_CHUNK_TEXTURES => {
                    debug!("Parsing W3D textures container, size: {}", chunk_size);
                    // Parse textures container - contains individual texture definitions
                    if is_container_chunk {
                        self.parse_textures_chunk(chunk_data, &mut model)?;
                    }
                }
                W3D_CHUNK_ANIMATION => {
                    debug!("Found W3D animation chunk, size: {}", chunk_size);
                    if is_container_chunk {
                        if let Err(e) = self.parse_container_chunk(chunk_data, &mut model) {
                            warn!("Failed to parse animation container: {}", e);
                        }
                    }
                }
                W3D_CHUNK_HMODEL => {
                    debug!("Found W3D hierarchical model chunk, size: {}", chunk_size);
                    if is_container_chunk {
                        if let Err(e) = self.parse_container_chunk(chunk_data, &mut model) {
                            warn!("Failed to parse hierarchical model container: {}", e);
                        }
                    }
                }
                W3D_CHUNK_LODMODEL => {
                    debug!("Found W3D LOD model chunk, size: {}", chunk_size);
                    if is_container_chunk {
                        if let Err(e) = self.parse_container_chunk(chunk_data, &mut model) {
                            warn!("Failed to parse LOD model container: {}", e);
                        }
                    }
                }
                W3D_CHUNK_HLOD => {
                    debug!(
                        "Found W3D HLOD (Hierarchical LOD) chunk, size: {}",
                        chunk_size
                    );
                    // HLOD is a container chunk with hierarchical models and LOD info
                    // Recursively parse to find mesh data
                    if is_container_chunk {
                        if let Err(e) = self.parse_container_chunk(chunk_data, &mut model) {
                            warn!("Failed to parse HLOD container: {}", e);
                        }
                    }
                }
                _ => {
                    debug!("Unknown W3D chunk type: 0x{:08X}", chunk_type);
                    // If it's a container chunk, try to parse it recursively
                    if is_container_chunk && chunk_size > 0 {
                        debug!("  -> Container chunk, parsing recursively");
                        if let Err(e) = self.parse_container_chunk(chunk_data, &mut model) {
                            warn!(
                                "Failed to parse container chunk 0x{:08X}: {}",
                                chunk_type, e
                            );
                        }
                    }
                }
            }

            offset += 8 + chunk_size;
        }

        if chunk_counter >= MAX_CHUNKS {
            warn!(
                "⚠️  W3D chunk parsing hit safety limit ({} chunks) - possible malformed file",
                MAX_CHUNKS
            );
        }

        if model.meshes.is_empty() {
            debug!("No valid meshes found, creating fallback mesh");
            let fallback_mesh = self.create_fallback_mesh("fallback_mesh".to_string());
            model.meshes.push(fallback_mesh);
        }

        // Post-process: Resolve texture indices to actual texture names from W3D_CHUNK_TEXTURES
        // This matches C++ behavior where W3D_CHUNK_MAP3_FILENAME contains texture indices
        // that need to be resolved against the texture_names array
        self.resolve_texture_indices(&mut model);

        model.calculate_bounding_box();
        Ok(model)
    }

    /// Parse a W3D container chunk recursively
    fn parse_container_chunk(&self, data: &[u8], model: &mut W3DModel) -> Result<()> {
        let mut offset = 0;
        let mut chunk_counter = 0;
        const MAX_CONTAINER_CHUNKS: usize = 5000; // Safety limit for container chunks

        while offset + 8 <= data.len() && chunk_counter < MAX_CONTAINER_CHUNKS {
            chunk_counter += 1;
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            let is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "Container sub-chunk extends beyond container: type 0x{:08X}, size {}",
                    chunk_type, chunk_size
                );
                break;
            }

            // Safety checks for container chunks
            if chunk_size == 0 {
                warn!(
                    "Zero-sized container chunk detected (type 0x{:08X}) - skipping",
                    chunk_type
                );
                offset += 8;
                continue;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MESH => {
                    debug!("Found mesh chunk in container, size: {}", chunk_size);
                    if let Ok(mut mesh) = self.parse_mesh_chunk(chunk_data) {
                        if mesh.texture_library.is_empty() && !model.texture_names.is_empty() {
                            mesh.texture_library = model.texture_names.clone();
                        }
                        model.meshes.push(mesh);
                    } else {
                        warn!("Failed to parse mesh chunk in container");
                    }
                }
                W3D_CHUNK_TEXTURES => {
                    debug!("Found textures chunk in container, size: {}", chunk_size);
                    if is_container_chunk {
                        if let Err(e) = self.parse_textures_chunk(chunk_data, model) {
                            warn!("Failed to parse textures chunk: {}", e);
                        }
                    }
                }
                _ => {
                    debug!(
                        "Container sub-chunk: type 0x{:08X}, size {}, container: {}",
                        chunk_type, chunk_size, is_container_chunk
                    );
                    // Recursively parse nested containers
                    if is_container_chunk && chunk_size > 0 {
                        if let Err(e) = self.parse_container_chunk(chunk_data, model) {
                            warn!(
                                "Failed to parse nested container 0x{:08X}: {}",
                                chunk_type, e
                            );
                        }
                    }
                }
            }

            offset += 8 + chunk_size;
        }

        if chunk_counter >= MAX_CONTAINER_CHUNKS {
            warn!(
                "⚠️  Container chunk parsing hit safety limit ({} chunks)",
                MAX_CONTAINER_CHUNKS
            );
        }

        Ok(())
    }

    /// Resolve texture indices to actual texture names - matches C++ behavior
    /// W3D_CHUNK_MAP3_FILENAME may contain texture indices (e.g., "1", "2", "3")
    /// which need to be resolved against the model.texture_names array
    ///
    /// Special case: If texture_names is empty but materials have numeric texture references,
    /// we need to build a texture array from materials in order (C++ behavior when W3D_CHUNK_TEXTURES is missing)
    fn resolve_texture_indices(&self, model: &mut W3DModel) {
        // Check if any texture references are numeric indices
        let has_numeric_indices = model.materials.values().any(|mat| {
            if let Some(ref tex_ref) = mat.texture_name {
                tex_ref.parse::<usize>().is_ok()
            } else {
                false
            }
        }) || model.meshes.iter().any(|mesh| {
            if let Some(ref tex_ref) = mesh.material.texture_name {
                tex_ref.parse::<usize>().is_ok()
            } else {
                false
            }
        });

        // If we have numeric indices but no texture_names array, build one from materials
        if has_numeric_indices && model.texture_names.is_empty() {
            debug!("Building texture array from materials (W3D_CHUNK_TEXTURES missing)");

            // Collect all actual texture filenames from materials in order they appear
            let mut collected_textures: Vec<String> = Vec::new();

            for material in model.materials.values() {
                // Some materials might point to actual filenames (from DC_MAP chunks)
                if let Some(ref tex_name) = material.texture_name {
                    // Only add non-numeric filenames - these are actual texture names
                    if tex_name.parse::<usize>().is_err() {
                        if !collected_textures.contains(tex_name) {
                            debug!("  Added texture from material: {}", tex_name);
                            collected_textures.push(tex_name.clone());
                        }
                    }
                }
            }

            // If we collected any textures, use them as the texture_names array
            if !collected_textures.is_empty() {
                debug!(
                    "Collected {} textures from materials",
                    collected_textures.len()
                );
                model.texture_names = collected_textures;
            } else {
                // No actual filenames found - this might be a pure index-based model
                debug!("No texture filenames in materials, cannot resolve indices");
                return;
            }
        }

        if model.texture_names.is_empty() {
            debug!("No texture names loaded from W3D_CHUNK_TEXTURES, skipping texture index resolution");
            return;
        }

        debug!("Resolving texture indices for model: {}", model.name);
        debug!("  Available textures: {:?}", model.texture_names);

        // Go through each mesh and resolve texture indices
        for mesh in &mut model.meshes {
            if let Some(ref texture_ref) = mesh.material.texture_name {
                // Try to parse texture_ref as an index
                if let Ok(index) = texture_ref.parse::<usize>() {
                    // It's an index - resolve it
                    if index < model.texture_names.len() {
                        let resolved_name = model.texture_names[index].clone();
                        debug!(
                            "Resolved texture index {} to texture name: {}",
                            index, resolved_name
                        );
                        mesh.material.texture_name = Some(resolved_name);
                    } else {
                        warn!(
                            "Texture index {} out of bounds (only {} textures available)",
                            index,
                            model.texture_names.len()
                        );
                    }
                } else {
                    // It's a filename, keep as-is
                    debug!(
                        "Texture reference '{}' is not an index, keeping as filename",
                        texture_ref
                    );
                }
            }
        }

        // Also update materials map if they have texture references
        for (name, material) in &mut model.materials {
            if let Some(ref texture_ref) = material.texture_name {
                if let Ok(index) = texture_ref.parse::<usize>() {
                    if index < model.texture_names.len() {
                        let resolved_name = model.texture_names[index].clone();
                        debug!(
                            "Resolved material '{}' texture index {} to: {}",
                            name, index, resolved_name
                        );
                        let mut updated_material = material.clone();
                        updated_material.texture_name = Some(resolved_name);
                        *material = updated_material;
                    }
                }
            }
        }
    }

    /// Parse a W3D mesh chunk
    fn parse_mesh_chunk(&self, data: &[u8]) -> Result<W3DMesh> {
        debug!("parse_mesh_chunk called, data size: {} bytes", data.len());
        let mut mesh = W3DMesh::new("unknown_mesh".to_string());
        let mut offset = 0;

        let mut vertices: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut texcoords: Vec<[f32; 2]> = Vec::new();
        let mut vertex_colors: Vec<[f32; 4]> = Vec::new();
        let mut triangles: Vec<[u32; 3]> = Vec::new();
        let mut expected_vertex_count: Option<u32> = None;
        let mut texture_names: Vec<String> = Vec::new(); // C++ MeshLoadContextClass texture array

        // Parse mesh sub-chunks with safety counter
        let mut mesh_chunk_counter = 0;
        const MAX_MESH_CHUNKS: usize = 1000; // Safety limit for mesh chunks

        while offset + 8 <= data.len() && mesh_chunk_counter < MAX_MESH_CHUNKS {
            mesh_chunk_counter += 1;
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            let _is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "Mesh sub-chunk extends beyond mesh: type 0x{:08X}, size {}",
                    chunk_type, chunk_size
                );
                break;
            }

            // Safety checks for mesh chunks
            if chunk_size == 0 {
                warn!(
                    "Zero-sized mesh chunk detected (type 0x{:08X}) - skipping",
                    chunk_type
                );
                offset += 8;
                continue;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MESH_HEADER => {
                    debug!(
                        "Parsing mesh header (W3dMeshHeader3Struct), size: {}",
                        chunk_size
                    );
                    if let Ok(header) = self.parse_mesh_header(chunk_data) {
                        mesh.name = header.mesh_name;
                        expected_vertex_count = Some(header.num_vertices);
                        debug!(
                            "Mesh name: '{}', expecting {} vertices, {} triangles",
                            mesh.name, header.num_vertices, header.num_triangles
                        );
                    } else {
                        warn!("Failed to parse mesh header");
                    }
                }
                W3D_CHUNK_VERTICES => {
                    vertices = self.parse_vertices_with_count(chunk_data, expected_vertex_count)?;
                    debug!("Parsed {} vertices", vertices.len());
                }
                W3D_CHUNK_VERTEX_NORMALS => {
                    normals = self.parse_normals(chunk_data)?;
                    debug!("Parsed {} normals", normals.len());
                }
                W3D_CHUNK_TEXCOORDS => {
                    texcoords = self.parse_texcoords(chunk_data)?;
                    debug!("Parsed {} texture coordinates", texcoords.len());
                }
                W3D_CHUNK_VERTEX_COLORS => {
                    vertex_colors = self.parse_vertex_colors(chunk_data)?;
                    debug!("Parsed {} vertex colors", vertex_colors.len());
                }
                W3D_CHUNK_TRIANGLES => {
                    triangles = self.parse_triangles(chunk_data)?;
                    debug!("Parsed {} triangles", triangles.len());
                }
                W3D_CHUNK_MATERIAL_INFO => {
                    debug!("Parsing material info chunk, size: {}", chunk_size);
                    if let Ok(material) = self.parse_material_info(chunk_data) {
                        mesh.material = material;
                        debug!(
                            "Parsed material: {} (texture: {:?})",
                            mesh.material.name, mesh.material.texture_name
                        );
                    } else {
                        warn!("Failed to parse material info chunk");
                    }
                }
                W3D_CHUNK_MAP3_FILENAME => {
                    // Extract texture filename from MAP3_FILENAME chunk
                    // Read null-terminated string directly from chunk data
                    let mut filename = String::new();
                    for &byte in chunk_data {
                        if byte == 0 {
                            break;
                        }
                        if byte.is_ascii() && byte >= 32 {
                            filename.push(byte as char);
                        }
                    }
                    if !filename.is_empty() {
                        debug!(
                            "Found texture filename in W3D_CHUNK_MAP3_FILENAME: {}",
                            filename
                        );
                        mesh.material.texture_name = Some(filename);
                    }
                }
                W3D_CHUNK_VERTEX_SHADE_INDICES => {
                    // Shade indices for vertex coloring - skip for now
                    debug!(
                        "Skipping W3D_CHUNK_VERTEX_SHADE_INDICES ({} bytes)",
                        chunk_size
                    );
                }
                W3D_CHUNK_SHADERS => {
                    // Shader definitions - skip for now
                    debug!("Skipping W3D_CHUNK_SHADERS ({} bytes)", chunk_size);
                }
                W3D_CHUNK_VERTEX_MATERIALS => {
                    // Parse vertex materials container
                    debug!("Parsing W3D_CHUNK_VERTEX_MATERIALS ({} bytes)", chunk_size);
                    let mut vmat_offset = 0;
                    while vmat_offset + 8 <= chunk_data.len() {
                        let vmat_type = u32::from_le_bytes([
                            chunk_data[vmat_offset],
                            chunk_data[vmat_offset + 1],
                            chunk_data[vmat_offset + 2],
                            chunk_data[vmat_offset + 3],
                        ]);
                        let vmat_size = u32::from_le_bytes([
                            chunk_data[vmat_offset + 4],
                            chunk_data[vmat_offset + 5],
                            chunk_data[vmat_offset + 6],
                            chunk_data[vmat_offset + 7],
                        ]) as usize;

                        if vmat_type == 0x00000027 {
                            // W3D_CHUNK_VERTEX_MATERIAL
                            // Skip parsing individual vertex materials for now
                            debug!("Found W3D_CHUNK_VERTEX_MATERIAL, size: {}", vmat_size);
                        }
                        vmat_offset += 8 + vmat_size;
                    }
                }
                W3D_CHUNK_MATERIAL_PASS => {
                    // Material pass definitions - skip for now
                    debug!("Skipping W3D_CHUNK_MATERIAL_PASS ({} bytes)", chunk_size);
                }
                W3D_CHUNK_TEXTURES => {
                    // Parse textures container - C++ read_textures() equivalent
                    debug!(
                        "Found W3D_CHUNK_TEXTURES inside mesh, size: {} bytes",
                        chunk_size
                    );
                    // Parse texture names from W3D_CHUNK_TEXTURE/W3D_CHUNK_TEXTURE_NAME
                    if let Ok(names) = self.parse_textures_chunk_into_array(chunk_data) {
                        debug!("Loaded {} texture(s) for mesh: {:?}", names.len(), names);
                        texture_names.extend(names);
                    }
                }
                _ => {
                    debug!("Unknown mesh sub-chunk: 0x{:08X}", chunk_type);
                }
            }

            offset += 8 + chunk_size;
        }

        if mesh_chunk_counter >= MAX_MESH_CHUNKS {
            warn!(
                "⚠️  Mesh chunk parsing hit safety limit ({} chunks)",
                MAX_MESH_CHUNKS
            );
        }

        // Build final mesh (logging disabled)
        self.build_mesh_from_data(
            &mut mesh,
            vertices,
            normals,
            texcoords,
            vertex_colors,
            triangles,
        )?;

        // C++ behavior: Associate first texture with material (index 0)
        if !texture_names.is_empty() {
            mesh.material.texture_name = Some(texture_names[0].clone());
            debug!(
                "Mesh '{}' will use texture: '{}'",
                mesh.name, texture_names[0]
            );
        }

        Ok(mesh)
    }

    /// Parse mesh header - C++ compatible W3dMeshHeader3Struct format
    fn parse_mesh_header(&self, data: &[u8]) -> Result<MeshHeader> {
        // W3dMeshHeader3Struct layout:
        // 0: uint32 Version
        // 4: uint32 Attributes
        // 8: char MeshName[16]
        // 24: char ContainerName[16]
        // 40: uint32 NumTris
        // 44: uint32 NumVertices
        // 48: uint32 NumMaterials
        // 52: uint32 NumDamageStages
        // 56: sint32 SortLevel
        // 60: uint32 PrelitVersion
        // 64: uint32 FutureCounts[1]
        // 68: uint32 VertexChannels
        // 72: uint32 FaceChannels
        // Plus bounding box, sphere data...

        if data.len() < 76 {
            // Minimum size for core header fields
            return Err(anyhow!("Mesh header too small: {} bytes", data.len()));
        }

        let version = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let attributes = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let num_triangles = u32::from_le_bytes([data[40], data[41], data[42], data[43]]);
        let num_vertices = u32::from_le_bytes([data[44], data[45], data[46], data[47]]);

        // Extract mesh name (null-terminated string at offset 8, max 16 chars)
        let mut mesh_name = String::new();
        for i in 8..24 {
            if i >= data.len() || data[i] == 0 {
                break;
            }
            mesh_name.push(data[i] as char);
        }

        // Extract container name (null-terminated string at offset 24, max 16 chars)
        let mut container_name = String::new();
        for i in 24..40 {
            if i >= data.len() || data[i] == 0 {
                break;
            }
            container_name.push(data[i] as char);
        }

        debug!("Mesh header - version: 0x{:08X}, attributes: 0x{:08X}, triangles: {}, vertices: {}, mesh_name: '{}', container: '{}'", 
               version, attributes, num_triangles, num_vertices, mesh_name, container_name);

        Ok(MeshHeader {
            version,
            flags: attributes, // attributes field is what was called flags in the old structure
            num_triangles,
            num_vertices,
            mesh_name: if mesh_name.is_empty() {
                "unnamed_mesh".to_string()
            } else {
                mesh_name
            },
        })
    }

    /// Parse vertices array with expected count validation - C++ compatible version
    fn parse_vertices_with_count(
        &self,
        data: &[u8],
        expected_count: Option<u32>,
    ) -> Result<Vec<[f32; 3]>> {
        // In C++: reads vertex count from mesh header, then reads that many W3dVectorStruct (12 bytes each)
        // No headers or padding in vertex chunk data itself - just raw vertex data

        let vertex_count = if let Some(expected) = expected_count {
            expected as usize
        } else {
            // Fallback: assume data contains only vertices (12 bytes each)
            data.len() / 12
        };

        debug!(
            "parse_vertices_with_count: data.len()={}, expected_count={:?}, using vertex_count={}",
            data.len(),
            expected_count,
            vertex_count
        );

        // Verify we have enough data for the expected vertices
        let required_size = vertex_count * 12; // 12 bytes per W3dVectorStruct
        if data.len() < required_size {
            return Err(anyhow!(
                "Insufficient vertex data: need {} bytes, have {} (for {} vertices)",
                required_size,
                data.len(),
                vertex_count
            ));
        }

        let mut vertices = Vec::with_capacity(vertex_count);

        // Read vertices directly as W3dVectorStruct (float32 X, Y, Z)
        for i in 0..vertex_count {
            let offset = i * 12;
            if offset + 12 > data.len() {
                warn!(
                    "Vertex {} would exceed data bounds, stopping at {} vertices",
                    i,
                    vertices.len()
                );
                break;
            }

            let x = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let y = f32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let z = f32::from_le_bytes([
                data[offset + 8],
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
            ]);

            // Validate vertices are reasonable (not NaN, not infinite)
            if !x.is_finite() || !y.is_finite() || !z.is_finite() {
                warn!(
                    "Vertex {} has non-finite coordinates: ({}, {}, {})",
                    i, x, y, z
                );
                continue;
            }

            vertices.push([x, y, z]);

            // Log first few vertices for debugging
            if i < 3 {
                debug!("Vertex {}: ({:.3}, {:.3}, {:.3})", i, x, y, z);
            }
        }

        if vertices.is_empty() {
            return Err(anyhow!("No valid vertices parsed from data"));
        }

        debug!("Successfully parsed {} vertices", vertices.len());
        Ok(vertices)
    }

    /// Legacy parse vertices for backward compatibility
    fn parse_vertices(&self, data: &[u8]) -> Result<Vec<[f32; 3]>> {
        self.parse_vertices_with_count(data, None)
    }

    /// Parse normals array
    fn parse_normals(&self, data: &[u8]) -> Result<Vec<[f32; 3]>> {
        if data.len() % 12 != 0 {
            return Err(anyhow!("Invalid normals data size: {}", data.len()));
        }

        let normal_count = data.len() / 12;
        let mut normals = Vec::with_capacity(normal_count);

        for i in 0..normal_count {
            let offset = i * 12;
            let x = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let y = f32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let z = f32::from_le_bytes([
                data[offset + 8],
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
            ]);
            normals.push([x, y, z]);
        }

        Ok(normals)
    }

    /// Parse texture coordinates array
    fn parse_texcoords(&self, data: &[u8]) -> Result<Vec<[f32; 2]>> {
        if data.len() % 8 != 0 {
            return Err(anyhow!("Invalid texcoords data size: {}", data.len()));
        }

        let texcoord_count = data.len() / 8;
        let mut texcoords = Vec::with_capacity(texcoord_count);

        for i in 0..texcoord_count {
            let offset = i * 8;
            let u = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let v = f32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            texcoords.push([u, v]);
        }

        Ok(texcoords)
    }

    /// Parse vertex colors array
    fn parse_vertex_colors(&self, data: &[u8]) -> Result<Vec<[f32; 4]>> {
        if data.len() % 4 != 0 {
            return Err(anyhow!("Invalid vertex colors data size: {}", data.len()));
        }

        let color_count = data.len() / 4;
        let mut colors = Vec::with_capacity(color_count);

        for i in 0..color_count {
            let offset = i * 4;
            let color_rgba = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);

            // Convert RGBA to float components
            let r = ((color_rgba >> 16) & 0xFF) as f32 / 255.0;
            let g = ((color_rgba >> 8) & 0xFF) as f32 / 255.0;
            let b = (color_rgba & 0xFF) as f32 / 255.0;
            let a = ((color_rgba >> 24) & 0xFF) as f32 / 255.0;

            colors.push([r, g, b, a]);
        }

        Ok(colors)
    }

    /// Parse material info
    fn parse_material_info(&self, data: &[u8]) -> Result<W3DMaterial> {
        if data.len() < 4 {
            // Need at least 4 bytes for basic parsing
            return Err(anyhow!("Material info chunk too small: {}", data.len()));
        }

        // Material info structure is complex, let's extract basic information
        let mut material = W3DMaterial::default();

        // For small material info chunks (16 bytes), extract basic properties
        // For larger chunks, try to extract more detailed information

        if data.len() >= 48 {
            // Extract C++ VertexMaterialClass compatible color values for larger chunks
            let diffuse_r = f32::from_le_bytes(data[32..36].try_into().unwrap_or([0; 4]));
            let diffuse_g = f32::from_le_bytes(data[36..40].try_into().unwrap_or([0; 4]));
            let diffuse_b = f32::from_le_bytes(data[40..44].try_into().unwrap_or([0; 4]));

            if diffuse_r.is_finite() && diffuse_g.is_finite() && diffuse_b.is_finite() {
                material.diffuse_color = Vec3::new(diffuse_r, diffuse_g, diffuse_b);
            }
        }

        if data.len() >= 32 {
            // Try to extract material name for larger chunks
            let mut name = String::new();
            for i in 0..std::cmp::min(32, data.len()) {
                if data[i] == 0 {
                    break;
                }
                if data[i].is_ascii() && data[i] >= 32 {
                    name.push(data[i] as char);
                }
            }
            if !name.is_empty() {
                material.name = name;
            }
        } else if data.len() >= 16 {
            // For small material info chunks (16 bytes), extract basic properties
            debug!("Parsing 16-byte material info chunk - basic material properties");

            // Try to extract some basic properties from the first few bytes
            // Material index or type might be at the beginning
            let material_type = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            debug!("Material type/index: 0x{:08X}", material_type);

            // Set basic properties for small chunks
            material.name = format!("material_{:08X}", material_type);
            material.diffuse_color = Vec3::new(0.8, 0.8, 0.8); // Default gray
        }

        // Note: Texture names are now loaded separately from W3D_CHUNK_TEXTURES
        // They will be associated with materials through material passes

        // Set C++ compatible material properties
        material.stage0_mapping.uv_source = UVSource::UV0;
        material.stage0_mapping.blend_mode = TextureBlendMode::Modulate;
        material.blend_mode = BlendMode::Opaque;

        // Set texture name in stage 0 if found
        if let Some(ref texture_name) = material.texture_name {
            material.stage0_mapping.texture_name = Some(texture_name.clone());
        }

        debug!(
            "Parsed material: name='{}', diffuse={:?}, texture={:?}",
            material.name, material.diffuse_color, material.texture_name
        );

        Ok(material)
    }

    /// Parse W3D textures container chunk - contains individual texture definitions
    fn parse_textures_chunk(&self, data: &[u8], model: &mut W3DModel) -> Result<()> {
        let mut offset = 0;
        let mut texture_count = 0;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "Invalid texture chunk size: {} at offset {}",
                    chunk_size, offset
                );
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_TEXTURE => {
                    debug!("Parsing individual texture chunk, size: {}", chunk_size);
                    if is_container_chunk {
                        if let Ok(texture_name) = self.parse_single_texture_chunk(chunk_data) {
                            debug!("Found texture: {}", texture_name);
                            model.texture_names.push(texture_name);
                            texture_count += 1;
                        }
                    }
                }
                _ => {
                    debug!(
                        "Unknown texture sub-chunk: 0x{:08X}, size: {}",
                        chunk_type, chunk_size
                    );
                }
            }

            offset += 8 + chunk_size;
        }

        debug!("Loaded {} textures from W3D_CHUNK_TEXTURES", texture_count);
        Ok(())
    }

    /// Parse W3D_CHUNK_TEXTURES and return array of texture names - C++ read_textures() equivalent
    fn parse_textures_chunk_into_array(&self, data: &[u8]) -> Result<Vec<String>> {
        debug!("parse_textures_chunk_into_array: data.len()={}", data.len());
        let mut textures = Vec::new();
        let mut offset = 0;

        // C++ code: for (TextureClass *newtex = ::Load_Texture(cload); newtex != NULL; newtex = ::Load_Texture(cload))
        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            // Check for container chunk flag (bit 31 set on chunk size - C++ behavior)
            let is_container = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            // C++ Load_Texture checks for W3D_CHUNK_TEXTURE
            if chunk_type == W3D_CHUNK_TEXTURE && is_container {
                if let Ok(texture_name) = self.parse_single_texture_chunk(chunk_data) {
                    textures.push(texture_name);
                }
            }

            offset += 8 + chunk_size;
        }

        debug!("Returning {} textures", textures.len());
        Ok(textures)
    }

    /// Parse a single W3D_CHUNK_TEXTURE and extract the texture name
    fn parse_single_texture_chunk(&self, data: &[u8]) -> Result<String> {
        let mut offset = 0;
        let mut texture_name: Option<String> = None;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_TEXTURE_NAME => {
                    // Read null-terminated string directly from chunk data
                    let mut name = String::new();
                    for &byte in chunk_data {
                        if byte == 0 {
                            break;
                        }
                        if byte.is_ascii() && byte >= 32 {
                            name.push(byte as char);
                        }
                    }

                    if !name.is_empty() {
                        debug!("Found texture name in W3D_CHUNK_TEXTURE_NAME: {}", name);
                        texture_name = Some(name);
                    }
                }
                W3D_CHUNK_TEXTURE_INFO => {
                    debug!("Found W3D_CHUNK_TEXTURE_INFO (not parsing texture properties yet)");
                    // W3dTextureInfoStruct parsing can be added here later if needed
                }
                _ => {
                    debug!(
                        "Unknown texture sub-chunk in W3D_CHUNK_TEXTURE: 0x{:08X}",
                        chunk_type
                    );
                }
            }

            offset += 8 + chunk_size;
        }

        texture_name.ok_or_else(|| anyhow!("No texture name found in W3D_CHUNK_TEXTURE"))
    }

    /// Parse W3D MATERIALS3 container chunk - contains material definitions with texture filenames
    /// This matches the C++ approach: create materials and directly assign texture names
    fn parse_materials3_chunk(&self, data: &[u8], model: &mut W3DModel) -> Result<()> {
        let mut offset = 0;
        let mut material_count = 0;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "Invalid materials3 chunk size: {} at offset {}",
                    chunk_size, offset
                );
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MATERIAL3 => {
                    debug!("Parsing individual material3 chunk, size: {}", chunk_size);
                    if is_container_chunk {
                        // Parse the complete material (name + properties + texture) like C++ does
                        if let Ok(material) = self.parse_complete_material3_chunk(chunk_data) {
                            debug!(
                                "Found material3: '{}' with texture: {:?}",
                                material.name, material.texture_name
                            );

                            // Store the material in the model's materials HashMap
                            model
                                .materials
                                .insert(material.name.clone(), material.clone());

                            // Also add texture name to the model's texture list for loading
                            if let Some(ref texture_name) = material.texture_name {
                                model.texture_names.push(texture_name.clone());
                            }
                            material_count += 1;
                        }
                    }
                }
                _ => {
                    debug!(
                        "Unknown materials3 sub-chunk: 0x{:08X}, size: {}",
                        chunk_type, chunk_size
                    );
                }
            }

            offset += 8 + chunk_size;
        }

        debug!(
            "Loaded {} complete materials from W3D_CHUNK_MATERIALS3",
            material_count
        );
        Ok(())
    }

    /// Parse a complete W3D_CHUNK_MATERIAL3 exactly like C++ does:
    /// 1. Read W3D_CHUNK_MATERIAL3_NAME
    /// 2. Read W3D_CHUNK_MATERIAL3_INFO (material properties)
    /// 3. Read W3D_CHUNK_MATERIAL3_DC_MAP -> W3D_CHUNK_MAP3_FILENAME (texture)
    fn parse_complete_material3_chunk(&self, data: &[u8]) -> Result<W3DMaterial> {
        let mut offset = 0;
        let mut material = W3DMaterial::default();
        let mut material_name: Option<String> = None;

        // Parse chunks inside W3D_CHUNK_MATERIAL3 container like C++ does
        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MATERIAL3_NAME => {
                    // 0x0000002D
                    // Read material name exactly like C++: cload.Read(name,cload.Cur_Chunk_Length());
                    let mut name = String::new();
                    for &byte in chunk_data {
                        if byte == 0 {
                            break;
                        }
                        if byte.is_ascii() && byte >= 32 {
                            name.push(byte as char);
                        }
                    }

                    if !name.is_empty() {
                        material_name = Some(name);
                        debug!("Found material3 name: {}", material_name.as_ref().unwrap());
                    }
                }
                W3D_CHUNK_MATERIAL3_INFO => {
                    // 0x0000002E
                    // Read W3dMaterial3Struct like C++: cload.Read(&mat,sizeof(W3dMaterial3Struct))
                    debug!("Parsing W3D_CHUNK_MATERIAL3_INFO, size: {}", chunk_size);
                    // For now, set basic material properties - we can expand this later
                    material.diffuse_color = Vec3::new(0.8, 0.8, 0.8);
                    material.specular_color = Vec3::new(0.2, 0.2, 0.2);
                    material.shininess = 16.0;
                    material.opacity = 1.0;
                }
                W3D_CHUNK_MATERIAL3_DC_MAP => {
                    // 0x0000002F - Diffuse Color Map
                    debug!(
                        "Found W3D_CHUNK_MATERIAL3_DC_MAP, extracting texture filename like C++"
                    );
                    let _is_container_chunk = (chunk_type & 0x80000000) != 0 || chunk_size > 256; // DC_MAP is a container

                    if let Ok(texture_filename) = self.parse_material3_dc_map_chunk(chunk_data) {
                        debug!(
                            "C++ style: Found texture filename from DC_MAP: {}",
                            texture_filename
                        );
                        material.texture_name = Some(texture_filename);
                        material.stage0_mapping.texture_name = material.texture_name.clone();
                    }
                }
                _ => {
                    debug!("Unknown material3 sub-chunk: 0x{:08X}", chunk_type);
                }
            }

            offset += 8 + chunk_size;
        }

        // Set material name like C++: vmat->Set_Name(name);
        if let Some(name) = material_name {
            material.name = name;
        } else {
            material.name = "unnamed_material3".to_string();
        }

        // Set C++ compatible material properties
        material.stage0_mapping.uv_source = UVSource::UV0;
        material.stage0_mapping.blend_mode = TextureBlendMode::Modulate;
        material.blend_mode = BlendMode::Opaque;

        debug!(
            "Completed material3 parsing: '{}' with texture: {:?}",
            material.name, material.texture_name
        );

        Ok(material)
    }

    /// Parse a single W3D_CHUNK_MATERIAL3 and extract texture filenames from DC_MAP chunks
    fn parse_single_material3_chunk(&self, data: &[u8]) -> Result<Vec<String>> {
        let mut offset = 0;
        let mut texture_names: Vec<String> = Vec::new();

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            let is_container_chunk = (chunk_type & 0x80000000) != 0;
            let chunk_type = chunk_type & 0x7FFFFFFF;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MATERIAL3_DC_MAP => {
                    debug!("Found W3D_CHUNK_MATERIAL3_DC_MAP, extracting texture filename");
                    if is_container_chunk {
                        // Parse the DC_MAP container to find the filename
                        if let Ok(filename) = self.parse_material3_dc_map_chunk(chunk_data) {
                            debug!("Found texture filename from material3 DC_MAP: {}", filename);
                            texture_names.push(filename);
                        }
                    }
                }
                _ => {
                    debug!("Unknown material3 sub-chunk: 0x{:08X}", chunk_type);
                }
            }

            offset += 8 + chunk_size;
        }

        Ok(texture_names)
    }

    /// Parse W3D_CHUNK_MATERIAL3_DC_MAP to extract texture filename
    fn parse_material3_dc_map_chunk(&self, data: &[u8]) -> Result<String> {
        let mut offset = 0;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MAP3_FILENAME => {
                    // 0x00000030
                    // Read null-terminated string directly from chunk data
                    let mut filename = String::new();
                    for &byte in chunk_data {
                        if byte == 0 {
                            break;
                        }
                        if byte.is_ascii() && byte >= 32 {
                            filename.push(byte as char);
                        }
                    }

                    if !filename.is_empty() {
                        debug!(
                            "Found texture filename in W3D_CHUNK_MAP3_FILENAME: {}",
                            filename
                        );
                        return Ok(filename);
                    }
                }
                _ => {
                    debug!("Unknown DC_MAP sub-chunk: 0x{:08X}", chunk_type);
                }
            }

            offset += 8 + chunk_size;
        }

        Err(anyhow!(
            "No texture filename found in W3D_CHUNK_MATERIAL3_DC_MAP"
        ))
    }

    /// Parse triangles array - C++ compatible W3dTriStruct format
    fn parse_triangles(&self, data: &[u8]) -> Result<Vec<[u32; 3]>> {
        // W3dTriStruct format: 3 x uint32 vertex indices, uint32 attributes, W3dVectorStruct normal, float32 distance
        // Total size: 3*4 + 4 + 3*4 + 4 = 32 bytes per triangle
        const TRI_STRUCT_SIZE: usize = 32;

        if data.len() % TRI_STRUCT_SIZE != 0 {
            return Err(anyhow!(
                "Invalid triangles data size: {} (expected multiple of {})",
                data.len(),
                TRI_STRUCT_SIZE
            ));
        }

        let triangle_count = data.len() / TRI_STRUCT_SIZE;
        let mut triangles = Vec::with_capacity(triangle_count);

        for i in 0..triangle_count {
            let offset = i * TRI_STRUCT_SIZE;

            // Read the 3 vertex indices (first 12 bytes of W3dTriStruct)
            let v0 = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let v1 = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let v2 = u32::from_le_bytes([
                data[offset + 8],
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
            ]);

            // Skip attributes (4 bytes), normal (12 bytes), and distance (4 bytes) for now
            // We only need the vertex indices for basic rendering

            triangles.push([v0, v1, v2]);

            // Log first few triangles for debugging
            if i < 3 {
                debug!("Triangle {}: [{}, {}, {}]", i, v0, v1, v2);
            }
        }

        debug!("Successfully parsed {} triangles", triangles.len());
        Ok(triangles)
    }

    /// Build final mesh from parsed data
    fn build_mesh_from_data(
        &self,
        mesh: &mut W3DMesh,
        vertices: Vec<[f32; 3]>,
        normals: Vec<[f32; 3]>,
        texcoords: Vec<[f32; 2]>,
        vertex_colors: Vec<[f32; 4]>,
        triangles: Vec<[u32; 3]>,
    ) -> Result<()> {
        if vertices.is_empty() {
            return Err(anyhow!("No vertices in mesh"));
        }

        let vertex_count = vertices.len();
        mesh.vertices.clear();
        mesh.vertices.reserve(vertex_count);
        mesh.indices.clear();

        // Build vertices with available data
        for i in 0..vertex_count {
            let position = w3d_position_to_world(vertices[i]);
            let normal = if i < normals.len() {
                w3d_normal_to_world(normals[i])
            } else {
                [0.0, 1.0, 0.0]
            };
            let uv = if i < texcoords.len() {
                texcoords[i]
            } else {
                [0.0, 0.0]
            };
            let color = if i < vertex_colors.len() {
                vertex_colors[i]
            } else {
                [1.0, 1.0, 1.0, 1.0]
            };

            mesh.vertices.push(W3DVertex {
                position,
                normal,
                uv,
                color,
            });
        }
        mesh.vertices_in_render_space = true;
        mesh.has_explicit_vertex_colors = !vertex_colors.is_empty();

        // Build indices from triangles
        for triangle in triangles {
            if triangle[0] < vertex_count as u32
                && triangle[1] < vertex_count as u32
                && triangle[2] < vertex_count as u32
            {
                push_world_space_triangle(&mut mesh.indices, triangle[0], triangle[1], triangle[2]);
            }
        }

        // If no triangles provided, create a simple triangle list
        if mesh.indices.is_empty() && vertex_count >= 3 {
            for i in (0..vertex_count - 2).step_by(3) {
                mesh.indices.push(i as u32);
                mesh.indices.push((i + 1) as u32);
                mesh.indices.push((i + 2) as u32);
            }
        }

        debug!(
            "Built mesh with {} vertices and {} indices",
            mesh.vertices.len(),
            mesh.indices.len()
        );
        Ok(())
    }

    /// Create fallback mesh when W3D parsing fails or for testing
    fn create_fallback_mesh(&self, name: String) -> W3DMesh {
        let mut mesh = W3DMesh::new(name);

        // Create a simple tank-like shape
        mesh.vertices = vec![
            // Bottom face (tank base)
            W3DVertex {
                position: [-2.0, -0.5, -1.0],
                normal: [0.0, -1.0, 0.0],
                uv: [0.0, 0.0],
                color: [0.8, 0.8, 0.8, 1.0],
            },
            W3DVertex {
                position: [2.0, -0.5, -1.0],
                normal: [0.0, -1.0, 0.0],
                uv: [1.0, 0.0],
                color: [0.8, 0.8, 0.8, 1.0],
            },
            W3DVertex {
                position: [2.0, -0.5, 1.0],
                normal: [0.0, -1.0, 0.0],
                uv: [1.0, 1.0],
                color: [0.8, 0.8, 0.8, 1.0],
            },
            W3DVertex {
                position: [-2.0, -0.5, 1.0],
                normal: [0.0, -1.0, 0.0],
                uv: [0.0, 1.0],
                color: [0.8, 0.8, 0.8, 1.0],
            },
            // Top face (tank hull)
            W3DVertex {
                position: [-1.5, 0.5, -0.8],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                color: [0.7, 0.7, 0.7, 1.0],
            },
            W3DVertex {
                position: [1.5, 0.5, -0.8],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 0.0],
                color: [0.7, 0.7, 0.7, 1.0],
            },
            W3DVertex {
                position: [1.5, 0.5, 0.8],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 1.0],
                color: [0.7, 0.7, 0.7, 1.0],
            },
            W3DVertex {
                position: [-1.5, 0.5, 0.8],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 1.0],
                color: [0.7, 0.7, 0.7, 1.0],
            },
            // Turret base
            W3DVertex {
                position: [-0.8, 0.5, -0.6],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                color: [0.6, 0.6, 0.6, 1.0],
            },
            W3DVertex {
                position: [0.8, 0.5, -0.6],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 0.0],
                color: [0.6, 0.6, 0.6, 1.0],
            },
            W3DVertex {
                position: [0.8, 0.5, 0.6],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 1.0],
                color: [0.6, 0.6, 0.6, 1.0],
            },
            W3DVertex {
                position: [-0.8, 0.5, 0.6],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 1.0],
                color: [0.6, 0.6, 0.6, 1.0],
            },
            // Turret top
            W3DVertex {
                position: [-0.6, 1.0, -0.4],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                color: [0.5, 0.5, 0.5, 1.0],
            },
            W3DVertex {
                position: [0.6, 1.0, -0.4],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 0.0],
                color: [0.5, 0.5, 0.5, 1.0],
            },
            W3DVertex {
                position: [0.6, 1.0, 0.4],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 1.0],
                color: [0.5, 0.5, 0.5, 1.0],
            },
            W3DVertex {
                position: [-0.6, 1.0, 0.4],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 1.0],
                color: [0.5, 0.5, 0.5, 1.0],
            },
            // Cannon barrel (simplified as box)
            W3DVertex {
                position: [0.6, 1.0, -0.1],
                normal: [1.0, 0.0, 0.0],
                uv: [0.0, 0.0],
                color: [0.4, 0.4, 0.4, 1.0],
            },
            W3DVertex {
                position: [3.0, 1.0, -0.1],
                normal: [1.0, 0.0, 0.0],
                uv: [1.0, 0.0],
                color: [0.4, 0.4, 0.4, 1.0],
            },
            W3DVertex {
                position: [3.0, 1.2, -0.1],
                normal: [1.0, 0.0, 0.0],
                uv: [1.0, 1.0],
                color: [0.4, 0.4, 0.4, 1.0],
            },
            W3DVertex {
                position: [0.6, 1.2, -0.1],
                normal: [1.0, 0.0, 0.0],
                uv: [0.0, 1.0],
                color: [0.4, 0.4, 0.4, 1.0],
            },
            W3DVertex {
                position: [0.6, 1.0, 0.1],
                normal: [1.0, 0.0, 0.0],
                uv: [0.0, 0.0],
                color: [0.4, 0.4, 0.4, 1.0],
            },
            W3DVertex {
                position: [3.0, 1.0, 0.1],
                normal: [1.0, 0.0, 0.0],
                uv: [1.0, 0.0],
                color: [0.4, 0.4, 0.4, 1.0],
            },
            W3DVertex {
                position: [3.0, 1.2, 0.1],
                normal: [1.0, 0.0, 0.0],
                uv: [1.0, 1.0],
                color: [0.4, 0.4, 0.4, 1.0],
            },
            W3DVertex {
                position: [0.6, 1.2, 0.1],
                normal: [1.0, 0.0, 0.0],
                uv: [0.0, 1.0],
                color: [0.4, 0.4, 0.4, 1.0],
            },
        ];

        // Tank indices - creating faces for the tank
        mesh.indices = vec![
            // Bottom face
            0, 1, 2, 2, 3, 0, // Top face
            4, 6, 5, 6, 4, 7, // Turret base
            8, 10, 9, 10, 8, 11, // Turret top
            12, 14, 13, 14, 12, 15, // Cannon faces
            16, 17, 18, 18, 19, 16, // Top
            20, 22, 21, 22, 20, 23, // Bottom
        ];

        // Set C++ VertexMaterialClass compatible properties for tank
        mesh.material.name = "tank_material".to_string();
        mesh.material.diffuse_color = Vec3::new(0.8, 0.8, 0.8); // Light gray base
        mesh.material.specular_color = Vec3::new(0.3, 0.3, 0.3); // Moderate metallic reflection
        mesh.material.emissive_color = Vec3::ZERO; // No self-illumination
        mesh.material.shininess = 16.0; // Metal-like specular power
        mesh.material.opacity = 1.0; // Fully opaque

        // Setup stage 0 texture mapping (primary diffuse texture)
        mesh.material.stage0_mapping.uv_source = UVSource::UV0;
        mesh.material.stage0_mapping.blend_mode = TextureBlendMode::Modulate;
        mesh.material.stage0_mapping.address_u = TextureAddressMode::Wrap;
        mesh.material.stage0_mapping.address_v = TextureAddressMode::Wrap;

        // Standard blending for solid objects
        mesh.material.blend_mode = BlendMode::Opaque;
        mesh.material.alpha_test_enabled = false;

        mesh
    }

    /// Load well-known C&C models - updated with actual file names from BIG archives
    pub async fn load_cnc_model(
        &self,
        archive_system: &mut ArchiveFileSystem,
        unit_name: &str,
    ) -> Result<W3DModel> {
        // Loading C&C model (logging disabled)
        let model_name = match unit_name.to_lowercase().as_str() {
            // USA Units (av prefix = Army Vehicle, ab prefix = Army Building)
            "abrams" | "m1a1" | "tank" | "usa_tank" => "avcrusader", // Crusader tank (main US tank)
            "humvee" | "hummer" | "usa_humvee" => "avhummer",        // Army Humvee
            "chinook" | "helicopter" | "transport" => "avchinook",   // Transport helicopter
            "patriot" | "missile" | "aa" => "avpatriot", // Patriot AA system (if exists)
            "ranger" | "soldier" | "infantry" => "airanger", // Infantry (if exists)
            "paladin" | "artillery" => "avpaladin",      // Artillery (if exists)
            "raptor" | "fighter" => "avraptor",          // Fighter jet (if exists)
            "stealth" | "bomber" => "avstealth",         // Stealth bomber (if exists)
            "crusader" => "avcrusader",                  // Crusader tank
            "comanche" => "avcomanche",                  // Attack helicopter

            // China Units (nv prefix = Navy Vehicle, nb prefix = Navy Building)
            "battlemaster" | "china_tank" => "nvbattlemaster", // Chinese main battle tank (if exists)
            "dragon" | "dragon_tank" => "nvdragon",            // Dragon tank (if exists)
            "overlord" | "heavy_tank" => "nvoverlord",         // Overlord heavy tank (if exists)
            "gattling" | "gattling_cannon" => "nvgatttank",    // Gattling tank
            "mig" | "fighter_jet" => "nvmign",                 // MiG fighter
            "helix" | "china_helicopter" => "nvhelix",         // Helix helicopter

            // GLA Units (uv prefix = Utility Vehicle, ub prefix = Utility Building)
            "marauder" | "gla_tank" => "uvmarauder", // GLA main tank (if exists)
            "scorpion" | "scorpion_tank" => "uvscorpion", // Scorpion tank
            "technical" | "truck" | "gla_vehicle" => "uvtechnical", // Technical truck (if exists)
            "toxin" | "toxin_tractor" => "uvtoxintrk", // Toxin tractor
            "scud" | "scud_launcher" => "uvscudlchr", // SCUD launcher (if exists)
            "quad" | "quad_cannon" => "uvquadcannon", // Quad cannon (if exists)

            // Test units - use known good models
            "test_tank" => "uvscorpion",  // Use Scorpion as test tank
            "test_vehicle" => "avhummer", // Use Humvee as test vehicle
            "test_air" => "nvhelix",      // Use Helix as test aircraft

            _ => unit_name, // Try the name as-is
        };

        debug!("Loading C&C model: {} -> {}", unit_name, model_name);

        // Try to load the actual model, fall back to placeholder if not found
        match self.load_model(archive_system, model_name).await {
            Ok(model) => Ok(model),
            Err(e) => {
                warn!(
                    "Failed to load C&C model {}: {}, using fallback",
                    model_name, e
                );
                let mut fallback = W3DModel::new(format!("fallback_{}", unit_name));
                fallback
                    .meshes
                    .push(self.create_fallback_mesh(format!("{}_mesh", unit_name)));
                fallback.calculate_bounding_box();
                Ok(fallback)
            }
        }
    }

    /// List available W3D models in archives
    pub fn list_available_models(&self, archive_system: &ArchiveFileSystem) -> Vec<String> {
        let mut models = Vec::new();
        let all_files = archive_system.list_all_files();

        for file in all_files {
            if file.to_lowercase().ends_with(".w3d") {
                models.push(file);
            }
        }

        models.sort();
        models
    }
}

/// Mesh header structure
#[derive(Debug)]
struct MeshHeader {
    pub version: u32,
    pub flags: u32,
    pub num_triangles: u32,
    pub num_vertices: u32,
    pub mesh_name: String,
}

/// Get common C&C unit models - updated with actual units found in archives
pub fn get_common_cnc_units() -> Vec<&'static str> {
    vec![
        // USA Units
        "humvee",   // avhummer - Confirmed exists
        "crusader", // avcrusader - Confirmed exists
        "chinook",  // avchinook - Confirmed exists
        "comanche", // avcomanche - Attack helicopter
        "abrams",   // Maps to crusader (main US tank)
        // China Units
        "mig",          // nvmign - Confirmed exists
        "helix",        // nvhelix - Confirmed exists
        "gattling",     // nvgatttank - Confirmed exists
        "battlemaster", // Chinese main battle tank
        "dragon",       // Dragon tank
        // GLA Units
        "scorpion",  // uvscorpion - Confirmed exists
        "toxin",     // uvtoxintrk - Confirmed exists
        "scud",      // SCUD launcher
        "technical", // Technical truck
        "marauder",  // GLA tank
        // Test units with confirmed models
        "test_tank",    // Uses uvscorpion
        "test_vehicle", // Uses avhummer
        "test_air",     // Uses nvhelix
    ]
}

fn deduplicate_stage_uv_layers(layers: Vec<Vec<[f32; 2]>>) -> (Vec<Vec<[f32; 2]>>, Vec<u8>) {
    const MAX_CHANNELS: usize = 4;
    let mut unique_layers: Vec<Vec<[f32; 2]>> = Vec::new();
    let mut stage_channels: Vec<u8> = Vec::new();
    let mut crc_map = HashMap::new();

    for coords in layers {
        if coords.is_empty() {
            if unique_layers.is_empty() {
                unique_layers.push(Vec::new());
            }
            stage_channels.push(0);
            continue;
        }

        let mut hasher = Hasher::new();
        hasher.update(bytemuck::cast_slice(&coords));
        let crc = hasher.finalize();

        let channel = if let Some(&existing) = crc_map.get(&crc) {
            existing
        } else {
            let assigned = if unique_layers.len() < MAX_CHANNELS {
                let ch = unique_layers.len() as u8;
                unique_layers.push(coords.clone());
                ch
            } else {
                (MAX_CHANNELS.saturating_sub(1)) as u8
            };
            crc_map.insert(crc, assigned);
            assigned
        };

        stage_channels.push(channel);
    }

    if unique_layers.is_empty() {
        unique_layers.push(Vec::new());
    }

    (unique_layers, stage_channels)
}

fn companion_root_proto_name(prototype_names: &[String], model_name: &str) -> Option<String> {
    let target = model_name.to_ascii_lowercase();
    let companion_targets: Vec<String> = companion_stem_variants(model_name)
        .into_iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();
    prototype_names.iter().find_map(|name| {
        let lower = name.to_ascii_lowercase();
        if lower == target
            || companion_targets
                .iter()
                .any(|companion| lower == *companion || lower.starts_with(&format!("{companion}.")))
        {
            Some(name.clone())
        } else {
            None
        }
    })
}

fn collect_companion_prefixed_prototypes(
    prototype_names: &[String],
    roots: &[&str],
) -> Vec<String> {
    let mut matches = Vec::new();
    let mut seen = HashSet::new();
    let mut prefixes = Vec::new();

    for root in roots {
        for variant in companion_stem_variants(root) {
            prefixes.push(variant.to_ascii_lowercase());
        }
    }

    for name in prototype_names {
        let lower = name.to_ascii_lowercase();
        if prefixes
            .iter()
            .any(|prefix| lower.starts_with(&format!("{prefix}.")))
            && seen.insert(lower)
        {
            matches.push(name.clone());
        }
    }

    matches
}

fn companion_stem_variants(stem: &str) -> Vec<String> {
    const FAMILY_SUFFIXES: &[&str] = &[
        "MSH", "_MSH", "SK", "_SK", "SKN", "_SKN", "SKN2", "_SKN2", "SKNP", "_SKNP", "SKL", "_SKL",
    ];

    let mut variants = Vec::new();
    let mut seen = HashSet::new();
    let stem_upper = stem.to_ascii_uppercase();

    let mut push = |candidate: String| {
        if seen.insert(candidate.to_ascii_lowercase()) {
            variants.push(candidate);
        }
    };

    push(stem.to_string());

    for suffix in FAMILY_SUFFIXES {
        push(format!("{stem}{suffix}"));
    }

    for suffix in FAMILY_SUFFIXES {
        if let Some(prefix) = stem_upper.strip_suffix(suffix) {
            let base_len = prefix.len();
            let base = &stem[..base_len];
            for replacement in FAMILY_SUFFIXES {
                if replacement.eq_ignore_ascii_case(suffix) {
                    continue;
                }
                push(format!("{base}{replacement}"));
            }
        }
    }

    variants
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicate_stage_uv_layers_merges_duplicate_channels() {
        let stage0 = vec![[0.0, 0.0], [1.0, 0.0]];
        let stage1 = stage0.clone();
        let stage2 = vec![[0.5, 0.5], [0.75, 0.75]];
        let layers = vec![stage0.clone(), stage1, stage2.clone()];
        let (unique_layers, stage_channels) = deduplicate_stage_uv_layers(layers);

        assert_eq!(unique_layers.len(), 2);
        assert_eq!(unique_layers[0], stage0);
        assert_eq!(unique_layers[1], stage2);
        assert_eq!(stage_channels, vec![0, 0, 1]);
    }

    #[test]
    fn apply_material_stage_mappings_sets_texture_and_uv_source() {
        let mut material = W3DMaterial::default();
        let mut mesh = W3DMesh::new("TestMesh".to_string());
        mesh.stage_uv_channels = vec![0, 2];
        mesh.per_pass_stage_texture_names = vec![vec![
            vec!["base.dds".to_string()],
            vec!["detail.dds".to_string()],
        ]];

        W3DLoader::apply_material_stage_mappings(&mut material, &mesh);

        assert_eq!(
            material.stage0_mapping.texture_name.as_deref(),
            Some("base.dds")
        );
        assert!(matches!(material.stage0_mapping.uv_source, UVSource::UV0));
        let stage1 = material
            .stage1_mapping
            .as_ref()
            .expect("stage 1 mapping missing");
        assert_eq!(stage1.texture_name.as_deref(), Some("detail.dds"));
        assert!(matches!(stage1.uv_source, UVSource::UV2));
    }
}
