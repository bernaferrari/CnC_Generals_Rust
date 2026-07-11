//! Mesh Material Description
//!
//! This module handles material descriptions for meshes, including
//! vertex materials, textures, and UV coordinates per polygon/vertex.

use crate::*;
use glam::{Vec2, Vec3};
use std::hash::Hash;
use std::rc::Rc;

/// Mesh material description class
#[derive(Debug, Clone)]
pub struct MeshMatDesc {
    pub max_polygons: usize,
    pub max_vertices: usize,
    pub polygon_materials: Vec<Option<Rc<VertexMaterial>>>,
    pub vertex_materials: Vec<Option<Rc<VertexMaterial>>>,
    pub polygon_textures: Vec<Vec<Option<Rc<Texture>>>>,
    pub vertex_textures: Vec<Vec<Option<Rc<Texture>>>>,
    pub uv_buffers: Vec<UVBuffer>,
    pub pass_count: usize,
    pub shader_overrides: Vec<Option<Shader>>,
}

impl MeshMatDesc {
    /// Create a new mesh material description
    pub fn new(max_polygons: usize, max_vertices: usize) -> Self {
        Self {
            max_polygons,
            max_vertices,
            polygon_materials: vec![None; max_polygons],
            vertex_materials: vec![None; max_vertices],
            polygon_textures: vec![vec![None; 8]; max_polygons], // Support up to 8 texture stages
            vertex_textures: vec![vec![None; 8]; max_vertices],
            uv_buffers: Vec::new(),
            pass_count: 1,
            shader_overrides: vec![None; max_polygons],
        }
    }

    /// Set polygon count
    pub fn set_polygon_count(&mut self, count: usize) {
        self.max_polygons = count;
        self.polygon_materials.resize(count, None);
        self.polygon_textures.resize(count, vec![None; 8]);
        self.shader_overrides.resize(count, None);
    }

    /// Set vertex count
    pub fn set_vertex_count(&mut self, count: usize) {
        self.max_vertices = count;
        self.vertex_materials.resize(count, None);
        self.vertex_textures.resize(count, vec![None; 8]);
    }

    /// Set single material for all polygons
    pub fn set_single_material(&mut self, material: &VertexMaterial, _pass: usize) {
        let material_rc = Rc::new(material.clone());
        for mat in &mut self.polygon_materials {
            *mat = Some(material_rc.clone());
        }
    }

    /// Set single texture for all polygons
    pub fn set_single_texture(&mut self, texture: &Texture, _pass: usize, stage: usize) {
        if stage >= 8 {
            return;
        }

        let texture_rc = Rc::new(texture.clone());
        for poly_textures in &mut self.polygon_textures {
            if stage < poly_textures.len() {
                poly_textures[stage] = Some(texture_rc.clone());
            }
        }
    }

    /// Set material for specific polygon
    pub fn set_polygon_material(
        &mut self,
        polygon_index: usize,
        material: &VertexMaterial,
        _pass: usize,
    ) {
        if polygon_index < self.polygon_materials.len() {
            self.polygon_materials[polygon_index] = Some(Rc::new(material.clone()));
        }
    }

    /// Set texture for specific polygon
    pub fn set_polygon_texture(
        &mut self,
        polygon_index: usize,
        texture: &Texture,
        _pass: usize,
        stage: usize,
    ) {
        if polygon_index < self.polygon_textures.len()
            && stage < self.polygon_textures[polygon_index].len()
        {
            self.polygon_textures[polygon_index][stage] = Some(Rc::new(texture.clone()));
        }
    }

    /// Set shader for specific polygon
    pub fn set_polygon_shader(&mut self, polygon_index: usize, shader: Shader, _pass: usize) {
        if polygon_index < self.shader_overrides.len() {
            self.shader_overrides[polygon_index] = Some(shader);
        }
    }

    /// Set material for specific vertex
    pub fn set_vertex_material(
        &mut self,
        vertex_index: usize,
        material: &VertexMaterial,
        _pass: usize,
    ) {
        if vertex_index < self.vertex_materials.len() {
            self.vertex_materials[vertex_index] = Some(Rc::new(material.clone()));
        }
    }

    /// Set texture for specific vertex
    pub fn set_vertex_texture(
        &mut self,
        vertex_index: usize,
        texture: &Texture,
        _pass: usize,
        stage: usize,
    ) {
        if vertex_index < self.vertex_textures.len()
            && stage < self.vertex_textures[vertex_index].len()
        {
            self.vertex_textures[vertex_index][stage] = Some(Rc::new(texture.clone()));
        }
    }

    /// Get material for polygon
    pub fn get_polygon_material(
        &self,
        polygon_index: usize,
        _pass: usize,
    ) -> Option<Rc<VertexMaterial>> {
        self.polygon_materials
            .get(polygon_index)
            .and_then(|m| m.clone())
    }

    /// Get texture for polygon
    pub fn get_polygon_texture(
        &self,
        polygon_index: usize,
        _pass: usize,
        stage: usize,
    ) -> Option<Rc<Texture>> {
        self.polygon_textures
            .get(polygon_index)
            .and_then(|textures| textures.get(stage).and_then(|t| t.clone()))
    }

    /// Get material for vertex
    pub fn get_vertex_material(
        &self,
        vertex_index: usize,
        _pass: usize,
    ) -> Option<Rc<VertexMaterial>> {
        self.vertex_materials
            .get(vertex_index)
            .and_then(|m| m.clone())
    }

    /// Get texture for vertex
    pub fn get_vertex_texture(
        &self,
        vertex_index: usize,
        _pass: usize,
        stage: usize,
    ) -> Option<Rc<Texture>> {
        self.vertex_textures
            .get(vertex_index)
            .and_then(|textures| textures.get(stage).and_then(|t| t.clone()))
    }

    /// Set pass count
    pub fn set_pass_count(&mut self, passes: usize) {
        self.pass_count = passes;
    }

    /// Get pass count
    pub fn get_pass_count(&self) -> usize {
        self.pass_count
    }

    /// Add UV buffer
    pub fn add_uv_buffer(&mut self, buffer: UVBuffer) {
        self.uv_buffers.push(buffer);
    }

    /// Get UV buffer
    pub fn get_uv_buffer(&self, index: usize) -> Option<&UVBuffer> {
        self.uv_buffers.get(index)
    }

    /// Initialize texture array for a pass and stage
    pub fn initialize_texture_array(
        &mut self,
        _pass: usize,
        stage: usize,
        texture: Option<&Texture>,
    ) {
        if stage >= 8 {
            return;
        }

        let texture_rc = texture.map(|t| Rc::new(t.clone()));
        for poly_textures in &mut self.polygon_textures {
            if stage < poly_textures.len() {
                poly_textures[stage] = texture_rc.clone();
            }
        }
    }

    /// Initialize material array for a pass
    pub fn initialize_material_array(&mut self, _pass: usize, material: Option<&VertexMaterial>) {
        let material_rc = material.map(|m| Rc::new(m.clone()));
        for mat in &mut self.polygon_materials {
            *mat = material_rc.clone();
        }
    }

    /// Get color array for a color array index (placeholder for now)
    pub fn get_color_array(&self, _color_array_index: usize) -> Option<&[Vec3]> {
        // In a real implementation, this would return actual color arrays
        None
    }

    /// Get UV array by index
    pub fn get_uv_array(&self, uv_array_index: usize) -> Option<&UVBuffer> {
        self.uv_buffers.get(uv_array_index)
    }

    /// Compute CRC for change detection
    pub fn compute_crc(&self) -> u32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Hash polygon materials
        for m in self.polygon_materials.iter().flatten() {
            m.name.hash(&mut hasher);
            Vec3Hashable(m.diffuse_color).hash(&mut hasher);
        }

        // Hash textures
        for poly_textures in &self.polygon_textures {
            for t in poly_textures.iter().flatten() {
                t.name.hash(&mut hasher);
            }
        }

        hasher.finish() as u32
    }
}

/// UV buffer for texture coordinates
#[derive(Debug, Clone)]
pub struct UVBuffer {
    pub uv_coords: Vec<Vec2>,
    pub crc: u32,
}

impl UVBuffer {
    /// Create a new UV buffer
    pub fn new(uv_coords: Vec<Vec2>) -> Self {
        let crc = Self::compute_crc(&uv_coords);
        Self { uv_coords, crc }
    }

    /// Get UV coordinate
    pub fn get_uv(&self, index: usize) -> Option<Vec2> {
        self.uv_coords.get(index).copied()
    }

    /// Set UV coordinate
    pub fn set_uv(&mut self, index: usize, uv: Vec2) {
        if index < self.uv_coords.len() {
            self.uv_coords[index] = uv;
            self.crc = Self::compute_crc(&self.uv_coords);
        }
    }

    /// Get buffer size
    pub fn len(&self) -> usize {
        self.uv_coords.len()
    }

    /// Check equality with another UV buffer
    pub fn equals(&self, other: &UVBuffer) -> bool {
        if self.crc != other.crc || self.uv_coords.len() != other.uv_coords.len() {
            return false;
        }

        for (a, b) in self.uv_coords.iter().zip(other.uv_coords.iter()) {
            if (a.x - b.x).abs() > EPSILON || (a.y - b.y).abs() > EPSILON {
                return false;
            }
        }

        true
    }

    /// Compute CRC for UV coordinates
    fn compute_crc(uv_coords: &[Vec2]) -> u32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        for uv in uv_coords {
            (uv.x.to_bits()).hash(&mut hasher);
            (uv.y.to_bits()).hash(&mut hasher);
        }
        hasher.finish() as u32
    }
}

impl PartialEq for UVBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

/// Vertex material class
#[derive(Debug, Clone)]
pub struct VertexMaterial {
    pub name: String,
    pub diffuse_color: Vec3,
    pub specular_color: Vec3,
    pub emissive_color: Vec3,
    pub shininess: f32,
    pub opacity: f32,
    pub diffuse_texture: Option<Rc<Texture>>,
    pub normal_texture: Option<Rc<Texture>>,
    pub specular_texture: Option<Rc<Texture>>,
}

impl VertexMaterial {
    /// Create a new vertex material
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            diffuse_color: Vec3::new(0.8, 0.8, 0.8),
            specular_color: Vec3::new(1.0, 1.0, 1.0),
            emissive_color: Vec3::ZERO,
            shininess: 32.0,
            opacity: 1.0,
            diffuse_texture: None,
            normal_texture: None,
            specular_texture: None,
        }
    }

    /// Set diffuse texture
    pub fn set_diffuse_texture(&mut self, texture: Rc<Texture>) {
        self.diffuse_texture = Some(texture);
    }

    /// Set normal texture
    pub fn set_normal_texture(&mut self, texture: Rc<Texture>) {
        self.normal_texture = Some(texture);
    }

    /// Set specular texture
    pub fn set_specular_texture(&mut self, texture: Rc<Texture>) {
        self.specular_texture = Some(texture);
    }
}

// Hash implementation for Vec3 - using a wrapper to avoid orphan rule
#[derive(Debug, Clone, Copy)]
pub struct Vec3Hashable(Vec3);

impl Vec3Hashable {
    pub fn new(v: Vec3) -> Self {
        Self(v)
    }

    pub fn inner(&self) -> Vec3 {
        self.0
    }
}

impl Hash for Vec3Hashable {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.x.to_bits().hash(state);
        self.0.y.to_bits().hash(state);
        self.0.z.to_bits().hash(state);
    }
}

impl Hash for VertexMaterial {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        Vec3Hashable(self.diffuse_color).hash(state);
        Vec3Hashable(self.specular_color).hash(state);
        Vec3Hashable(self.emissive_color).hash(state);
        self.shininess.to_bits().hash(state);
        self.opacity.to_bits().hash(state);
    }
}

/// Texture class
#[derive(Debug, Clone)]
pub struct Texture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub data: Option<Vec<u8>>,
}

impl Texture {
    /// Create a new texture
    pub fn new(name: &str, width: u32, height: u32) -> Self {
        Self {
            name: name.to_string(),
            width,
            height,
            data: None,
        }
    }

    /// Create texture with data
    pub fn with_data(name: &str, width: u32, height: u32, data: Vec<u8>) -> Self {
        Self {
            name: name.to_string(),
            width,
            height,
            data: Some(data),
        }
    }
}

impl Hash for Texture {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.width.hash(state);
        self.height.hash(state);
    }
}

/// Shader class
#[derive(Debug, Clone, Copy)]
pub struct Shader {
    pub depth_compare: DepthCompare,
    pub depth_mask: DepthMask,
    pub color_mask: ColorMask,
    pub src_blend: BlendFactor,
    pub dest_blend: BlendFactor,
    pub fog_func: FogFunc,
    pub alpha_test: AlphaTest,
    pub cull_mode: CullMode,
}

impl Shader {
    /// Create default shader
    pub fn default() -> Self {
        Self {
            depth_compare: DepthCompare::LessEqual,
            depth_mask: DepthMask::WriteEnable,
            color_mask: ColorMask::WriteEnable,
            src_blend: BlendFactor::One,
            dest_blend: BlendFactor::Zero,
            fog_func: FogFunc::Disable,
            alpha_test: AlphaTest::Disable,
            cull_mode: CullMode::Backface,
        }
    }
}

/// Shader enumeration types
#[derive(Debug, Clone, Copy)]
pub enum DepthCompare {
    PassAlways,
    PassNever,
    Less,
    LessEqual,
    Equal,
    Greater,
    GreaterEqual,
    NotEqual,
}

#[derive(Debug, Clone, Copy)]
pub enum DepthMask {
    WriteEnable,
    WriteDisable,
}

#[derive(Debug, Clone, Copy)]
pub enum ColorMask {
    WriteEnable,
    WriteDisable,
}

#[derive(Debug, Clone, Copy)]
pub enum BlendFactor {
    Zero,
    One,
    SrcColor,
    InvSrcColor,
    SrcAlpha,
    InvSrcAlpha,
    DestAlpha,
    InvDestAlpha,
}

#[derive(Debug, Clone, Copy)]
pub enum FogFunc {
    Disable,
    Enable,
}

#[derive(Debug, Clone, Copy)]
pub enum AlphaTest {
    Disable,
    Enable,
}

#[derive(Debug, Clone, Copy)]
pub enum CullMode {
    None,
    Frontface,
    Backface,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_mat_desc_creation() {
        let desc = MeshMatDesc::new(10, 20);
        assert_eq!(desc.max_polygons, 10);
        assert_eq!(desc.max_vertices, 20);
        assert_eq!(desc.get_pass_count(), 1);
    }

    #[test]
    fn test_set_single_material() {
        let mut desc = MeshMatDesc::new(5, 10);
        let material = VertexMaterial::new("test_material");

        desc.set_single_material(&material, 0);

        for i in 0..5 {
            let poly_mat = desc.get_polygon_material(i, 0);
            assert!(poly_mat.is_some());
            assert_eq!(poly_mat.unwrap().name, "test_material");
        }
    }

    #[test]
    fn test_set_single_texture() {
        let mut desc = MeshMatDesc::new(5, 10);
        let texture = Texture::new("test_texture", 64, 64);

        desc.set_single_texture(&texture, 0, 0);

        for i in 0..5 {
            let poly_tex = desc.get_polygon_texture(i, 0, 0);
            assert!(poly_tex.is_some());
            assert_eq!(poly_tex.unwrap().name, "test_texture");
        }
    }

    #[test]
    fn test_uv_buffer() {
        let uvs = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 1.0),
        ];

        let mut buffer = UVBuffer::new(uvs);
        assert_eq!(buffer.len(), 3);

        buffer.set_uv(0, Vec2::new(0.5, 0.5));
        assert_eq!(buffer.get_uv(0), Some(Vec2::new(0.5, 0.5)));
    }

    #[test]
    fn test_vertex_material() {
        let mut material = VertexMaterial::new("test");
        assert_eq!(material.name, "test");
        assert_eq!(material.diffuse_color, Vec3::new(0.8, 0.8, 0.8));

        let texture = Rc::new(Texture::new("diffuse", 128, 128));
        material.set_diffuse_texture(texture);

        assert!(material.diffuse_texture.is_some());
    }

    #[test]
    fn test_crc_computation() {
        let mut desc1 = MeshMatDesc::new(5, 10);
        let desc2 = MeshMatDesc::new(5, 10);

        // Initially should have same CRC
        assert_eq!(desc1.compute_crc(), desc2.compute_crc());

        // After setting different materials, should have different CRC
        let material = VertexMaterial::new("different");
        desc1.set_single_material(&material, 0);

        assert_ne!(desc1.compute_crc(), desc2.compute_crc());
    }
}
