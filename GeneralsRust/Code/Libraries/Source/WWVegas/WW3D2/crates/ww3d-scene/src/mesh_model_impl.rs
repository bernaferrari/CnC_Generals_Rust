//! Concrete Mesh Model Implementation - Package 7
//!
//! Full implementation of RenderObjClassExt for mesh models with:
//! - Skinning and deformation (get_deformed_vertices)
//! - Bone attachments
//! - Animation playback
//! - Material/texture replacement
//! - LOD management
//!
//! C++ Reference: /Code/Libraries/Source/W3D/Renderer3D/meshmodel.cpp lines 1-1500

use crate::htree::HTree;
use crate::render_object_ext::*;
use glam::{Mat4, Vec3, Vec4};
use std::any::Any;
use std::collections::HashMap;

/// Skin data for mesh deformation
/// SIMPLIFIED to match C++ single-bone-per-vertex model
/// C++ Reference: meshgeometry.cpp lines 1937-1953 (W3dVertInfStruct with single BoneIdx)
#[derive(Debug, Clone)]
pub struct SkinData {
    /// Single bone index per vertex (matches C++ VertexBoneLink array)
    /// C++ Reference: meshgeometry.h uint16 *VertexBoneLink
    pub bone_links: Vec<u16>,
    /// Bone names for lookup
    pub bone_names: Vec<String>,
}

impl SkinData {
    pub fn new() -> Self {
        Self {
            bone_links: Vec::new(),
            bone_names: Vec::new(),
        }
    }

    /// Get bone index for a vertex
    pub fn get_bone_index(&self, vertex_index: usize) -> Option<u16> {
        self.bone_links.get(vertex_index).copied()
    }

    /// Find bone by name
    pub fn find_bone_index(&self, bone_name: &str) -> Option<usize> {
        self.bone_names.iter().position(|name| name == bone_name)
    }
}

/// Mesh geometry data
/// C++ Reference: meshmodel.cpp lines 100-200
#[derive(Debug, Clone)]
pub struct MeshGeometry {
    /// Base vertex positions
    pub vertices: Vec<Vec3>,
    /// Vertex normals
    pub normals: Vec<Vec3>,
    /// Texture coordinates
    pub uvs: Vec<(f32, f32)>,
    /// Triangle indices
    pub indices: Vec<u32>,
}

impl MeshGeometry {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Compute bounding box from vertices
    /// C++ Reference: meshmodel.cpp lines 210-240
    pub fn compute_bounding_box(&self) -> AABoxClass {
        if self.vertices.is_empty() {
            return AABoxClass::empty();
        }

        let mut min = self.vertices[0];
        let mut max = self.vertices[0];

        for vertex in &self.vertices {
            min = min.min(*vertex);
            max = max.max(*vertex);
        }

        AABoxClass::new(min, max)
    }

    /// Compute bounding sphere from vertices
    /// C++ Reference: meshmodel.cpp lines 242-270
    pub fn compute_bounding_sphere(&self) -> SphereClass {
        if self.vertices.is_empty() {
            return SphereClass::new(Vec3::ZERO, 0.0);
        }

        // Compute center
        let mut center = Vec3::ZERO;
        for vertex in &self.vertices {
            center += *vertex;
        }
        center /= self.vertices.len() as f32;

        // Find max distance from center
        let mut max_radius_sq = 0.0;
        for vertex in &self.vertices {
            let dist_sq = (*vertex - center).length_squared();
            if dist_sq > max_radius_sq {
                max_radius_sq = dist_sq;
            }
        }

        SphereClass::new(center, max_radius_sq.sqrt())
    }
}

/// Material pass definition
/// C++ Reference: meshmodel.cpp lines 1200-1250
#[derive(Debug, Clone)]
pub struct MaterialPass {
    pub material: Material,
    pub textures: Vec<TextureId>,
    pub blend_mode: BlendMode,
    /// Pass index for multi-pass rendering
    /// Used to filter which polygons render in which pass
    /// C++ Reference: dx8polygonrenderer.h line 74 (DX8PolygonRendererClass::pass)
    pub pass_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Opaque,
    AlphaBlend,
    Additive,
    Multiply,
}

impl MaterialPass {
    pub fn new() -> Self {
        Self {
            material: Material::default(),
            textures: Vec::new(),
            blend_mode: BlendMode::Opaque,
            pass_index: 0,
        }
    }

    /// Create a new material pass with a specific pass index
    /// C++ Reference: dx8polygonrenderer.cpp lines 47-68
    pub fn new_with_pass(pass_index: usize) -> Self {
        Self {
            material: Material::default(),
            textures: Vec::new(),
            blend_mode: BlendMode::Opaque,
            pass_index,
        }
    }

    /// Get the pass index for this material pass
    pub fn get_pass_index(&self) -> usize {
        self.pass_index
    }

    /// Set the pass index for this material pass
    pub fn set_pass_index(&mut self, pass_index: usize) {
        self.pass_index = pass_index;
    }
}

/// Source animation metadata used by the scene-level animation wrapper.
///
/// C++ render objects store an HAnimClass pointer and query Get_Num_Frames()
/// and Get_Frame_Rate() from it. The Rust scene facade stores those values
/// explicitly because this crate does not own the asset manager.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationMetadata {
    pub frame_count: u32,
    pub frame_rate: f32,
}

impl AnimationMetadata {
    pub fn new(frame_count: u32, frame_rate: f32) -> Self {
        Self {
            frame_count,
            frame_rate,
        }
    }

    pub fn is_valid(self) -> bool {
        self.frame_count > 0 && self.frame_rate > 0.0
    }
}

/// Complete mesh model with all features
/// C++ Reference: meshmodel.h lines 1-350
#[derive(Debug)]
pub struct MeshModel {
    /// Object name
    name: String,
    /// World transform
    transform: Mat4,
    /// Mesh geometry
    geometry: MeshGeometry,
    /// Skinning data (optional)
    skin_data: Option<SkinData>,
    /// Bone hierarchy for animation
    hierarchy: Option<HTree>,
    /// Current animation state
    animation_state: AnimationState,
    /// Metadata for animations known to this render object.
    animation_metadata: HashMap<String, AnimationMetadata>,
    /// Bone attachments
    attachments: Vec<BoneAttachment>,
    /// Material passes
    material_passes: Vec<MaterialPass>,
    /// Hidden state
    hidden: bool,
    /// Current LOD level
    lod_level: usize,
    /// Damage state (0=pristine, 1=minor damage, 2=medium damage, 3=heavy damage, 4=destroyed)
    /// C++ Reference: meshmodel.cpp lines 450-475 (MeshModelClass::Set_Damage_State)
    damage_state: u8,
    /// Map of damage state index to mesh variants (geometry per damage level)
    /// When damage_state changes, we select the appropriate mesh variant
    damage_state_meshes: HashMap<u8, MeshGeometry>,
    /// Container this mesh belongs to (from W3D header container_name field)
    /// C++ Reference: w3d_file.h W3dMeshHeader3Struct line 54
    container_name: Option<String>,
    /// Parent container transform (for hierarchical rendering)
    parent_transform: Mat4,
    /// Cached bounding volumes
    cached_bbox: AABoxClass,
    cached_sphere: SphereClass,
    /// Cached bone transforms for current frame
    cached_bone_transforms: HashMap<String, Mat4>,
}

impl MeshModel {
    /// Create a new mesh model
    /// C++ Reference: meshmodel.cpp lines 50-85
    pub fn new(name: String) -> Self {
        Self {
            name,
            transform: Mat4::IDENTITY,
            geometry: MeshGeometry::new(),
            skin_data: None,
            hierarchy: None,
            animation_state: AnimationState::default(),
            animation_metadata: HashMap::new(),
            attachments: Vec::new(),
            material_passes: Vec::new(),
            hidden: false,
            lod_level: 0,
            damage_state: 0, // Pristine condition by default
            damage_state_meshes: HashMap::new(),
            container_name: None,
            parent_transform: Mat4::IDENTITY,
            cached_bbox: AABoxClass::from_center_extent(Vec3::ZERO, Vec3::splat(1.0)),
            cached_sphere: SphereClass::new(Vec3::ZERO, 1.0),
            cached_bone_transforms: HashMap::new(),
        }
    }

    /// Set mesh geometry
    pub fn set_geometry(&mut self, geometry: MeshGeometry) {
        self.geometry = geometry;
        self.update_bounding_volumes();
    }

    /// Set skin data for deformation
    pub fn set_skin_data(&mut self, skin_data: SkinData) {
        self.skin_data = Some(skin_data);
    }

    /// Set bone hierarchy
    pub fn set_hierarchy(&mut self, hierarchy: HTree) {
        self.hierarchy = Some(hierarchy);
    }

    pub fn register_animation_metadata(
        &mut self,
        anim_name: impl Into<String>,
        frame_count: u32,
        frame_rate: f32,
    ) {
        let metadata = AnimationMetadata::new(frame_count, frame_rate);
        if metadata.is_valid() {
            self.animation_metadata.insert(anim_name.into(), metadata);
        }
    }

    pub fn play_animation_with_metadata(
        &mut self,
        anim_name: &str,
        mode: AnimationMode,
        frame_count: u32,
        frame_rate: f32,
    ) {
        self.register_animation_metadata(anim_name, frame_count, frame_rate);
        self.play_animation(anim_name, mode);
    }

    pub fn get_animation_metadata(&self, anim_name: &str) -> Option<AnimationMetadata> {
        self.animation_metadata.get(anim_name).copied()
    }

    /// Get material passes
    pub fn get_material_passes(&self) -> &[MaterialPass] {
        &self.material_passes
    }

    /// Get mutable material passes
    pub fn get_material_passes_mut(&mut self) -> &mut Vec<MaterialPass> {
        &mut self.material_passes
    }

    /// Update animation and bone transforms
    /// C++ Reference: meshmodel.cpp lines 300-380
    pub fn update(&mut self, delta_time: f32) {
        // Update animation state (always, regardless of hierarchy)
        self.animation_state.update(delta_time, 30.0); // Assume 30 FPS

        let root_transform = self.get_effective_transform();

        // Update bone transforms from hierarchy if present.
        if let Some(hierarchy) = &mut self.hierarchy {
            hierarchy.base_update(&root_transform);
            self.cached_bone_transforms.clear();

            for pivot_index in 0..hierarchy.num_pivots() {
                if let (Some(bone_name), Some(transform)) = (
                    hierarchy.get_bone_name(pivot_index),
                    hierarchy.get_transform(pivot_index),
                ) {
                    self.cached_bone_transforms
                        .insert(bone_name.to_string(), *transform);
                }
            }
        }

        // Update bone attachments
        self.update_attachments(&self.cached_bone_transforms.clone());
    }

    /// Get deformed vertices with single-bone skinning (C++ parity)
    /// This is the critical method for collision detection and physics
    /// C++ Reference: meshgeometry.cpp lines 2076-2093 (VectorProcessorClass::Transform)
    /// SIMPLIFIED: Uses single bone index per vertex, matching C++ implementation
    /// OPTIMIZED: Batches vertices by bone to reduce HashMap lookups (O(bones) vs O(vertices))
    fn compute_deformed_vertices(&self) -> Vec<Vec3> {
        if let Some(skin_data) = &self.skin_data {
            let vertex_count = self.geometry.vertex_count();
            // Start with base vertices, then transform only those with bone assignments
            let mut deformed = self.geometry.vertices.clone();

            // OPTIMIZATION: Build batches of vertices per bone
            // This reduces HashMap lookups from O(num_vertices) to O(num_bones)
            // For a mesh with 1000 vertices using 10 bones: 1000 lookups -> 10 lookups
            use std::collections::HashMap;
            let mut bone_batches: HashMap<u16, Vec<usize>> = HashMap::new();

            for (vertex_idx, bone_idx) in skin_data.bone_links.iter().enumerate() {
                if vertex_idx < vertex_count {
                    bone_batches
                        .entry(*bone_idx)
                        .or_insert_with(Vec::new)
                        .push(vertex_idx);
                }
            }

            // Process each bone's vertices in batch
            for (bone_idx, vertex_indices) in bone_batches {
                if let Some(bone_name) = skin_data.bone_names.get(bone_idx as usize) {
                    if let Some(bone_transform) = self.cached_bone_transforms.get(bone_name) {
                        // Transform all vertices for this bone at once
                        for vertex_idx in vertex_indices {
                            deformed[vertex_idx] =
                                bone_transform.transform_point3(self.geometry.vertices[vertex_idx]);
                        }
                    }
                    // If bone not found, vertices remain at base position (already set)
                }
                // If invalid bone index, vertices remain at base position (already set)
            }

            deformed
        } else {
            // Not a skinned mesh, return base vertices
            self.geometry.vertices.clone()
        }
    }

    /// Get deformed normals with single-bone skinning (C++ parity)
    /// CRITICAL FIX: Uses rotation-only matrix (translation zeroed)
    /// C++ Reference: meshgeometry.cpp lines 2090 (mytm.Set_Translation to zero)
    /// OPTIMIZED: Batches normals by bone to reduce HashMap lookups (O(bones) vs O(normals))
    fn compute_deformed_normals(&self) -> Vec<Vec3> {
        if let Some(skin_data) = &self.skin_data {
            let normal_count = self.geometry.normals.len();
            // Start with base normals, then transform only those with bone assignments
            let mut deformed = self.geometry.normals.clone();

            // OPTIMIZATION: Build batches of normals per bone (same as vertices)
            use std::collections::HashMap;
            let mut bone_batches: HashMap<u16, Vec<usize>> = HashMap::new();

            for (normal_idx, bone_idx) in skin_data.bone_links.iter().enumerate().take(normal_count)
            {
                bone_batches
                    .entry(*bone_idx)
                    .or_insert_with(Vec::new)
                    .push(normal_idx);
            }

            // Process each bone's normals in batch
            for (bone_idx, normal_indices) in bone_batches {
                if let Some(bone_name) = skin_data.bone_names.get(bone_idx as usize) {
                    if let Some(bone_transform) = self.cached_bone_transforms.get(bone_name) {
                        // Extract rotation-only matrix (zero translation) ONCE per bone
                        // C++ does: mytm.Set_Translation(Vector3(0.0f,0.0f,0.0f))
                        let mut rotation_only = *bone_transform;
                        rotation_only.w_axis = Vec4::new(0.0, 0.0, 0.0, 1.0);

                        // Transform all normals for this bone at once
                        for normal_idx in normal_indices {
                            let transformed =
                                rotation_only.transform_vector3(self.geometry.normals[normal_idx]);
                            deformed[normal_idx] = transformed.normalize();
                        }
                    }
                    // If bone not found, normals remain at base orientation (already set)
                }
                // If invalid bone index, normals remain at base orientation (already set)
            }

            deformed
        } else {
            // Not a skinned mesh, return base normals
            self.geometry.normals.clone()
        }
    }

    /// Set damage state mesh variant
    /// C++ Reference: meshmodel.cpp lines 450-475
    pub fn set_damage_state_mesh(&mut self, damage_level: u8, geometry: MeshGeometry) {
        self.damage_state_meshes.insert(damage_level, geometry);
    }

    /// Get the current damage state
    /// C++ Reference: meshmodel.h lines 220-230
    pub fn get_damage_state(&self) -> u8 {
        self.damage_state
    }

    /// Set the damage state and switch to corresponding mesh variant
    /// C++ Reference: meshmodel.cpp lines 450-475 (MeshModelClass::Set_Damage_State)
    pub fn set_damage_state(&mut self, damage_level: u8) {
        // Clamp damage level to valid range (0-4)
        let clamped_level = std::cmp::min(damage_level, 4);
        self.damage_state = clamped_level;

        // Switch to the appropriate mesh geometry if available
        if let Some(damage_geometry) = self.damage_state_meshes.get(&clamped_level) {
            // Replace geometry but keep the transform and other properties
            self.geometry = damage_geometry.clone();
            self.update_bounding_volumes();
        }
        // If no damage mesh is available for this level, keep the current geometry
    }

    /// Check if this model has damage states configured
    pub fn has_damage_states(&self) -> bool {
        !self.damage_state_meshes.is_empty()
    }

    /// Get the number of available damage states
    pub fn damage_state_count(&self) -> usize {
        self.damage_state_meshes.len()
    }

    /// Update LOD level based on distance from camera
    /// C++ Reference: meshmodel.cpp lines 650-750 (MeshModelClass::Update_LOD)
    /// This implements proper distance-based LOD selection instead of just using the first available
    pub fn update_lod_by_distance(&mut self, camera_position: Vec3, lod_scales: &[f32]) {
        // Extract object center from transform matrix (column 3, first 3 components)
        let object_center = Vec3::new(
            self.transform.col(3).x,
            self.transform.col(3).y,
            self.transform.col(3).z,
        );
        let distance = (camera_position - object_center).length();

        // Select appropriate LOD based on distance
        let mut new_lod = 0;
        for (i, &threshold) in lod_scales.iter().enumerate() {
            if distance >= threshold {
                new_lod = i;
            } else {
                break;
            }
        }

        // Clamp to valid LOD range
        let max_lod = lod_scales.len().saturating_sub(1);
        self.lod_level = std::cmp::min(new_lod, max_lod);
    }

    /// Set this mesh as part of a container (from W3D header)
    /// C++ Reference: w3d_file.h W3dMeshHeader3Struct container_name field
    pub fn set_container_name(&mut self, container_name: String) {
        if !container_name.is_empty() && container_name != "\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0" {
            self.container_name = Some(container_name);
        }
    }

    /// Get container name this mesh belongs to
    pub fn get_container_name(&self) -> Option<&str> {
        self.container_name.as_deref()
    }

    /// Set parent container transform (applied during rendering)
    /// When rendering, the effective transform is: parent_transform * local_transform
    pub fn set_parent_transform(&mut self, parent_transform: Mat4) {
        self.parent_transform = parent_transform;
    }

    /// Get parent container transform
    pub fn get_parent_transform(&self) -> Mat4 {
        self.parent_transform
    }

    /// Get the effective transform combining parent and local transforms
    /// Used during rendering to position the mesh in world space
    pub fn get_effective_transform(&self) -> Mat4 {
        self.parent_transform * self.transform
    }

    /// Get the number of rendering passes for this mesh
    /// C++ Reference: meshmodel.h Get_Pass_Count()
    pub fn get_pass_count(&self) -> usize {
        if self.material_passes.is_empty() {
            1 // Default to 1 pass if no material passes defined
        } else {
            // Find the maximum pass index + 1
            self.material_passes
                .iter()
                .map(|pass| pass.pass_index + 1)
                .max()
                .unwrap_or(1)
        }
    }

    /// Get material passes for a specific rendering pass
    /// C++ Reference: dx8renderer.cpp lines 1225-1240 (filtering polygons by pass)
    pub fn get_passes_for_render_pass(&self, pass_index: usize) -> Vec<&MaterialPass> {
        self.material_passes
            .iter()
            .filter(|pass| pass.pass_index == pass_index)
            .collect()
    }

    /// Check if this mesh has any material passes for a given rendering pass
    pub fn has_passes_for_render_pass(&self, pass_index: usize) -> bool {
        self.material_passes
            .iter()
            .any(|pass| pass.pass_index == pass_index)
    }

    /// Render this mesh for a specific rendering pass
    /// This is the public API for multi-pass rendering
    /// Only renders material passes that match the current rendering pass
    /// C++ Reference: dx8renderer.cpp lines 1225-1240 (pass filtering in rendering loop)
    pub fn render_pass(&self, context: &RenderContext, pass_index: usize) {
        if self.hidden {
            return;
        }

        // Filter material passes for the current rendering pass
        let passes_for_current_pass = self.get_passes_for_render_pass(pass_index);

        if passes_for_current_pass.is_empty() {
            return; // Nothing to render in this pass
        }

        let _mvp = context.view_projection_matrix * self.transform;

        // Render only the material passes that belong to this rendering pass
        for _pass in passes_for_current_pass {
            // Submit geometry with material
            // In actual implementation:
            // 1. Install material pass (shader, textures, blend mode)
            // 2. Draw the geometry (vertices and indices)
            // 3. Restore previous state if needed
        }
    }
}

impl RenderObjClassExt for MeshModel {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn get_transform(&self) -> &Mat4 {
        &self.transform
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }

    fn play_animation(&mut self, anim_name: &str, mode: AnimationMode) {
        self.animation_state.animation_name = anim_name.to_string();
        self.animation_state.mode = mode;
        self.animation_state.current_frame = 0.0;
        self.animation_state.ping_pong_direction = 1.0;

        if let Some(metadata) = self.animation_metadata.get(anim_name).copied() {
            self.animation_state.frame_count = metadata.frame_count as f32;
            self.animation_state.frame_rate = metadata.frame_rate;
            self.animation_state.is_playing = true;
        } else {
            self.animation_state.frame_count = 0.0;
            self.animation_state.frame_rate = 0.0;
            self.animation_state.is_playing = false;
        }
    }

    fn stop_animation(&mut self) {
        self.animation_state.is_playing = false;
    }

    fn set_animation_frame(&mut self, frame: f32) {
        if self.animation_state.frame_count <= 0.0 {
            self.animation_state.current_frame = 0.0;
        } else {
            self.animation_state.current_frame =
                frame.clamp(0.0, self.animation_state.frame_count - 1.0);
        }
    }

    fn get_animation_frame(&self) -> f32 {
        self.animation_state.current_frame
    }

    fn is_animation_playing(&self) -> bool {
        self.animation_state.is_playing
    }

    fn set_animation_speed(&mut self, speed: f32) {
        self.animation_state.speed = speed;
    }

    fn set_texture(&mut self, stage: usize, texture: TextureId) {
        for pass in &mut self.material_passes {
            if stage < pass.textures.len() {
                pass.textures[stage] = texture;
            }
        }
    }

    fn get_texture(&self, stage: usize) -> Option<TextureId> {
        self.material_passes
            .first()
            .and_then(|pass| pass.textures.get(stage).copied())
    }

    fn replace_all_textures(&mut self, old_tex: TextureId, new_tex: TextureId) {
        for pass in &mut self.material_passes {
            for texture in &mut pass.textures {
                if *texture == old_tex {
                    *texture = new_tex;
                }
            }
        }
    }

    fn set_material(&mut self, material_index: usize, material: Material) {
        if let Some(pass) = self.material_passes.get_mut(material_index) {
            pass.material = material;
        }
    }

    fn get_material_count(&self) -> usize {
        self.material_passes.len()
    }

    fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    fn is_hidden(&self) -> bool {
        self.hidden
    }

    fn get_bounding_box(&self) -> AABoxClass {
        self.cached_bbox
    }

    fn get_bounding_sphere(&self) -> SphereClass {
        self.cached_sphere
    }

    fn update_bounding_volumes(&mut self) {
        // CRITICAL: For skinned meshes, compute bounds from deformed vertices!
        // C++ Reference: meshgeometry.cpp - bounds must reflect current pose, not base mesh
        if self.skin_data.is_some() {
            let deformed_verts = self.compute_deformed_vertices();

            // Compute bounding box from deformed vertices
            if !deformed_verts.is_empty() {
                let mut min = deformed_verts[0];
                let mut max = deformed_verts[0];

                for v in &deformed_verts {
                    min = min.min(*v);
                    max = max.max(*v);
                }

                self.cached_bbox = AABoxClass { min, max };

                // Compute bounding sphere from deformed vertices
                let center = (min + max) * 0.5;
                let mut max_dist_sq = 0.0f32;

                for v in &deformed_verts {
                    let dist_sq = (*v - center).length_squared();
                    max_dist_sq = max_dist_sq.max(dist_sq);
                }

                self.cached_sphere = SphereClass {
                    center,
                    radius: max_dist_sq.sqrt(),
                };
            } else {
                // Fallback to base geometry if no deformed vertices
                self.cached_bbox = self.geometry.compute_bounding_box();
                self.cached_sphere = self.geometry.compute_bounding_sphere();
            }
        } else {
            // Not a skinned mesh, use base geometry bounds
            self.cached_bbox = self.geometry.compute_bounding_box();
            self.cached_sphere = self.geometry.compute_bounding_sphere();
        }
    }

    fn render(&self, context: &RenderContext) {
        if self.hidden {
            return;
        }

        // In a real implementation, this would submit draw calls to the GPU
        // For now, this is a placeholder that demonstrates the API
        let _mvp = context.view_projection_matrix * self.transform;

        // Render each material pass
        // Note: The actual multi-pass filtering happens in render_pass
        for _pass in &self.material_passes {
            // Submit geometry with material
        }
    }

    fn render_with_pass_filter(&self, context: &RenderContext, pass_index: usize) {
        if self.hidden {
            return;
        }

        // Filter material passes for the current rendering pass
        let passes_for_current_pass: Vec<_> = self
            .material_passes
            .iter()
            .enumerate()
            .filter(|(_, pass)| pass.pass_index == pass_index)
            .collect();

        if passes_for_current_pass.is_empty() {
            return;
        }

        let _mvp = context.view_projection_matrix * self.transform;

        // Render only the material passes that belong to this rendering pass
        for (_idx, _pass) in passes_for_current_pass {
            // Submit geometry with material
        }
    }

    fn get_polygon_count(&self) -> usize {
        self.geometry.triangle_count()
    }

    fn get_vertex_count(&self) -> usize {
        self.geometry.vertex_count()
    }

    fn has_transparency(&self) -> bool {
        self.material_passes
            .iter()
            .any(|pass| pass.material.opacity < 1.0 || pass.blend_mode != BlendMode::Opaque)
    }

    fn get_deformed_vertices(&self) -> Option<Vec<Vec3>> {
        Some(self.compute_deformed_vertices())
    }

    fn get_deformed_normals(&self) -> Option<Vec<Vec3>> {
        Some(self.compute_deformed_normals())
    }

    fn attach_to_bone(&mut self, bone_name: &str, obj: Box<dyn RenderObjClassExt>) {
        self.attachments
            .push(BoneAttachment::new(bone_name.to_string(), obj));
    }

    fn detach_from_bone(&mut self, bone_name: &str) -> Option<Box<dyn RenderObjClassExt>> {
        if let Some(index) = self
            .attachments
            .iter()
            .position(|a| a.bone_name == bone_name)
        {
            Some(self.attachments.remove(index).object)
        } else {
            None
        }
    }

    fn get_bone_attachments(&self) -> &[BoneAttachment] {
        &self.attachments
    }

    fn get_bone_attachments_mut(&mut self) -> &mut Vec<BoneAttachment> {
        &mut self.attachments
    }

    fn set_lod_level(&mut self, level: usize) {
        self.lod_level = level;
    }

    fn get_lod_level(&self) -> usize {
        self.lod_level
    }

    fn clone_obj(&self) -> Box<dyn RenderObjClassExt> {
        Box::new(MeshModel {
            name: self.name.clone(),
            transform: self.transform,
            geometry: self.geometry.clone(),
            skin_data: self.skin_data.clone(),
            hierarchy: None, // Don't deep clone hierarchy
            animation_state: self.animation_state.clone(),
            animation_metadata: self.animation_metadata.clone(),
            attachments: Vec::new(), // Don't clone attachments
            material_passes: self.material_passes.clone(),
            hidden: self.hidden,
            lod_level: self.lod_level,
            damage_state: self.damage_state,
            damage_state_meshes: self.damage_state_meshes.clone(),
            container_name: self.container_name.clone(),
            parent_transform: self.parent_transform,
            cached_bbox: self.cached_bbox,
            cached_sphere: self.cached_sphere,
            cached_bone_transforms: HashMap::new(),
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_model_creation() {
        let model = MeshModel::new("TestModel".to_string());
        assert_eq!(model.get_name(), "TestModel");
        assert!(!model.is_hidden());
        assert_eq!(model.get_lod_level(), 0);
    }

    #[test]
    fn test_mesh_geometry() {
        let mut geometry = MeshGeometry::new();
        geometry.vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        geometry.indices = vec![0, 1, 2];

        assert_eq!(geometry.vertex_count(), 3);
        assert_eq!(geometry.triangle_count(), 1);
    }

    #[test]
    fn test_bounding_volumes() {
        let mut geometry = MeshGeometry::new();
        geometry.vertices = vec![Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0)];

        let bbox = geometry.compute_bounding_box();
        let sphere = geometry.compute_bounding_sphere();

        // Bounding volumes should contain all vertices
        assert!(sphere.radius > 0.0);
    }

    #[test]
    fn test_animation_control() {
        let mut model = MeshModel::new("AnimTest".to_string());

        model.play_animation_with_metadata("walk", AnimationMode::Loop, 30, 30.0);
        assert!(model.is_animation_playing());
        assert_eq!(model.get_animation_frame(), 0.0);

        model.set_animation_frame(15.0);
        assert_eq!(model.get_animation_frame(), 15.0);

        model.stop_animation();
        assert!(!model.is_animation_playing());
    }

    #[test]
    fn test_animation_missing_metadata_does_not_fabricate_frames() {
        let mut model = MeshModel::new("MissingAnim".to_string());

        model.play_animation("unknown", AnimationMode::Loop);

        assert!(!model.is_animation_playing());
        assert_eq!(model.get_animation_frame(), 0.0);

        model.set_animation_frame(12.0);
        assert_eq!(model.get_animation_frame(), 0.0);
    }

    #[test]
    fn test_animation_uses_registered_frame_rate_and_count() {
        let mut model = MeshModel::new("SlowAnim".to_string());
        model.play_animation_with_metadata("slow", AnimationMode::Loop, 20, 10.0);

        model.update(1.0);

        assert!(model.is_animation_playing());
        assert_eq!(model.get_animation_frame(), 10.0);

        model.set_animation_frame(99.0);
        assert_eq!(model.get_animation_frame(), 19.0);
    }

    #[test]
    fn test_texture_replacement() {
        let mut model = MeshModel::new("TexTest".to_string());

        let mut pass = MaterialPass::new();
        pass.textures = vec![TextureId::new(1), TextureId::new(2)];
        model.material_passes.push(pass);

        model.set_texture(0, TextureId::new(99));
        assert_eq!(model.get_texture(0), Some(TextureId::new(99)));
    }

    #[test]
    fn test_deformed_vertices_no_skin() {
        let mut model = MeshModel::new("NoSkin".to_string());
        let mut geometry = MeshGeometry::new();
        geometry.vertices = vec![
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ];
        model.set_geometry(geometry);

        let deformed = model.get_deformed_vertices().unwrap();
        assert_eq!(deformed.len(), 3);
        // Without skinning, vertices should match base geometry
        assert_eq!(deformed[0], Vec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn test_skinned_vertices_use_hierarchy_base_transforms() {
        let mut model = MeshModel::new("Skinned".to_string());
        let mut geometry = MeshGeometry::new();
        geometry.vertices = vec![Vec3::new(1.0, 0.0, 0.0)];
        geometry.normals = vec![Vec3::Y];
        model.set_geometry(geometry);
        model.set_skin_data(SkinData {
            bone_links: vec![1],
            bone_names: vec!["RootTransform".to_string(), "Forearm".to_string()],
        });

        let mut hierarchy = HTree::new("Unit".to_string());
        hierarchy.init_default();
        hierarchy.add_pivot(
            "Forearm".to_string(),
            Some(0),
            Mat4::from_translation(Vec3::new(4.0, 0.0, 0.0)),
        );
        model.set_hierarchy(hierarchy);

        model.update(0.0);

        let deformed = model.get_deformed_vertices().unwrap();
        assert_eq!(deformed[0], Vec3::new(5.0, 0.0, 0.0));

        let normals = model.get_deformed_normals().unwrap();
        assert_eq!(normals[0], Vec3::Y);
    }

    #[test]
    fn test_bone_attachments_follow_hierarchy_base_transforms() {
        let mut model = MeshModel::new("Parent".to_string());
        model.attach_to_bone("Turret", Box::new(MeshModel::new("Child".to_string())));

        let mut hierarchy = HTree::new("Vehicle".to_string());
        hierarchy.init_default();
        hierarchy.add_pivot(
            "Turret".to_string(),
            Some(0),
            Mat4::from_translation(Vec3::new(0.0, 3.0, 0.0)),
        );
        model.set_hierarchy(hierarchy);

        model.update(0.0);

        let attachments = model.get_bone_attachments();
        assert_eq!(attachments.len(), 1);
        assert_eq!(
            attachments[0].object.get_transform().w_axis.truncate(),
            Vec3::new(0.0, 3.0, 0.0)
        );
    }

    #[test]
    fn test_bone_attachments() {
        let mut model = MeshModel::new("WithAttachments".to_string());
        let attachment = Box::new(MeshModel::new("Weapon".to_string()));

        model.attach_to_bone("hand_right", attachment);
        assert_eq!(model.get_bone_attachments().len(), 1);

        let detached = model.detach_from_bone("hand_right");
        assert!(detached.is_some());
        assert_eq!(model.get_bone_attachments().len(), 0);
    }

    #[test]
    fn test_material_properties() {
        let mut model = MeshModel::new("MatTest".to_string());

        let mut pass = MaterialPass::new();
        pass.material.opacity = 0.5;
        pass.blend_mode = BlendMode::AlphaBlend;
        model.material_passes.push(pass);

        assert!(model.has_transparency());
    }

    #[test]
    fn test_visibility() {
        let mut model = MeshModel::new("VisTest".to_string());
        assert!(!model.is_hidden());

        model.set_hidden(true);
        assert!(model.is_hidden());

        model.set_hidden(false);
        assert!(!model.is_hidden());
    }

    #[test]
    fn test_container_system() {
        let mut model = MeshModel::new("ContainerMesh".to_string());

        // Initially no container
        assert!(model.get_container_name().is_none());

        // Set container name
        model.set_container_name("turret_container".to_string());
        assert_eq!(model.get_container_name(), Some("turret_container"));

        // Parent transform starts as identity
        assert_eq!(model.get_parent_transform(), Mat4::IDENTITY);

        // Set parent transform
        let parent = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        model.set_parent_transform(parent);
        assert_eq!(model.get_parent_transform(), parent);

        // Local transform is separate
        let local = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0));
        model.set_transform(local);

        // Effective transform is parent * local
        let effective = model.get_effective_transform();
        let expected = parent * local;
        assert!((effective.col(3) - expected.col(3)).length() < 0.001);
    }

    #[test]
    fn test_container_empty_name_rejected() {
        let mut model = MeshModel::new("TestModel".to_string());

        // Empty string should not be set
        model.set_container_name("".to_string());
        assert!(model.get_container_name().is_none());

        // Null-padded string should not be set
        model.set_container_name("\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0".to_string());
        assert!(model.get_container_name().is_none());

        // Valid names should be set
        model.set_container_name("valid_container".to_string());
        assert!(model.get_container_name().is_some());
    }

    // ===== Multi-Pass Rendering Tests =====

    #[test]
    fn test_material_pass_index() {
        let mut pass = MaterialPass::new();
        assert_eq!(pass.get_pass_index(), 0);

        pass.set_pass_index(2);
        assert_eq!(pass.get_pass_index(), 2);
    }

    #[test]
    fn test_material_pass_new_with_pass() {
        let pass = MaterialPass::new_with_pass(3);
        assert_eq!(pass.get_pass_index(), 3);
    }

    #[test]
    fn test_mesh_pass_count_no_passes() {
        let model = MeshModel::new("TestModel".to_string());
        // Default to 1 pass when no material passes are defined
        assert_eq!(model.get_pass_count(), 1);
    }

    #[test]
    fn test_mesh_pass_count_single_pass() {
        let mut model = MeshModel::new("TestModel".to_string());

        let pass = MaterialPass::new_with_pass(0);
        model.material_passes.push(pass);

        assert_eq!(model.get_pass_count(), 1);
    }

    #[test]
    fn test_mesh_pass_count_multiple_passes() {
        let mut model = MeshModel::new("TestModel".to_string());

        // Add passes with indices 0, 1, 2
        model.material_passes.push(MaterialPass::new_with_pass(0));
        model.material_passes.push(MaterialPass::new_with_pass(1));
        model.material_passes.push(MaterialPass::new_with_pass(2));

        // Pass count should be max(pass_index) + 1 = 2 + 1 = 3
        assert_eq!(model.get_pass_count(), 3);
    }

    #[test]
    fn test_mesh_pass_count_non_sequential() {
        let mut model = MeshModel::new("TestModel".to_string());

        // Add passes with non-sequential indices (0, 3, 5)
        model.material_passes.push(MaterialPass::new_with_pass(0));
        model.material_passes.push(MaterialPass::new_with_pass(3));
        model.material_passes.push(MaterialPass::new_with_pass(5));

        // Pass count should be max(pass_index) + 1 = 5 + 1 = 6
        assert_eq!(model.get_pass_count(), 6);
    }

    #[test]
    fn test_get_passes_for_render_pass() {
        let mut model = MeshModel::new("TestModel".to_string());

        // Create material passes with different pass indices
        let mut pass0_a = MaterialPass::new_with_pass(0);
        pass0_a.blend_mode = BlendMode::Opaque;

        let mut pass0_b = MaterialPass::new_with_pass(0);
        pass0_b.blend_mode = BlendMode::AlphaBlend;

        let mut pass1 = MaterialPass::new_with_pass(1);
        pass1.blend_mode = BlendMode::Additive;

        let mut pass2 = MaterialPass::new_with_pass(2);
        pass2.blend_mode = BlendMode::Multiply;

        model.material_passes.push(pass0_a);
        model.material_passes.push(pass0_b);
        model.material_passes.push(pass1);
        model.material_passes.push(pass2);

        // Get passes for render pass 0 - should return 2 passes
        let passes_for_pass_0 = model.get_passes_for_render_pass(0);
        assert_eq!(passes_for_pass_0.len(), 2);
        assert_eq!(passes_for_pass_0[0].pass_index, 0);
        assert_eq!(passes_for_pass_0[1].pass_index, 0);

        // Get passes for render pass 1 - should return 1 pass
        let passes_for_pass_1 = model.get_passes_for_render_pass(1);
        assert_eq!(passes_for_pass_1.len(), 1);
        assert_eq!(passes_for_pass_1[0].pass_index, 1);

        // Get passes for render pass 2 - should return 1 pass
        let passes_for_pass_2 = model.get_passes_for_render_pass(2);
        assert_eq!(passes_for_pass_2.len(), 1);
        assert_eq!(passes_for_pass_2[0].pass_index, 2);

        // Get passes for render pass 3 - should return 0 passes
        let passes_for_pass_3 = model.get_passes_for_render_pass(3);
        assert_eq!(passes_for_pass_3.len(), 0);
    }

    #[test]
    fn test_has_passes_for_render_pass() {
        let mut model = MeshModel::new("TestModel".to_string());

        model.material_passes.push(MaterialPass::new_with_pass(0));
        model.material_passes.push(MaterialPass::new_with_pass(2));

        // Should have passes for index 0 and 2
        assert!(model.has_passes_for_render_pass(0));
        assert!(!model.has_passes_for_render_pass(1));
        assert!(model.has_passes_for_render_pass(2));
        assert!(!model.has_passes_for_render_pass(3));
    }

    #[test]
    fn test_multi_pass_polygon_separation() {
        // This test simulates the C++ behavior where different polygons
        // are assigned to different rendering passes
        let mut model = MeshModel::new("MultiPassModel".to_string());

        // Create a simple mesh with 3 triangles
        let mut geometry = MeshGeometry::new();
        geometry.vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
        ];
        geometry.indices = vec![
            0, 1, 2, // Triangle 0
            1, 3, 2, // Triangle 1
        ];
        model.set_geometry(geometry);

        // Assign base pass (pass 0) with opaque blend mode
        let mut pass0 = MaterialPass::new_with_pass(0);
        pass0.blend_mode = BlendMode::Opaque;

        // Assign detail pass (pass 1) with alpha blend
        let mut pass1 = MaterialPass::new_with_pass(1);
        pass1.blend_mode = BlendMode::AlphaBlend;

        // Assign emissive pass (pass 2) with additive blend
        let mut pass2 = MaterialPass::new_with_pass(2);
        pass2.blend_mode = BlendMode::Additive;

        model.material_passes.push(pass0);
        model.material_passes.push(pass1);
        model.material_passes.push(pass2);

        // Verify pass count
        assert_eq!(model.get_pass_count(), 3);

        // Verify each pass has exactly one material pass
        assert_eq!(model.get_passes_for_render_pass(0).len(), 1);
        assert_eq!(model.get_passes_for_render_pass(1).len(), 1);
        assert_eq!(model.get_passes_for_render_pass(2).len(), 1);

        // Verify blend modes are correct for each pass
        let pass0_materials = model.get_passes_for_render_pass(0);
        assert_eq!(pass0_materials[0].blend_mode, BlendMode::Opaque);

        let pass1_materials = model.get_passes_for_render_pass(1);
        assert_eq!(pass1_materials[0].blend_mode, BlendMode::AlphaBlend);

        let pass2_materials = model.get_passes_for_render_pass(2);
        assert_eq!(pass2_materials[0].blend_mode, BlendMode::Additive);
    }

    #[test]
    fn test_multi_pass_rendering_order() {
        // Test that verifies the C++ behavior of rendering passes in order
        let mut model = MeshModel::new("OrderedModel".to_string());

        // Add passes in reverse order to test that filtering works by index, not order
        model.material_passes.push(MaterialPass::new_with_pass(2));
        model.material_passes.push(MaterialPass::new_with_pass(0));
        model.material_passes.push(MaterialPass::new_with_pass(1));

        // Rendering should filter by pass index, regardless of storage order
        let pass0 = model.get_passes_for_render_pass(0);
        assert_eq!(pass0.len(), 1);
        assert_eq!(pass0[0].pass_index, 0);

        let pass1 = model.get_passes_for_render_pass(1);
        assert_eq!(pass1.len(), 1);
        assert_eq!(pass1[0].pass_index, 1);

        let pass2 = model.get_passes_for_render_pass(2);
        assert_eq!(pass2.len(), 1);
        assert_eq!(pass2[0].pass_index, 2);
    }
}
