// GNU General Public License v3.0 - See LICENSE file for details
// Command & Conquer Generals Zero Hour(tm)
// Copyright 2025 Electronic Arts Inc.
//
// Complete port of MeshMatDescClass from C++ (meshmatdesc.h/cpp)
// Original: meshmatdesc.h lines 38-494, meshmatdesc.cpp lines 38-987

use super::mesh_geometry::ShareBuffer;
use super::shader_system::shader::ShaderClass;
use crate::material_system::VertexMaterialClass;
use crate::texture_system::TextureClass;
use glam::Vec2;
use std::sync::Arc;

/// Maximum number of rendering passes
/// C++: MAX_PASSES (meshmatdesc.h line 68)
pub const MAX_PASSES: usize = 4;

/// Maximum number of texture stages per pass
/// C++: MAX_TEX_STAGES (meshmatdesc.h line 69)
pub const MAX_TEX_STAGES: usize = 2;

/// Maximum number of color arrays
/// C++: MAX_COLOR_ARRAYS (meshmatdesc.h line 70)
pub const MAX_COLOR_ARRAYS: usize = 2;

/// Maximum number of UV arrays
/// C++: MAX_UV_ARRAYS (meshmatdesc.h line 71)
pub const MAX_UV_ARRAYS: usize = MAX_PASSES * MAX_TEX_STAGES;

/// Color source type for vertex materials
/// C++: VertexMaterialClass::ColorSourceType (vertmaterial.h)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSourceType {
    Material,
    Color1,
    Color2,
}

/// Material buffer - ref-counted array of VertexMaterialClass pointers
/// C++: MatBufferClass (meshmatdesc.h lines 232-247, meshmatdesc.cpp lines 55-89)
#[derive(Debug, Clone)]
pub struct MatBuffer {
    materials: Arc<Vec<Option<Arc<VertexMaterialClass>>>>,
}

impl MatBuffer {
    /// Create new material buffer with given count
    /// C++: MatBufferClass::MatBufferClass constructor (meshmatdesc.h line 236)
    pub fn new(count: usize) -> Self {
        Self {
            materials: Arc::new(vec![None; count]),
        }
    }

    /// Create from existing array
    pub fn from_vec(materials: Vec<Option<Arc<VertexMaterialClass>>>) -> Self {
        Self {
            materials: Arc::new(materials),
        }
    }

    /// Set element at index
    /// C++: Set_Element (meshmatdesc.h line 240, meshmatdesc.cpp line 73)
    pub fn set_element(&mut self, index: usize, mat: Option<Arc<VertexMaterialClass>>) {
        if index < self.materials.len() {
            Arc::make_mut(&mut self.materials)[index] = mat;
        }
    }

    /// Get element (adds reference in C++)
    /// C++: Get_Element (meshmatdesc.h line 241, meshmatdesc.cpp line 78)
    pub fn get_element(&self, index: usize) -> Option<Arc<VertexMaterialClass>> {
        self.materials.get(index).and_then(|m| m.clone())
    }

    /// Peek element (no reference added)
    /// C++: Peek_Element (meshmatdesc.h line 242, meshmatdesc.cpp line 86)
    pub fn peek_element(&self, index: usize) -> Option<&Arc<VertexMaterialClass>> {
        self.materials.get(index).and_then(|m| m.as_ref())
    }

    /// Get count
    pub fn count(&self) -> usize {
        self.materials.len()
    }

    /// Get reference count
    pub fn num_refs(&self) -> usize {
        Arc::strong_count(&self.materials)
    }

    /// Make unique for modification
    pub fn make_unique(&mut self) -> &mut Vec<Option<Arc<VertexMaterialClass>>> {
        Arc::make_mut(&mut self.materials)
    }
}

/// Texture buffer - ref-counted array of TextureClass pointers
/// C++: TexBufferClass (meshmatdesc.h lines 254-269, meshmatdesc.cpp lines 99-133)
#[derive(Debug, Clone)]
pub struct TexBuffer {
    textures: Arc<Vec<Option<Arc<TextureClass>>>>,
}

impl TexBuffer {
    /// Create new texture buffer with given count
    /// C++: TexBufferClass::TexBufferClass constructor (meshmatdesc.h line 258)
    pub fn new(count: usize) -> Self {
        Self {
            textures: Arc::new(vec![None; count]),
        }
    }

    /// Create from existing array
    pub fn from_vec(textures: Vec<Option<Arc<TextureClass>>>) -> Self {
        Self {
            textures: Arc::new(textures),
        }
    }

    /// Set element at index
    /// C++: Set_Element (meshmatdesc.h line 262, meshmatdesc.cpp line 117)
    pub fn set_element(&mut self, index: usize, tex: Option<Arc<TextureClass>>) {
        if index < self.textures.len() {
            Arc::make_mut(&mut self.textures)[index] = tex;
        }
    }

    /// Get element (adds reference in C++)
    /// C++: Get_Element (meshmatdesc.h line 263, meshmatdesc.cpp line 122)
    pub fn get_element(&self, index: usize) -> Option<Arc<TextureClass>> {
        self.textures.get(index).and_then(|t| t.clone())
    }

    /// Peek element (no reference added)
    /// C++: Peek_Element (meshmatdesc.h line 264, meshmatdesc.cpp line 130)
    pub fn peek_element(&self, index: usize) -> Option<&Arc<TextureClass>> {
        self.textures.get(index).and_then(|t| t.as_ref())
    }

    /// Get count
    pub fn count(&self) -> usize {
        self.textures.len()
    }

    /// Get reference count
    pub fn num_refs(&self) -> usize {
        Arc::strong_count(&self.textures)
    }

    /// Make unique for modification
    pub fn make_unique(&mut self) -> &mut Vec<Option<Arc<TextureClass>>> {
        Arc::make_mut(&mut self.textures)
    }
}

/// UV coordinate buffer with CRC for redundancy detection
/// C++: UVBufferClass (meshmatdesc.h lines 276-294, meshmatdesc.cpp lines 143-165)
#[derive(Debug, Clone)]
pub struct UVBuffer {
    uvs: ShareBuffer<Vec2>,
    crc: u32,
}

impl UVBuffer {
    /// Create new UV buffer
    /// C++: UVBufferClass::UVBufferClass constructor (meshmatdesc.h line 280)
    pub fn new(uvs: Vec<Vec2>) -> Self {
        let mut buffer = Self {
            uvs: ShareBuffer::new(uvs),
            crc: 0xFFFFFFFF,
        };
        buffer.update_crc();
        buffer
    }

    /// Get UV array
    pub fn get_array(&self) -> &[Vec2] {
        self.uvs.get_array()
    }

    /// Get count
    pub fn count(&self) -> usize {
        self.uvs.count()
    }

    /// Get CRC
    /// C++: Get_CRC (meshmatdesc.h line 287)
    pub fn get_crc(&self) -> u32 {
        self.crc
    }

    /// Update CRC from data
    /// C++: Update_CRC (meshmatdesc.h line 286, meshmatdesc.cpp line 162)
    pub fn update_crc(&mut self) {
        self.crc = compute_crc(bytemuck::cast_slice(self.uvs.get_array()));
    }

    /// Check equality by CRC
    /// C++: operator== (meshmatdesc.h line 283, meshmatdesc.cpp line 149)
    pub fn is_equal_to(&self, other: &UVBuffer) -> bool {
        self.crc == other.crc
    }

    /// Get reference count
    pub fn num_refs(&self) -> usize {
        self.uvs.num_refs()
    }

    /// Make unique for modification
    pub fn make_unique(&mut self) -> &mut Vec<Vec2> {
        self.uvs.make_unique();
        self.update_crc();
        self.uvs.make_unique()
    }
}

/// Null shader constant
/// C++: MeshMatDescClass::NullShader (meshmatdesc.h line 190, meshmatdesc.cpp line 176)
pub fn null_shader() -> ShaderClass {
    ShaderClass::default()
}

/// Material description for a mesh - encapsulates all material/texture data
/// C++: MeshMatDescClass (meshmatdesc.h lines 61-223, meshmatdesc.cpp lines 178-987)
///
/// This class manages:
/// - Multiple rendering passes (up to 4)
/// - Multiple texture stages per pass (up to 2)
/// - UV coordinate arrays with redundancy detection
/// - Vertex materials (per-vertex or single)
/// - Textures (per-polygon or single)
/// - Shaders (per-polygon or single)
/// - Vertex color arrays
#[derive(Debug, Clone)]
pub struct MeshMatDesc {
    // Counts - C++: lines 198-200
    pass_count: usize,
    vertex_count: usize,
    poly_count: usize,

    // UV coordinates - C++: lines 203-204
    // Multiple UV arrays can be shared between passes/stages via UVSource indices
    uv: [Option<Arc<UVBuffer>>; MAX_UV_ARRAYS],
    uv_source: [[i32; MAX_TEX_STAGES]; MAX_PASSES], // -1 = no UV

    // Vertex color arrays - C++: lines 208-210
    color_array: [Option<ShareBuffer<u32>>; MAX_COLOR_ARRAYS],
    dcg_source: [ColorSourceType; MAX_PASSES], // Diffuse color source
    dig_source: [ColorSourceType; MAX_PASSES], // Emissive color source

    // Default materials/textures/shaders - C++: lines 212-215
    texture: [[Option<Arc<TextureClass>>; MAX_TEX_STAGES]; MAX_PASSES],
    shader: [ShaderClass; MAX_PASSES],
    material: [Option<Arc<VertexMaterialClass>>; MAX_PASSES],

    // Per-polygon/per-vertex arrays - C++: lines 217-220
    texture_array: [[Option<Arc<TexBuffer>>; MAX_TEX_STAGES]; MAX_PASSES],
    material_array: [Option<Arc<MatBuffer>>; MAX_PASSES],
    shader_array: [Option<ShareBuffer<ShaderClass>>; MAX_PASSES],
}

impl MeshMatDesc {
    /// Create new material description
    /// C++: MeshMatDescClass::MeshMatDescClass constructor (meshmatdesc.cpp lines 178-204)
    pub fn new() -> Self {
        Self {
            pass_count: 1,
            vertex_count: 0,
            poly_count: 0,
            uv: Default::default(),
            uv_source: [[-1; MAX_TEX_STAGES]; MAX_PASSES],
            color_array: Default::default(),
            dcg_source: [ColorSourceType::Material; MAX_PASSES],
            dig_source: [ColorSourceType::Material; MAX_PASSES],
            texture: Default::default(),
            shader: [ShaderClass::default(); MAX_PASSES],
            material: Default::default(),
            texture_array: Default::default(),
            material_array: Default::default(),
            shader_array: Default::default(),
        }
    }

    /// Reset to initial state with new counts
    /// C++: Reset (meshmatdesc.h line 77, meshmatdesc.cpp lines 308-338)
    pub fn reset(&mut self, poly_count: usize, vertex_count: usize, pass_count: usize) {
        self.poly_count = poly_count;
        self.vertex_count = vertex_count;
        self.pass_count = pass_count;

        // Clear all arrays
        self.color_array = Default::default();
        self.uv = Default::default();
        self.uv_source = [[-1; MAX_TEX_STAGES]; MAX_PASSES];

        for pass in 0..MAX_PASSES {
            self.texture[pass] = Default::default();
            self.texture_array[pass] = Default::default();
            self.dcg_source[pass] = ColorSourceType::Material;
            self.dig_source[pass] = ColorSourceType::Material;
            self.shader[pass] = ShaderClass::default();
            self.shader_array[pass] = None;
            self.material[pass] = None;
            self.material_array[pass] = None;
        }
    }

    /// Check if material description is empty
    /// C++: Is_Empty (meshmatdesc.h line 85, meshmatdesc.cpp lines 439-462)
    pub fn is_empty(&self) -> bool {
        // Check color arrays
        for ca in &self.color_array {
            if ca.is_some() {
                return false;
            }
        }

        // Check UV arrays
        for uv in &self.uv {
            if uv.is_some() {
                return false;
            }
        }

        // Check materials, textures, arrays
        for pass in 0..MAX_PASSES {
            for stage in 0..MAX_TEX_STAGES {
                if self.texture[pass][stage].is_some() || self.texture_array[pass][stage].is_some()
                {
                    return false;
                }
            }
            if self.material[pass].is_some() || self.material_array[pass].is_some() {
                return false;
            }
        }

        true
    }

    // Getters/Setters for counts

    /// Set pass count
    /// C++: Set_Pass_Count (meshmatdesc.h line 90)
    pub fn set_pass_count(&mut self, passes: usize) {
        self.pass_count = passes.min(MAX_PASSES);
    }

    /// Get pass count
    /// C++: Get_Pass_Count (meshmatdesc.h line 91)
    pub fn get_pass_count(&self) -> usize {
        self.pass_count
    }

    /// Set vertex count
    /// C++: Set_Vertex_Count (meshmatdesc.h line 92)
    pub fn set_vertex_count(&mut self, count: usize) {
        self.vertex_count = count;
    }

    /// Get vertex count
    /// C++: Get_Vertex_Count (meshmatdesc.h line 93)
    pub fn get_vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// Set polygon count
    /// C++: Set_Polygon_Count (meshmatdesc.h line 94)
    pub fn set_polygon_count(&mut self, count: usize) {
        self.poly_count = count;
    }

    /// Get polygon count
    /// C++: Get_Polygon_Count (meshmatdesc.h line 95)
    pub fn get_polygon_count(&self) -> usize {
        self.poly_count
    }

    // UV Array Management

    /// Get UV array for pass/stage
    /// C++: Get_UV_Array (meshmatdesc.h lines 100, 304-313)
    pub fn get_uv_array(&self, pass: usize, stage: usize) -> Option<&[Vec2]> {
        if pass >= MAX_PASSES || stage >= MAX_TEX_STAGES {
            return None;
        }
        let source = self.uv_source[pass][stage];
        if source == -1 {
            return None;
        }
        self.uv
            .get(source as usize)
            .and_then(|uv_opt| uv_opt.as_ref())
            .map(|uv| uv.get_array())
    }

    /// Install UV array for pass/stage with deduplication
    /// C++: Install_UV_Array (meshmatdesc.h line 101, meshmatdesc.cpp lines 598-640)
    pub fn install_uv_array(&mut self, pass: usize, stage: usize, uvs: Vec<Vec2>) {
        if pass >= MAX_PASSES || stage >= MAX_TEX_STAGES {
            return;
        }

        let new_buffer = UVBuffer::new(uvs);
        let new_crc = new_buffer.get_crc();

        // Check for existing matching UV array by CRC
        for (i, uv_opt) in self.uv.iter().enumerate() {
            if let Some(existing) = uv_opt {
                if existing.get_crc() == new_crc {
                    // Found matching UV array, reuse it
                    self.uv_source[pass][stage] = i as i32;
                    return;
                }
            }
        }

        // No match found, install as new array
        for (i, uv_opt) in self.uv.iter_mut().enumerate() {
            if uv_opt.is_none() {
                *uv_opt = Some(Arc::new(new_buffer));
                self.uv_source[pass][stage] = i as i32;
                return;
            }
        }
    }

    /// Set UV source index
    /// C++: Set_UV_Source (meshmatdesc.h lines 102, 315-322)
    pub fn set_uv_source(&mut self, pass: usize, stage: usize, source_index: i32) {
        if pass < MAX_PASSES && stage < MAX_TEX_STAGES {
            self.uv_source[pass][stage] = source_index;
        }
    }

    /// Get UV source index
    /// C++: Get_UV_Source (meshmatdesc.h lines 103, 324-331)
    pub fn get_uv_source(&self, pass: usize, stage: usize) -> i32 {
        if pass < MAX_PASSES && stage < MAX_TEX_STAGES {
            self.uv_source[pass][stage]
        } else {
            -1
        }
    }

    /// Get total number of UV arrays
    /// C++: Get_UV_Array_Count (meshmatdesc.h lines 105, 333-340)
    pub fn get_uv_array_count(&self) -> usize {
        self.uv.iter().take_while(|uv| uv.is_some()).count()
    }

    /// Get UV array by index
    /// C++: Get_UV_Array_By_Index (meshmatdesc.h lines 106, 342-353)
    pub fn get_uv_array_by_index(&self, index: usize, _create: bool) -> Option<&[Vec2]> {
        if index >= MAX_UV_ARRAYS {
            return None;
        }
        // Note: C++ version creates if needed, but we return None for simplicity
        // Create functionality would need mutable access
        self.uv
            .get(index)
            .and_then(|uv_opt| uv_opt.as_ref())
            .map(|uv| uv.get_array())
    }

    /// Make UV array unique (copy-on-write)
    /// C++: Make_UV_Array_Unique (meshmatdesc.h line 175, meshmatdesc.cpp lines 579-587)
    pub fn make_uv_array_unique(&mut self, pass: usize, stage: usize) {
        if pass >= MAX_PASSES || stage >= MAX_TEX_STAGES {
            return;
        }
        let source = self.uv_source[pass][stage];
        if source == -1 {
            return;
        }
        let index = source as usize;
        if index >= self.uv.len() {
            return;
        }

        if let Some(uv_arc) = &self.uv[index] {
            if Arc::strong_count(uv_arc) > 1 {
                // Make a copy
                let data = uv_arc.get_array().to_vec();
                self.uv[index] = Some(Arc::new(UVBuffer::new(data)));
            }
        }
    }

    // Color Array Management

    /// Get diffuse color (DCG) array
    /// C++: Get_DCG_Array (meshmatdesc.h lines 108, 355-382)
    pub fn get_dcg_array(&self, pass: usize) -> Option<&[u32]> {
        if pass >= MAX_PASSES {
            return None;
        }
        match self.dcg_source[pass] {
            ColorSourceType::Material => None,
            ColorSourceType::Color1 => self.color_array[0].as_ref().map(|ca| ca.get_array()),
            ColorSourceType::Color2 => self.color_array[1].as_ref().map(|ca| ca.get_array()),
        }
    }

    /// Get emissive color (DIG) array
    /// C++: Get_DIG_Array (meshmatdesc.h lines 109, 384-411)
    pub fn get_dig_array(&self, pass: usize) -> Option<&[u32]> {
        if pass >= MAX_PASSES {
            return None;
        }
        match self.dig_source[pass] {
            ColorSourceType::Material => None,
            ColorSourceType::Color1 => self.color_array[0].as_ref().map(|ca| ca.get_array()),
            ColorSourceType::Color2 => self.color_array[1].as_ref().map(|ca| ca.get_array()),
        }
    }

    /// Set DCG source
    /// C++: Set_DCG_Source (meshmatdesc.h lines 110, 413-416)
    pub fn set_dcg_source(&mut self, pass: usize, source: ColorSourceType) {
        if pass < MAX_PASSES {
            self.dcg_source[pass] = source;
        }
    }

    /// Set DIG source
    /// C++: Set_DIG_Source (meshmatdesc.h lines 111, 418-421)
    pub fn set_dig_source(&mut self, pass: usize, source: ColorSourceType) {
        if pass < MAX_PASSES {
            self.dig_source[pass] = source;
        }
    }

    /// Get DCG source
    /// C++: Get_DCG_Source (meshmatdesc.h lines 112, 423-426)
    pub fn get_dcg_source(&self, pass: usize) -> ColorSourceType {
        if pass < MAX_PASSES {
            self.dcg_source[pass]
        } else {
            ColorSourceType::Material
        }
    }

    /// Get DIG source
    /// C++: Get_DIG_Source (meshmatdesc.h lines 113, 428-431)
    pub fn get_dig_source(&self, pass: usize) -> ColorSourceType {
        if pass < MAX_PASSES {
            self.dig_source[pass]
        } else {
            ColorSourceType::Material
        }
    }

    /// Get color array by index
    /// C++: Get_Color_Array (meshmatdesc.h lines 114, 433-442)
    pub fn get_color_array(&self, index: usize) -> Option<&[u32]> {
        if index < MAX_COLOR_ARRAYS {
            self.color_array[index].as_ref().map(|ca| ca.get_array())
        } else {
            None
        }
    }

    /// Install color array
    pub fn install_color_array(&mut self, index: usize, colors: Vec<u32>) {
        if index < MAX_COLOR_ARRAYS {
            self.color_array[index] = Some(ShareBuffer::new(colors));
        }
    }

    /// Make color array unique
    /// C++: Make_Color_Array_Unique (meshmatdesc.h line 176, meshmatdesc.cpp lines 589-596)
    pub fn make_color_array_unique(&mut self, index: usize) {
        if index >= MAX_COLOR_ARRAYS {
            return;
        }
        if let Some(ca) = &mut self.color_array[index] {
            if ca.num_refs() > 1 {
                let data = ca.get_array().to_vec();
                self.color_array[index] = Some(ShareBuffer::new(data));
            }
        }
    }

    // Single Material/Texture/Shader Access

    /// Set single material for pass
    /// C++: Set_Single_Material (meshmatdesc.h lines 116, meshmatdesc.cpp lines 464-467)
    pub fn set_single_material(&mut self, pass: usize, vmat: Option<Arc<VertexMaterialClass>>) {
        if pass < MAX_PASSES {
            self.material[pass] = vmat;
        }
    }

    /// Set single texture for pass/stage
    /// C++: Set_Single_Texture (meshmatdesc.h lines 117, meshmatdesc.cpp lines 469-472)
    pub fn set_single_texture(
        &mut self,
        pass: usize,
        stage: usize,
        tex: Option<Arc<TextureClass>>,
    ) {
        if pass < MAX_PASSES && stage < MAX_TEX_STAGES {
            self.texture[pass][stage] = tex;
        }
    }

    /// Set single shader for pass
    /// C++: Set_Single_Shader (meshmatdesc.h lines 118, meshmatdesc.cpp lines 474-477)
    pub fn set_single_shader(&mut self, pass: usize, shader: ShaderClass) {
        if pass < MAX_PASSES {
            self.shader[pass] = shader;
        }
    }

    /// Get single material (adds reference)
    /// C++: Get_Single_Material (meshmatdesc.h lines 123, 444-450)
    pub fn get_single_material(&self, pass: usize) -> Option<Arc<VertexMaterialClass>> {
        if pass < MAX_PASSES {
            self.material[pass].clone()
        } else {
            None
        }
    }

    /// Get single texture (adds reference)
    /// C++: Get_Single_Texture (meshmatdesc.h lines 124, meshmatdesc.cpp lines 300-306)
    pub fn get_single_texture(&self, pass: usize, stage: usize) -> Option<Arc<TextureClass>> {
        if pass < MAX_PASSES && stage < MAX_TEX_STAGES {
            self.texture[pass][stage].clone()
        } else {
            None
        }
    }

    /// Get single shader
    /// C++: Get_Single_Shader (meshmatdesc.h lines 125, 462-465)
    pub fn get_single_shader(&self, pass: usize) -> ShaderClass {
        if pass < MAX_PASSES {
            self.shader[pass]
        } else {
            ShaderClass::default()
        }
    }

    /// Peek single material (no reference)
    /// C++: Peek_Single_Material (meshmatdesc.h lines 131, 452-455)
    pub fn peek_single_material(&self, pass: usize) -> Option<&Arc<VertexMaterialClass>> {
        if pass < MAX_PASSES {
            self.material[pass].as_ref()
        } else {
            None
        }
    }

    /// Peek single texture (no reference)
    /// C++: Peek_Single_Texture (meshmatdesc.h lines 132, 457-460)
    pub fn peek_single_texture(&self, pass: usize, stage: usize) -> Option<&Arc<TextureClass>> {
        if pass < MAX_PASSES && stage < MAX_TEX_STAGES {
            self.texture[pass][stage].as_ref()
        } else {
            None
        }
    }

    // Per-Polygon/Per-Vertex Arrays

    /// Set material at vertex index
    /// C++: Set_Material (meshmatdesc.h line 134, meshmatdesc.cpp lines 479-483)
    pub fn set_material(
        &mut self,
        pass: usize,
        vidx: usize,
        vmat: Option<Arc<VertexMaterialClass>>,
    ) {
        if pass >= MAX_PASSES {
            return;
        }
        let mat_array = self.get_or_create_material_array(pass);
        Arc::make_mut(mat_array).set_element(vidx, vmat);
    }

    /// Set shader at polygon index
    /// C++: Set_Shader (meshmatdesc.h line 135, meshmatdesc.cpp lines 485-489)
    pub fn set_shader(&mut self, pass: usize, pidx: usize, shader: ShaderClass) {
        if pass >= MAX_PASSES {
            return;
        }
        let shader_array = self.get_or_create_shader_array(pass);
        if pidx < shader_array.count() {
            Arc::make_mut(shader_array.data_mut())[pidx] = shader;
        }
    }

    /// Set texture at polygon index
    /// C++: Set_Texture (meshmatdesc.h line 136, meshmatdesc.cpp lines 491-495)
    pub fn set_texture(
        &mut self,
        pass: usize,
        stage: usize,
        pidx: usize,
        tex: Option<Arc<TextureClass>>,
    ) {
        if pass >= MAX_PASSES || stage >= MAX_TEX_STAGES {
            return;
        }
        let tex_array = self.get_or_create_texture_array(pass, stage);
        Arc::make_mut(tex_array).set_element(pidx, tex);
    }

    /// Check if has material array
    /// C++: Has_Material_Array (meshmatdesc.h lines 141, 467-470)
    pub fn has_material_array(&self, pass: usize) -> bool {
        pass < MAX_PASSES && self.material_array[pass].is_some()
    }

    /// Check if has shader array
    /// C++: Has_Shader_Array (meshmatdesc.h lines 142, 472-475)
    pub fn has_shader_array(&self, pass: usize) -> bool {
        pass < MAX_PASSES && self.shader_array[pass].is_some()
    }

    /// Check if has texture array
    /// C++: Has_Texture_Array (meshmatdesc.h lines 143, 477-480)
    pub fn has_texture_array(&self, pass: usize, stage: usize) -> bool {
        pass < MAX_PASSES && stage < MAX_TEX_STAGES && self.texture_array[pass][stage].is_some()
    }

    /// Get material at vertex index
    /// C++: Get_Material (meshmatdesc.h line 158, meshmatdesc.cpp lines 497-510)
    pub fn get_material(&self, pass: usize, vidx: usize) -> Option<Arc<VertexMaterialClass>> {
        if pass >= MAX_PASSES {
            return None;
        }
        if let Some(mat_array) = &self.material_array[pass] {
            mat_array.get_element(vidx)
        } else {
            self.material[pass].clone()
        }
    }

    /// Get shader at polygon index
    /// C++: Get_Shader (meshmatdesc.h line 160, meshmatdesc.cpp lines 512-518)
    pub fn get_shader(&self, pass: usize, pidx: usize) -> ShaderClass {
        if pass >= MAX_PASSES {
            return ShaderClass::default();
        }
        if let Some(shader_array) = &self.shader_array[pass] {
            shader_array
                .get_array()
                .get(pidx)
                .cloned()
                .unwrap_or_default()
        } else {
            self.shader[pass]
        }
    }

    /// Get texture at polygon index
    /// C++: Get_Texture (meshmatdesc.h line 159, meshmatdesc.cpp lines 520-533)
    pub fn get_texture(&self, pass: usize, stage: usize, pidx: usize) -> Option<Arc<TextureClass>> {
        if pass >= MAX_PASSES || stage >= MAX_TEX_STAGES {
            return None;
        }
        if let Some(tex_array) = &self.texture_array[pass][stage] {
            tex_array.get_element(pidx)
        } else {
            self.texture[pass][stage].clone()
        }
    }

    /// Peek material at vertex index
    /// C++: Peek_Material (meshmatdesc.h line 165, meshmatdesc.cpp lines 535-541)
    pub fn peek_material(&self, pass: usize, vidx: usize) -> Option<&Arc<VertexMaterialClass>> {
        if pass >= MAX_PASSES {
            return None;
        }
        if let Some(mat_array) = &self.material_array[pass] {
            mat_array.peek_element(vidx)
        } else {
            self.material[pass].as_ref()
        }
    }

    /// Peek texture at polygon index
    /// C++: Peek_Texture (meshmatdesc.h line 166, meshmatdesc.cpp lines 543-549)
    pub fn peek_texture(
        &self,
        pass: usize,
        stage: usize,
        pidx: usize,
    ) -> Option<&Arc<TextureClass>> {
        if pass >= MAX_PASSES || stage >= MAX_TEX_STAGES {
            return None;
        }
        if let Some(tex_array) = &self.texture_array[pass][stage] {
            tex_array.peek_element(pidx)
        } else {
            self.texture[pass][stage].as_ref()
        }
    }

    /// Get or create material array
    /// C++: Get_Material_Array (meshmatdesc.h lines 172, meshmatdesc.cpp lines 559-565)
    pub fn get_or_create_material_array(&mut self, pass: usize) -> &mut Arc<MatBuffer> {
        if pass >= MAX_PASSES {
            panic!("Pass index out of range");
        }
        if self.material_array[pass].is_none() {
            self.material_array[pass] = Some(Arc::new(MatBuffer::new(self.vertex_count)));
        }
        self.material_array[pass].as_mut().unwrap()
    }

    /// Get or create texture array
    /// C++: Get_Texture_Array (meshmatdesc.h lines 171, meshmatdesc.cpp lines 551-557)
    pub fn get_or_create_texture_array(
        &mut self,
        pass: usize,
        stage: usize,
    ) -> &mut Arc<TexBuffer> {
        if pass >= MAX_PASSES || stage >= MAX_TEX_STAGES {
            panic!("Pass/stage index out of range");
        }
        if self.texture_array[pass][stage].is_none() {
            self.texture_array[pass][stage] = Some(Arc::new(TexBuffer::new(self.poly_count)));
        }
        self.texture_array[pass][stage].as_mut().unwrap()
    }

    /// Get or create shader array
    /// C++: Get_Shader_Array (meshmatdesc.h lines 173, meshmatdesc.cpp lines 567-577)
    pub fn get_or_create_shader_array(&mut self, pass: usize) -> &mut ShareBuffer<ShaderClass> {
        if pass >= MAX_PASSES {
            panic!("Pass index out of range");
        }
        if self.shader_array[pass].is_none() {
            self.shader_array[pass] = Some(ShareBuffer::new(vec![
                ShaderClass::default();
                self.poly_count
            ]));
        }
        self.shader_array[pass].as_mut().unwrap()
    }
}

impl Default for MeshMatDesc {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute CRC32 for byte data
/// C++: CRC_Memory (realcrc.h)
fn compute_crc(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc = crc.wrapping_mul(37).wrapping_add(byte as u32);
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mat_desc_creation() {
        let desc = MeshMatDesc::new();
        assert_eq!(desc.get_pass_count(), 1);
        assert_eq!(desc.get_vertex_count(), 0);
        assert_eq!(desc.get_polygon_count(), 0);
    }

    #[test]
    fn test_mat_desc_reset() {
        let mut desc = MeshMatDesc::new();
        desc.reset(100, 50, 2);
        assert_eq!(desc.get_polygon_count(), 100);
        assert_eq!(desc.get_vertex_count(), 50);
        assert_eq!(desc.get_pass_count(), 2);
    }

    #[test]
    fn test_uv_array_deduplication() {
        let mut desc = MeshMatDesc::new();
        desc.reset(10, 10, 1);

        let uvs1 = vec![Vec2::new(0.0, 0.0); 10];
        let uvs2 = vec![Vec2::new(0.0, 0.0); 10]; // Same as uvs1
        let uvs3 = vec![Vec2::new(1.0, 1.0); 10]; // Different

        desc.install_uv_array(0, 0, uvs1);
        desc.install_uv_array(0, 1, uvs2); // Should reuse uvs1
        desc.install_uv_array(1, 0, uvs3); // Should be new

        // Both pass 0 stages should point to same UV array
        assert_eq!(desc.get_uv_source(0, 0), desc.get_uv_source(0, 1));

        // Pass 1 should point to different UV array
        assert_ne!(desc.get_uv_source(0, 0), desc.get_uv_source(1, 0));

        // Total UV arrays should be 2
        assert_eq!(desc.get_uv_array_count(), 2);
    }

    #[test]
    fn test_color_source() {
        let mut desc = MeshMatDesc::new();
        desc.reset(10, 10, 2);

        desc.set_dcg_source(0, ColorSourceType::Color1);
        desc.set_dig_source(1, ColorSourceType::Color2);

        assert_eq!(desc.get_dcg_source(0), ColorSourceType::Color1);
        assert_eq!(desc.get_dig_source(1), ColorSourceType::Color2);
    }

    #[test]
    fn test_mat_buffer() {
        let buffer = MatBuffer::new(10);
        assert_eq!(buffer.count(), 10);

        // All should be None initially
        assert!(buffer.get_element(0).is_none());
    }

    #[test]
    fn test_tex_buffer() {
        let buffer = TexBuffer::new(10);
        assert_eq!(buffer.count(), 10);

        // All should be None initially
        assert!(buffer.get_element(0).is_none());
    }

    #[test]
    fn test_uv_buffer_crc() {
        let uvs1 = vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0)];
        let uvs2 = vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0)];
        let uvs3 = vec![Vec2::new(0.5, 0.5), Vec2::new(1.0, 1.0)];

        let buf1 = UVBuffer::new(uvs1);
        let buf2 = UVBuffer::new(uvs2);
        let buf3 = UVBuffer::new(uvs3);

        // Same data should have same CRC
        assert_eq!(buf1.get_crc(), buf2.get_crc());
        assert!(buf1.is_equal_to(&buf2));

        // Different data should have different CRC (very likely)
        assert_ne!(buf1.get_crc(), buf3.get_crc());
        assert!(!buf1.is_equal_to(&buf3));
    }
}
