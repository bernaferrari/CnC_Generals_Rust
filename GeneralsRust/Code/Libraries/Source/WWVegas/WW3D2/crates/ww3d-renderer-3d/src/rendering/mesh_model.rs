// GNU General Public License v3.0 - See LICENSE file for details
// Command & Conquer Generals Zero Hour(tm)
// Copyright 2025 Electronic Arts Inc.
//
// Complete port of MeshModelClass from C++ (meshmdl.h/cpp)
// Original: meshmdl.h lines 154-348, meshmdl.cpp lines 40-760

use super::mesh_geometry::{GeometryFlags, MeshGeometry, TriIndex};
use super::mesh_mat_desc::{ColorSourceType, MeshMatDesc, MAX_PASSES, MAX_TEX_STAGES};
use super::shader_system::shader::ShaderClass;
use crate::material_system::VertexMaterialClass;
use crate::texture_system::TextureClass;
use glam::Vec2;
use std::sync::Arc;

/// Material info holds the collection of unique materials in the mesh
/// C++: MaterialInfoClass (matinfo.h)
/// Simplified for Rust - just tracks unique materials
#[derive(Debug, Clone)]
pub struct MaterialInfo {
    materials: Vec<Arc<VertexMaterialClass>>,
}

impl MaterialInfo {
    pub fn new() -> Self {
        Self {
            materials: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.materials.clear();
    }

    pub fn add_material(&mut self, material: Arc<VertexMaterialClass>) {
        // Only add if not already present
        if !self.materials.iter().any(|m| Arc::ptr_eq(m, &material)) {
            self.materials.push(material);
        }
    }

    pub fn get_materials(&self) -> &[Arc<VertexMaterialClass>] {
        &self.materials
    }
}

impl Default for MaterialInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Gap filler for N-Patch rendering
/// C++: GapFillerClass (meshmdl.h lines 126-152, meshmdl.cpp lines 451-760)
///
/// This class generates gap-filling polygons for N-Patched meshes to avoid
/// visible seams at hard edges. It allocates arrays for the maximum possible
/// number of gap polygons, then shrinks them after all gaps are added.
#[derive(Debug, Clone)]
pub struct GapFiller {
    // Parent mesh model (weak reference to avoid cycles)
    // C++ holds raw pointer, we use index/lookup
    parent_poly_count: usize,
    parent_pass_count: usize,

    // Gap polygon data - C++: lines 130-132
    polygon_array: Vec<TriIndex>,
    polygon_count: usize,
    array_size: usize,

    // Material data for gap polygons - C++: lines 133-135
    texture_array: Vec<Vec<Option<Vec<Option<Arc<TextureClass>>>>>>, // [pass][stage][polygon] - stage is Option to indicate presence
    material_array: Vec<Vec<Option<Arc<VertexMaterialClass>>>>,      // [pass][vertex]
    shader_array: Vec<Vec<ShaderClass>>,                             // [pass][polygon]
}

impl GapFiller {
    /// Create new gap filler
    /// C++: GapFillerClass constructor (meshmdl.cpp lines 451-481)
    pub fn new(
        parent_poly_count: usize,
        parent_pass_count: usize,
        has_texture_arrays: &[[bool; MAX_TEX_STAGES]; MAX_PASSES],
        has_material_array: &[bool; MAX_PASSES],
        has_shader_array: &[bool; MAX_PASSES],
    ) -> Self {
        // Each side of each triangle can have 2 gap polygons in worst case
        let array_size = parent_poly_count * 6;

        // Allocate texture arrays
        let mut texture_array = Vec::with_capacity(parent_pass_count);
        for pass in 0..parent_pass_count {
            let mut stage_vec = Vec::with_capacity(MAX_TEX_STAGES);
            for stage in 0..MAX_TEX_STAGES {
                if has_texture_arrays[pass][stage] {
                    stage_vec.push(Some(vec![None; array_size]));
                } else {
                    stage_vec.push(None);
                }
            }
            texture_array.push(stage_vec);
        }

        // Allocate material arrays
        let mut material_array = Vec::with_capacity(parent_pass_count);
        for pass in 0..parent_pass_count {
            if has_material_array[pass] {
                material_array.push(vec![None; array_size]);
            } else {
                material_array.push(Vec::new());
            }
        }

        // Allocate shader arrays
        let mut shader_array = Vec::with_capacity(parent_pass_count);
        for pass in 0..parent_pass_count {
            if has_shader_array[pass] {
                shader_array.push(vec![ShaderClass::default(); array_size]);
            } else {
                shader_array.push(Vec::new());
            }
        }

        Self {
            parent_poly_count,
            parent_pass_count,
            polygon_array: vec![[0, 0, 0]; array_size],
            polygon_count: 0,
            array_size,
            texture_array,
            material_array,
            shader_array,
        }
    }

    /// Add a gap polygon
    /// C++: Add_Polygon (meshmdl.h line 150, meshmdl.cpp lines 568-597)
    pub fn add_polygon(
        &mut self,
        source_polygon_index: usize,
        vidx1: u16,
        vidx2: u16,
        vidx3: u16,
        source_materials: &MeshMatDesc,
        source_polygons: &[TriIndex],
    ) {
        if self.polygon_count >= self.array_size {
            return;
        }

        // Add triangle
        self.polygon_array[self.polygon_count] = [vidx1, vidx2, vidx3];

        // Copy materials from source polygon
        for pass in 0..self.parent_pass_count {
            // Copy shader
            if pass < self.shader_array.len() && !self.shader_array[pass].is_empty() {
                let shader = source_materials.get_shader(pass, source_polygon_index);
                self.shader_array[pass][self.polygon_count] = shader;
            }

            // Copy material (per-vertex, use first vertex of source)
            if pass < self.material_array.len() && !self.material_array[pass].is_empty() {
                if let Some(source_poly) = source_polygons.get(source_polygon_index) {
                    let first_vertex = source_poly[0] as usize;
                    if let Some(mat) = source_materials.get_material(pass, first_vertex) {
                        self.material_array[pass][self.polygon_count] = Some(mat);
                    }
                }
            }

            // Copy textures
            for stage in 0..MAX_TEX_STAGES {
                if pass < self.texture_array.len() && stage < self.texture_array[pass].len() {
                    if let Some(tex_vec) = &mut self.texture_array[pass][stage] {
                        if let Some(tex) =
                            source_materials.get_texture(pass, stage, source_polygon_index)
                        {
                            tex_vec[self.polygon_count] = Some(tex);
                        }
                    }
                }
            }
        }

        self.polygon_count += 1;
    }

    /// Shrink buffers to exact size
    /// C++: Shrink_Buffers (meshmdl.h line 151, meshmdl.cpp lines 606-644)
    pub fn shrink_buffers(&mut self) {
        if self.polygon_count == self.array_size {
            return;
        }

        // Shrink polygon array
        self.polygon_array.truncate(self.polygon_count);

        // Shrink texture arrays
        for pass_textures in &mut self.texture_array {
            for stage_opt in pass_textures {
                if let Some(tex_vec) = stage_opt {
                    tex_vec.truncate(self.polygon_count);
                }
            }
        }

        // Shrink material arrays
        for mat_vec in &mut self.material_array {
            if !mat_vec.is_empty() {
                mat_vec.truncate(self.polygon_count);
            }
        }

        // Shrink shader arrays
        for shader_vec in &mut self.shader_array {
            if !shader_vec.is_empty() {
                shader_vec.truncate(self.polygon_count);
            }
        }

        self.array_size = self.polygon_count;
    }

    /// Get polygon array
    /// C++: Get_Polygon_Array (meshmdl.h line 144)
    pub fn get_polygon_array(&self) -> &[TriIndex] {
        &self.polygon_array[..self.polygon_count]
    }

    /// Get polygon count
    /// C++: Get_Polygon_Count (meshmdl.h line 145)
    pub fn get_polygon_count(&self) -> usize {
        self.polygon_count
    }

    /// Get texture array for pass/stage
    /// C++: Get_Texture_Array (meshmdl.h line 146)
    pub fn get_texture_array(
        &self,
        pass: usize,
        stage: usize,
    ) -> Option<&[Option<Arc<TextureClass>>]> {
        self.texture_array
            .get(pass)
            .and_then(|stages| stages.get(stage))
            .and_then(|opt| opt.as_ref().map(|v| v.as_slice()))
    }

    /// Get material array for pass
    /// C++: Get_Material_Array (meshmdl.h line 147)
    pub fn get_material_array(&self, pass: usize) -> Option<&[Option<Arc<VertexMaterialClass>>]> {
        self.material_array
            .get(pass)
            .filter(|v| !v.is_empty())
            .map(|v| v.as_slice())
    }

    /// Get shader array for pass
    /// C++: Get_Shader_Array (meshmdl.h line 148)
    pub fn get_shader_array(&self, pass: usize) -> Option<&[ShaderClass]> {
        self.shader_array
            .get(pass)
            .filter(|v| !v.is_empty())
            .map(|v| v.as_slice())
    }
}

/// MeshModel - repository for all geometry/material data
/// C++: MeshModelClass (meshmdl.h lines 154-348, meshmdl.cpp lines 71-760)
///
/// This class is the main data container for mesh rendering. It inherits from
/// MeshGeometry and adds material description, allowing multiple instances to
/// share geometry while having different materials.
///
/// Key design (from C++ comments lines 90-119):
/// - Geometry is shared via reference counting (Arc in Rust)
/// - Some arrays are ALWAYS SHARED (poly, bone influences)
/// - Some are SHARED UNTIL DEFORMED (vertex positions, normals)
/// - Materials/textures/UV are ALWAYS UNIQUE per material representation
#[derive(Debug, Clone)]
pub struct MeshModel {
    // Base geometry (inherited from MeshGeometryClass)
    // C++: MeshGeometryClass base class
    pub geometry: MeshGeometry,

    // Material descriptions - C++: lines 327-329
    // DefMatDesc: default material description (always present)
    // AlternateMatDesc: optional alternate materials
    // CurMatDesc: pointer to currently active description
    def_mat_desc: MeshMatDesc,
    alternate_mat_desc: Option<MeshMatDesc>,
    use_alternate: bool, // true if using alternate instead of default

    // Material info - C++: line 332
    mat_info: MaterialInfo,

    // Gap filler for N-Patch rendering - C++: line 338
    gap_filler: Option<GapFiller>,

    // Polygon renderers (for DX8 rendering system) - C++: line 335
    // In Rust we'll use a simpler approach since we're using WGPU
    // Just track if registered
    has_polygon_renderers: bool,
    has_been_in_use: bool, // C++: line 339 - for debugging
}

impl MeshModel {
    /// Create new mesh model
    /// C++: MeshModelClass constructor (meshmdl.cpp lines 71-86)
    pub fn new() -> Self {
        Self {
            geometry: MeshGeometry::new(),
            def_mat_desc: MeshMatDesc::new(),
            alternate_mat_desc: None,
            use_alternate: false,
            mat_info: MaterialInfo::new(),
            gap_filler: None,
            has_polygon_renderers: false,
            has_been_in_use: false,
        }
    }

    /// Reset to initial state with new counts
    /// C++: Reset (meshmdl.h line 165, meshmdl.cpp lines 157-179)
    pub fn reset(&mut self, poly_count: usize, vertex_count: usize, pass_count: usize) {
        // Delete gap filler BEFORE geometry reset (C++ comment line 159-160)
        self.gap_filler = None;

        // Reset geometry
        self.geometry.reset_geometry(poly_count, vertex_count);

        // Reset materials
        self.mat_info.reset();
        self.def_mat_desc
            .reset(poly_count, vertex_count, pass_count);
        self.alternate_mat_desc = None;
        self.use_alternate = false;

        // Clear rendering state
        self.has_polygon_renderers = false;
    }

    /// Register mesh for rendering
    /// C++: Register_For_Rendering (meshmdl.h line 166, meshmdl.cpp lines 181-205)
    pub fn register_for_rendering(&mut self) {
        self.has_been_in_use = true;
        // N-Patch gap filling initialization would go here
        // For now just mark as registered
        self.has_polygon_renderers = true;
    }

    // Material Interface (C++ lines 169-223) - all delegate to current material desc

    /// Set pass count
    /// C++: Set_Pass_Count (meshmdl.h line 173)
    pub fn set_pass_count(&mut self, passes: usize) {
        self.current_mat_desc_mut().set_pass_count(passes);
    }

    /// Get pass count
    /// C++: Get_Pass_Count (meshmdl.h line 174)
    pub fn get_pass_count(&self) -> usize {
        self.current_mat_desc().get_pass_count()
    }

    /// Get UV array for pass/stage
    /// C++: Get_UV_Array (meshmdl.h line 176)
    pub fn get_uv_array(&self, pass: usize, stage: usize) -> Option<&[Vec2]> {
        self.current_mat_desc().get_uv_array(pass, stage)
    }

    /// Get UV array count
    /// C++: Get_UV_Array_Count (meshmdl.h line 177)
    pub fn get_uv_array_count(&self) -> usize {
        self.current_mat_desc().get_uv_array_count()
    }

    /// Get UV array by index
    /// C++: Get_UV_Array_By_Index (meshmdl.h line 178)
    pub fn get_uv_array_by_index(&self, index: usize) -> Option<&[Vec2]> {
        self.current_mat_desc().get_uv_array_by_index(index, false)
    }

    /// Get diffuse color array
    /// C++: Get_DCG_Array (meshmdl.h line 180)
    pub fn get_dcg_array(&self, pass: usize) -> Option<&[u32]> {
        self.current_mat_desc().get_dcg_array(pass)
    }

    /// Get emissive color array
    /// C++: Get_DIG_Array (meshmdl.h line 181)
    pub fn get_dig_array(&self, pass: usize) -> Option<&[u32]> {
        self.current_mat_desc().get_dig_array(pass)
    }

    /// Get DCG source
    /// C++: Get_DCG_Source (meshmdl.h line 182)
    pub fn get_dcg_source(&self, pass: usize) -> ColorSourceType {
        self.current_mat_desc().get_dcg_source(pass)
    }

    /// Get DIG source
    /// C++: Get_DIG_Source (meshmdl.h line 183)
    pub fn get_dig_source(&self, pass: usize) -> ColorSourceType {
        self.current_mat_desc().get_dig_source(pass)
    }

    /// Get color array by index
    /// C++: Get_Color_Array (meshmdl.h line 185)
    pub fn get_color_array(&self, array_index: usize) -> Option<&[u32]> {
        self.current_mat_desc().get_color_array(array_index)
    }

    /// Set single material for pass
    /// C++: Set_Single_Material (meshmdl.h line 187)
    pub fn set_single_material(&mut self, pass: usize, vmat: Option<Arc<VertexMaterialClass>>) {
        self.current_mat_desc_mut().set_single_material(pass, vmat);
    }

    /// Set single texture for pass/stage
    /// C++: Set_Single_Texture (meshmdl.h line 188)
    pub fn set_single_texture(
        &mut self,
        pass: usize,
        stage: usize,
        tex: Option<Arc<TextureClass>>,
    ) {
        self.current_mat_desc_mut()
            .set_single_texture(pass, stage, tex);
    }

    /// Set single shader for pass
    /// C++: Set_Single_Shader (meshmdl.h line 189)
    pub fn set_single_shader(&mut self, pass: usize, shader: ShaderClass) {
        self.current_mat_desc_mut().set_single_shader(pass, shader);
    }

    /// Get single material (adds reference)
    /// C++: Get_Single_Material (meshmdl.h line 192)
    pub fn get_single_material(&self, pass: usize) -> Option<Arc<VertexMaterialClass>> {
        self.current_mat_desc().get_single_material(pass)
    }

    /// Get single texture (adds reference)
    /// C++: Get_Single_Texture (meshmdl.h line 193)
    pub fn get_single_texture(&self, pass: usize, stage: usize) -> Option<Arc<TextureClass>> {
        self.current_mat_desc().get_single_texture(pass, stage)
    }

    /// Get single shader
    /// C++: Get_Single_Shader (meshmdl.h line 194)
    pub fn get_single_shader(&self, pass: usize) -> ShaderClass {
        self.current_mat_desc().get_single_shader(pass)
    }

    /// Peek single material (no reference)
    /// C++: Peek_Single_Material (meshmdl.h line 198)
    pub fn peek_single_material(&self, pass: usize) -> Option<&Arc<VertexMaterialClass>> {
        self.current_mat_desc().peek_single_material(pass)
    }

    /// Peek single texture (no reference)
    /// C++: Peek_Single_Texture (meshmdl.h line 199)
    pub fn peek_single_texture(&self, pass: usize, stage: usize) -> Option<&Arc<TextureClass>> {
        self.current_mat_desc().peek_single_texture(pass, stage)
    }

    /// Set material at vertex
    /// C++: Set_Material (meshmdl.h line 201)
    pub fn set_material(
        &mut self,
        pass: usize,
        vidx: usize,
        vmat: Option<Arc<VertexMaterialClass>>,
    ) {
        self.current_mat_desc_mut().set_material(pass, vidx, vmat);
    }

    /// Set shader at polygon
    /// C++: Set_Shader (meshmdl.h line 202)
    pub fn set_shader(&mut self, pass: usize, pidx: usize, shader: ShaderClass) {
        self.current_mat_desc_mut().set_shader(pass, pidx, shader);
    }

    /// Set texture at polygon
    /// C++: Set_Texture (meshmdl.h line 203)
    pub fn set_texture(
        &mut self,
        pass: usize,
        stage: usize,
        pidx: usize,
        tex: Option<Arc<TextureClass>>,
    ) {
        self.current_mat_desc_mut()
            .set_texture(pass, stage, pidx, tex);
    }

    /// Check if has material array
    /// C++: Has_Material_Array (meshmdl.h line 206)
    pub fn has_material_array(&self, pass: usize) -> bool {
        self.current_mat_desc().has_material_array(pass)
    }

    /// Check if has shader array
    /// C++: Has_Shader_Array (meshmdl.h line 207)
    pub fn has_shader_array(&self, pass: usize) -> bool {
        self.current_mat_desc().has_shader_array(pass)
    }

    /// Check if has texture array
    /// C++: Has_Texture_Array (meshmdl.h line 208)
    pub fn has_texture_array(&self, pass: usize, stage: usize) -> bool {
        self.current_mat_desc().has_texture_array(pass, stage)
    }

    /// Get material at vertex
    /// C++: Get_Material (meshmdl.h line 211)
    pub fn get_material(&self, pass: usize, vidx: usize) -> Option<Arc<VertexMaterialClass>> {
        self.current_mat_desc().get_material(pass, vidx)
    }

    /// Get texture at polygon
    /// C++: Get_Texture (meshmdl.h line 212)
    pub fn get_texture(&self, pass: usize, stage: usize, pidx: usize) -> Option<Arc<TextureClass>> {
        self.current_mat_desc().get_texture(pass, stage, pidx)
    }

    /// Get shader at polygon
    /// C++: Get_Shader (meshmdl.h line 213)
    pub fn get_shader(&self, pass: usize, pidx: usize) -> ShaderClass {
        self.current_mat_desc().get_shader(pass, pidx)
    }

    /// Peek material at vertex
    /// C++: Peek_Material (meshmdl.h line 216)
    pub fn peek_material(&self, pass: usize, vidx: usize) -> Option<&Arc<VertexMaterialClass>> {
        self.current_mat_desc().peek_material(pass, vidx)
    }

    /// Peek texture at polygon
    /// C++: Peek_Texture (meshmdl.h line 217)
    pub fn peek_texture(
        &self,
        pass: usize,
        stage: usize,
        pidx: usize,
    ) -> Option<&Arc<TextureClass>> {
        self.current_mat_desc().peek_texture(pass, stage, pidx)
    }

    /// Replace all instances of a texture with another
    /// C++: Replace_Texture (meshmdl.h line 219, meshmdl.cpp lines 207-233)
    pub fn replace_texture(&mut self, old_texture: &TextureClass, new_texture: Arc<TextureClass>) {
        let old_ptr = old_texture as *const TextureClass;

        for stage in 0..MAX_TEX_STAGES {
            for pass in 0..self.get_pass_count() {
                if self.has_texture_array(pass, stage) {
                    // Per-polygon textures
                    for pidx in 0..self.geometry.get_polygon_count() {
                        if let Some(tex) = self.peek_texture(pass, stage, pidx) {
                            if Arc::as_ptr(tex) == old_ptr {
                                self.set_texture(pass, stage, pidx, Some(new_texture.clone()));
                            }
                        }
                    }
                } else {
                    // Single texture
                    if let Some(tex) = self.peek_single_texture(pass, stage) {
                        if Arc::as_ptr(tex) == old_ptr {
                            self.set_single_texture(pass, stage, Some(new_texture.clone()));
                        }
                    }
                }
            }
        }
    }

    /// Replace all instances of a vertex material with another
    /// C++: Replace_VertexMaterial (meshmdl.h line 220, meshmdl.cpp lines 235-260)
    pub fn replace_vertex_material(
        &mut self,
        old_vmat: &VertexMaterialClass,
        new_vmat: Arc<VertexMaterialClass>,
    ) {
        let old_ptr = old_vmat as *const VertexMaterialClass;

        for pass in 0..self.get_pass_count() {
            if self.has_material_array(pass) {
                // Per-vertex materials
                for vidx in 0..self.geometry.get_vertex_count() {
                    if let Some(mat) = self.peek_material(pass, vidx) {
                        if Arc::as_ptr(mat) == old_ptr {
                            self.set_material(pass, vidx, Some(new_vmat.clone()));
                        }
                    }
                }
            } else {
                // Single material
                if let Some(mat) = self.peek_single_material(pass) {
                    if Arc::as_ptr(mat) == old_ptr {
                        self.set_single_material(pass, Some(new_vmat.clone()));
                    }
                }
            }
        }
    }

    // Modification Interface (C++ lines 222-230)

    /// Make geometry unique (copy-on-write)
    /// C++: Make_Geometry_Unique (meshmdl.h line 227, meshmdl.cpp lines 291-308)
    pub fn make_geometry_unique(&mut self) {
        self.geometry.make_vertex_array_unique();
        self.geometry.make_vertex_normal_array_unique();
        // Plane equations would also be made unique here if needed
    }

    /// Make UV array unique
    /// C++: Make_UV_Array_Unique (meshmdl.h line 228, meshmdl.cpp lines 310-313)
    pub fn make_uv_array_unique(&mut self, pass: usize, stage: usize) {
        self.current_mat_desc_mut()
            .make_uv_array_unique(pass, stage);
    }

    /// Make color array unique
    /// C++: Make_Color_Array_Unique (meshmdl.h line 229, meshmdl.cpp lines 315-318)
    pub fn make_color_array_unique(&mut self, array_index: usize) {
        self.current_mat_desc_mut()
            .make_color_array_unique(array_index);
    }

    // Alternate Material Description Interface (C++ lines 240-246)

    /// Enable or disable alternate material description
    /// C++: Enable_Alternate_Material_Description (meshmdl.h line 244, meshmdl.cpp lines 320-349)
    pub fn enable_alternate_material_description(&mut self, enable: bool) {
        if enable && self.alternate_mat_desc.is_some() {
            self.use_alternate = true;
        } else {
            self.use_alternate = false;
        }
    }

    /// Check if alternate material description is enabled
    /// C++: Is_Alternate_Material_Description_Enabled (meshmdl.h line 245, meshmdl.cpp lines 351-354)
    pub fn is_alternate_material_description_enabled(&self) -> bool {
        self.use_alternate && self.alternate_mat_desc.is_some()
    }

    /// Check if vertex normals are needed
    /// C++: Needs_Vertex_Normals (meshmdl.h line 254, meshmdl.cpp lines 361-367)
    pub fn needs_vertex_normals(&self) -> bool {
        let prelit = self.geometry.get_flag(GeometryFlags::PRELIT_MASK);
        if !prelit {
            return true;
        }
        // Would check if mappers need normals here
        false
    }

    /// Get gap filler
    /// C++: Get_Gap_Filler (meshmdl.h line 257)
    pub fn get_gap_filler(&self) -> Option<&GapFiller> {
        self.gap_filler.as_ref()
    }

    /// Check if has polygon renderers
    /// C++: Has_Polygon_Renderers (meshmdl.h line 259)
    pub fn has_polygon_renderers(&self) -> bool {
        self.has_polygon_renderers
    }

    /// Get number of vertices
    pub fn get_num_vertices(&self) -> usize {
        self.geometry.get_vertex_count()
    }

    /// Get number of triangles
    pub fn get_num_triangles(&self) -> usize {
        self.geometry.get_polygon_count()
    }

    /// Get number of indices
    pub fn get_num_indices(&self) -> usize {
        self.geometry.get_polygon_count() * 3
    }

    // Private helper methods

    /// Get current material description (immutable)
    fn current_mat_desc(&self) -> &MeshMatDesc {
        if self.use_alternate {
            self.alternate_mat_desc
                .as_ref()
                .unwrap_or(&self.def_mat_desc)
        } else {
            &self.def_mat_desc
        }
    }

    /// Get current material description (mutable)
    fn current_mat_desc_mut(&mut self) -> &mut MeshMatDesc {
        if self.use_alternate && self.alternate_mat_desc.is_some() {
            self.alternate_mat_desc.as_mut().unwrap()
        } else {
            &mut self.def_mat_desc
        }
    }

    /// Install alternate material description
    pub fn install_alternate_material_description(&mut self, mat_desc: MeshMatDesc) {
        self.alternate_mat_desc = Some(mat_desc);
    }
}

impl Default for MeshModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_model_creation() {
        let model = MeshModel::new();
        assert_eq!(model.get_pass_count(), 1);
        assert_eq!(model.get_num_vertices(), 0);
        assert_eq!(model.get_num_triangles(), 0);
    }

    #[test]
    fn test_mesh_model_reset() {
        let mut model = MeshModel::new();
        model.reset(10, 20, 2);

        assert_eq!(model.geometry.get_polygon_count(), 10);
        assert_eq!(model.geometry.get_vertex_count(), 20);
        assert_eq!(model.get_pass_count(), 2);
    }

    #[test]
    fn test_material_delegation() {
        let mut model = MeshModel::new();
        model.reset(10, 20, 2);

        // Test that methods delegate to material description
        model.set_pass_count(3);
        assert_eq!(model.get_pass_count(), 3);
    }

    #[test]
    fn test_alternate_material_description() {
        let mut model = MeshModel::new();
        model.reset(10, 20, 1);

        // Install alternate
        let alt_desc = MeshMatDesc::new();
        model.install_alternate_material_description(alt_desc);

        // Not enabled yet
        assert!(!model.is_alternate_material_description_enabled());

        // Enable it
        model.enable_alternate_material_description(true);
        assert!(model.is_alternate_material_description_enabled());

        // Disable it
        model.enable_alternate_material_description(false);
        assert!(!model.is_alternate_material_description_enabled());
    }

    #[test]
    fn test_gap_filler_creation() {
        let has_texture = [[false; MAX_TEX_STAGES]; MAX_PASSES];
        let has_material = [false; MAX_PASSES];
        let has_shader = [false; MAX_PASSES];

        let gap_filler = GapFiller::new(10, 1, &has_texture, &has_material, &has_shader);

        assert_eq!(gap_filler.get_polygon_count(), 0);
        assert_eq!(gap_filler.array_size, 60); // 10 * 6
    }

    #[test]
    fn test_gap_filler_shrink() {
        let has_texture = [[false; MAX_TEX_STAGES]; MAX_PASSES];
        let has_material = [false; MAX_PASSES];
        let has_shader = [false; MAX_PASSES];

        let mut gap_filler = GapFiller::new(10, 1, &has_texture, &has_material, &has_shader);

        let initial_size = gap_filler.array_size;
        assert_eq!(initial_size, 60);

        // No polygons added, so shrink should reduce to 0
        gap_filler.shrink_buffers();
        assert_eq!(gap_filler.array_size, 0);
        assert_eq!(gap_filler.get_polygon_array().len(), 0);
    }

    #[test]
    fn test_material_info() {
        let mut info = MaterialInfo::new();
        assert_eq!(info.get_materials().len(), 0);

        info.reset();
        assert_eq!(info.get_materials().len(), 0);
    }

    #[test]
    fn test_registration() {
        let mut model = MeshModel::new();
        assert!(!model.has_polygon_renderers());

        model.register_for_rendering();
        assert!(model.has_polygon_renderers());
        assert!(model.has_been_in_use);
    }
}
