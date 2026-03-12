//! Dynamic Mesh System - Runtime mesh deformation
//!
//! This module provides the DynamicMeshClass functionality from C++ WW3D2,
//! allowing runtime mesh modification for effects like tank treads, building
//! destruction, water surfaces, and other dynamic geometry.

use crate::bounding_volumes::aabox::AABoxClass;
use crate::material_system::VertexMaterialClass;
use crate::rendering::shader_system::shader::ShaderClass;
use crate::texture_system::TextureClass;
use glam::{Mat4, Vec2, Vec3, Vec4};
use std::sync::Arc;
use ww3d_collision::bounding_volumes::sphere::SphereClass;

const MAX_PASSES: usize = 4;
const MAX_COLOR_ARRAYS: usize = 2;

/// Triangle mode for dynamic mesh submission
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriMode {
    /// Triangle strips
    Strips = 0,
    /// Triangle fans
    Fans = 1,
}

/// Dynamic mesh model - low-level rendering data
pub struct DynamicMeshModel {
    /// Current polygon count
    dynamic_mesh_pnum: usize,
    /// Current vertex count
    dynamic_mesh_vnum: usize,
    /// Maximum polygon count
    max_poly_count: usize,
    /// Maximum vertex count
    max_vert_count: usize,

    /// Vertex positions
    vertices: Vec<Vec3>,
    /// Vertex normals
    normals: Vec<Vec3>,
    /// Polygon indices
    polygons: Vec<[u32; 3]>,
    /// UV coordinates per pass
    uv_coords: Vec<Vec<Vec2>>,
    /// Vertex colors per color array
    colors: Vec<Vec<u32>>,

    /// Number of passes
    pass_count: usize,
    /// Texture per polygon per pass per stage
    textures: Vec<Vec<Vec<Option<Arc<TextureClass>>>>>,
    /// Shader per polygon per pass
    shaders: Vec<Vec<ShaderClass>>,
    /// Vertex material per vertex per pass
    vertex_materials: Vec<Vec<Option<Arc<VertexMaterialClass>>>>,

    /// Bounding box
    bounding_box: AABoxClass,
    /// Bounding sphere
    bounding_sphere: SphereClass,
    /// Bounds dirty flag
    bounds_dirty: bool,
}

impl DynamicMeshModel {
    /// Create a new dynamic mesh model
    pub fn new(max_polys: usize, max_verts: usize) -> Self {
        Self {
            dynamic_mesh_pnum: 0,
            dynamic_mesh_vnum: 0,
            max_poly_count: max_polys,
            max_vert_count: max_verts,
            vertices: vec![Vec3::ZERO; max_verts],
            normals: vec![Vec3::ZERO; max_verts],
            polygons: vec![[0, 0, 0]; max_polys],
            uv_coords: vec![vec![Vec2::ZERO; max_verts]],
            colors: vec![vec![0xFFFFFFFF; max_verts]; MAX_COLOR_ARRAYS],
            pass_count: 1,
            textures: vec![vec![vec![None; 1]; max_polys]; 1],
            shaders: vec![vec![ShaderClass::default(); max_polys]; 1],
            vertex_materials: vec![vec![None; max_verts]; 1],
            bounding_box: AABoxClass::default(),
            bounding_sphere: SphereClass::empty(),
            bounds_dirty: true,
        }
    }

    /// Reset the mesh (clear current counts)
    pub fn reset(&mut self) {
        self.dynamic_mesh_pnum = 0;
        self.dynamic_mesh_vnum = 0;
        self.bounds_dirty = true;
    }

    /// Set current polygon and vertex counts
    pub fn set_counts(&mut self, pnum: usize, vnum: usize) {
        self.dynamic_mesh_pnum = pnum.min(self.max_poly_count);
        self.dynamic_mesh_vnum = vnum.min(self.max_vert_count);
        self.bounds_dirty = true;
    }

    /// Get vertex array
    pub fn get_vertex_array(&mut self) -> &mut [Vec3] {
        &mut self.vertices
    }

    /// Get normal array
    pub fn get_normal_array(&mut self) -> &mut [Vec3] {
        &mut self.normals
    }

    /// Get color array for a specific index
    pub fn get_color_array(&mut self, color_array_index: usize) -> Option<&mut [u32]> {
        if color_array_index < MAX_COLOR_ARRAYS {
            Some(&mut self.colors[color_array_index])
        } else {
            None
        }
    }

    /// Get UV array for a specific index
    pub fn get_uv_array(&mut self, uv_array_index: usize) -> Option<&mut [Vec2]> {
        self.uv_coords
            .get_mut(uv_array_index)
            .map(|v| v.as_mut_slice())
    }

    /// Set pass count
    pub fn set_pass_count(&mut self, passes: usize) {
        self.pass_count = passes.min(MAX_PASSES);
    }

    /// Get pass count
    pub fn get_pass_count(&self) -> usize {
        self.pass_count
    }

    /// Set single texture for all polygons
    pub fn set_single_texture(
        &mut self,
        tex: Option<Arc<TextureClass>>,
        pass: usize,
        stage: usize,
    ) {
        if pass < self.pass_count {
            for poly_idx in 0..self.max_poly_count {
                if poly_idx >= self.textures[pass].len() {
                    self.textures[pass].resize(poly_idx + 1, vec![None; stage + 1]);
                }
                if stage >= self.textures[pass][poly_idx].len() {
                    self.textures[pass][poly_idx].resize(stage + 1, None);
                }
                self.textures[pass][poly_idx][stage] = tex.clone();
            }
        }
    }

    /// Set single shader for all polygons
    pub fn set_single_shader(&mut self, shader: ShaderClass, pass: usize) {
        if pass < self.pass_count {
            for poly_idx in 0..self.max_poly_count {
                self.shaders[pass][poly_idx] = shader.clone();
            }
        }
    }

    /// Compute vertex normals from face normals
    pub fn compute_vertex_normals(&mut self) {
        // Reset normals
        for normal in &mut self.normals {
            *normal = Vec3::ZERO;
        }

        // Accumulate face normals
        for poly_idx in 0..self.dynamic_mesh_pnum {
            let poly = self.polygons[poly_idx];
            let v0 = self.vertices[poly[0] as usize];
            let v1 = self.vertices[poly[1] as usize];
            let v2 = self.vertices[poly[2] as usize];

            let edge1 = v1 - v0;
            let edge2 = v2 - v0;
            let face_normal = edge1.cross(edge2);

            self.normals[poly[0] as usize] += face_normal;
            self.normals[poly[1] as usize] += face_normal;
            self.normals[poly[2] as usize] += face_normal;
        }

        // Normalize
        for normal in &mut self.normals[0..self.dynamic_mesh_vnum] {
            let length = normal.length();
            if length > 0.0001 {
                *normal /= length;
            }
        }
    }

    /// Compute bounding volumes
    pub fn compute_bounds(&mut self) {
        if !self.bounds_dirty || self.dynamic_mesh_vnum == 0 {
            return;
        }

        let mut min = self.vertices[0];
        let mut max = self.vertices[0];

        for vert in &self.vertices[1..self.dynamic_mesh_vnum] {
            min = min.min(*vert);
            max = max.max(*vert);
        }

        self.bounding_box = AABoxClass::from_min_max(min, max);

        let center = (min + max) * 0.5;
        let mut radius = 0.0f32;
        for vert in &self.vertices[0..self.dynamic_mesh_vnum] {
            let dist = (*vert - center).length();
            radius = radius.max(dist);
        }

        self.bounding_sphere = SphereClass::new(center, radius);
        self.bounds_dirty = false;
    }

    /// Get bounding box
    pub fn get_bounding_box(&mut self) -> &AABoxClass {
        if self.bounds_dirty {
            self.compute_bounds();
        }
        &self.bounding_box
    }

    /// Get bounding sphere
    pub fn get_bounding_sphere(&mut self) -> &SphereClass {
        if self.bounds_dirty {
            self.compute_bounds();
        }
        &self.bounding_sphere
    }
}

/// Dynamic mesh class - high-level interface for runtime mesh modification
pub struct DynamicMeshClass {
    /// Low-level mesh model
    model: DynamicMeshModel,

    /// Current vertex count
    vert_count: usize,
    /// Current polygon count
    poly_count: usize,

    /// Triangle vertex count (for strip/fan building)
    tri_vertex_count: usize,
    /// Fan base vertex
    fan_vertex: usize,
    /// Triangle mode (strips or fans)
    tri_mode: TriMode,

    /// Current texture index per pass
    texture_idx: [i32; MAX_PASSES],
    /// Current vertex material index per pass
    vertex_material_idx: [i32; MAX_PASSES],
    /// Multi-texture flag per pass
    multi_texture: [bool; MAX_PASSES],
    /// Multi-vertex material flag per pass
    multi_vertex_material: [bool; MAX_PASSES],

    /// Current vertex color per color array
    cur_vertex_color: [Vec4; MAX_COLOR_ARRAYS],
    /// Multi-vertex color flag per color array
    multi_vertex_color: [bool; MAX_COLOR_ARRAYS],

    /// Static sort level
    sort_level: i32,

    /// Transform matrix
    transform: Mat4,
}

impl DynamicMeshClass {
    /// Create a new dynamic mesh
    pub fn new(max_poly: usize, max_vert: usize) -> Self {
        Self {
            model: DynamicMeshModel::new(max_poly, max_vert),
            vert_count: 0,
            poly_count: 0,
            tri_vertex_count: 0,
            fan_vertex: 0,
            tri_mode: TriMode::Strips,
            texture_idx: [-1; MAX_PASSES],
            vertex_material_idx: [-1; MAX_PASSES],
            multi_texture: [false; MAX_PASSES],
            multi_vertex_material: [false; MAX_PASSES],
            cur_vertex_color: [Vec4::ONE; MAX_COLOR_ARRAYS],
            multi_vertex_color: [false; MAX_COLOR_ARRAYS],
            sort_level: 0,
            transform: Mat4::IDENTITY,
        }
    }

    /// Reset the mesh
    pub fn reset(&mut self) {
        self.model.reset();
        self.poly_count = 0;
        self.vert_count = 0;
        self.tri_vertex_count = 0;

        for i in 0..MAX_PASSES {
            self.texture_idx[i] = -1;
            self.vertex_material_idx[i] = -1;
            self.multi_vertex_material[i] = false;
            self.multi_texture[i] = false;
        }
    }

    /// Resize the mesh (reallocate with new max counts)
    pub fn resize(&mut self, max_polys: usize, max_verts: usize) {
        self.model = DynamicMeshModel::new(max_polys, max_verts);
        self.reset();
    }

    /// Begin triangle strip submission
    pub fn begin_tri_strip(&mut self) {
        self.tri_vertex_count = 0;
        self.tri_mode = TriMode::Strips;
    }

    /// Begin triangle fan submission
    pub fn begin_tri_fan(&mut self) {
        self.tri_vertex_count = 0;
        self.tri_mode = TriMode::Fans;
        self.fan_vertex = self.vert_count;
    }

    /// Begin vertex submission
    pub fn begin_vertex(&mut self) {
        // Nothing to do for now
    }

    /// Set vertex location
    pub fn location(&mut self, x: f32, y: f32, z: f32) {
        if self.vert_count < self.model.max_vert_count {
            self.model.vertices[self.vert_count] = Vec3::new(x, y, z);
        }
    }

    /// Set vertex location inline (optimized version)
    pub fn location_inline(&mut self, x: f32, y: f32, z: f32) {
        if self.vert_count < self.model.max_vert_count {
            self.model.vertices[self.vert_count] = Vec3::new(x, y, z);
        }
    }

    /// Set vertex normal
    pub fn normal(&mut self, x: f32, y: f32, z: f32) {
        if self.vert_count < self.model.max_vert_count {
            self.model.normals[self.vert_count] = Vec3::new(x, y, z);
        }
    }

    /// Set vertex color
    pub fn color(&mut self, r: f32, g: f32, b: f32, a: f32, color_array_index: usize) {
        if self.vert_count < self.model.max_vert_count && color_array_index < MAX_COLOR_ARRAYS {
            let color = convert_color_clamp(Vec4::new(r, g, b, a));
            self.model.colors[color_array_index][self.vert_count] = color;
        }
    }

    /// Set vertex UV coordinates
    pub fn uv(&mut self, u: f32, v: f32, uv_array_index: usize) {
        if self.vert_count < self.model.max_vert_count {
            if let Some(uv_array) = self.model.get_uv_array(uv_array_index) {
                uv_array[self.vert_count] = Vec2::new(u, v);
            }
        }
    }

    /// End vertex submission
    pub fn end_vertex(&mut self) -> bool {
        self.vert_count += 1;
        self.tri_vertex_count += 1;

        // Build triangles for strips or fans
        if self.tri_vertex_count >= 3 {
            let should_add = match self.tri_mode {
                TriMode::Strips => true,
                TriMode::Fans => true,
            };

            if should_add && self.poly_count < self.model.max_poly_count {
                match self.tri_mode {
                    TriMode::Strips => {
                        let idx0 = (self.vert_count - 3) as u32;
                        let idx1 = (self.vert_count - 2) as u32;
                        let idx2 = (self.vert_count - 1) as u32;

                        if self.should_flip_face() {
                            self.model.polygons[self.poly_count] = [idx0, idx2, idx1];
                        } else {
                            self.model.polygons[self.poly_count] = [idx0, idx1, idx2];
                        }
                    }
                    TriMode::Fans => {
                        let idx0 = self.fan_vertex as u32;
                        let idx1 = (self.vert_count - 2) as u32;
                        let idx2 = (self.vert_count - 1) as u32;
                        self.model.polygons[self.poly_count] = [idx0, idx1, idx2];
                    }
                }

                self.poly_count += 1;
                self.model.set_counts(self.poly_count, self.vert_count);
            }
        }

        self.vert_count <= self.model.max_vert_count
    }

    /// Check if face needs to be flipped for strip winding
    fn should_flip_face(&self) -> bool {
        !(self.tri_vertex_count & 1 != 0)
    }

    /// End triangle strip
    pub fn end_tri_strip(&mut self) {
        self.tri_vertex_count = 0;
    }

    /// End triangle fan
    pub fn end_tri_fan(&mut self) {
        self.tri_vertex_count = 0;
    }

    /// Move a vertex to a new location
    pub fn move_vertex(&mut self, index: usize, x: f32, y: f32, z: f32) {
        if index < self.vert_count {
            self.model.vertices[index] = Vec3::new(x, y, z);
            self.model.bounds_dirty = true;
        }
    }

    /// Get vertex location
    pub fn get_vertex(&self, index: usize) -> Option<(f32, f32, f32)> {
        if index < self.vert_count {
            let v = self.model.vertices[index];
            Some((v.x, v.y, v.z))
        } else {
            None
        }
    }

    /// Translate all vertices by an offset
    pub fn translate_vertices(&mut self, offset: Vec3) {
        for i in 0..self.vert_count {
            self.model.vertices[i] += offset;
        }
        self.model.bounds_dirty = true;
    }

    /// Set single shader for all polygons
    pub fn set_shader(&mut self, shader: ShaderClass, pass: usize) {
        self.model.set_single_shader(shader, pass);
    }

    /// Set single texture for all polygons
    pub fn set_single_texture(&mut self, tex: Option<Arc<TextureClass>>, pass: usize) {
        self.model.set_single_texture(tex, pass, 0);
    }

    /// Set pass count
    pub fn set_pass_count(&mut self, passes: usize) {
        self.model.set_pass_count(passes);
    }

    /// Get pass count
    pub fn get_pass_count(&self) -> usize {
        self.model.get_pass_count()
    }

    /// Get number of polygons
    pub fn get_num_polys(&self) -> usize {
        self.poly_count
    }

    /// Get number of vertices
    pub fn get_num_vertices(&self) -> usize {
        self.vert_count
    }

    /// Get object space bounding sphere
    pub fn get_obj_space_bounding_sphere(&mut self) -> SphereClass {
        self.model.get_bounding_sphere().clone()
    }

    /// Get object space bounding box
    pub fn get_obj_space_bounding_box(&mut self) -> AABoxClass {
        self.model.get_bounding_box().clone()
    }

    /// Set transform
    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }

    /// Get transform
    pub fn get_transform(&self) -> Mat4 {
        self.transform
    }
}

/// Convert Vec4 color to u32 ARGB format with clamping
fn convert_color_clamp(color: Vec4) -> u32 {
    let r = (color.x.clamp(0.0, 1.0) * 255.0) as u32;
    let g = (color.y.clamp(0.0, 1.0) * 255.0) as u32;
    let b = (color.z.clamp(0.0, 1.0) * 255.0) as u32;
    let a = (color.w.clamp(0.0, 1.0) * 255.0) as u32;

    (a << 24) | (r << 16) | (g << 8) | b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_mesh_creation() {
        let mesh = DynamicMeshClass::new(100, 300);
        assert_eq!(mesh.get_num_polys(), 0);
        assert_eq!(mesh.get_num_vertices(), 0);
    }

    #[test]
    fn test_triangle_strip() {
        let mut mesh = DynamicMeshClass::new(100, 300);

        mesh.begin_tri_strip();

        // Add 4 vertices to make 2 triangles
        for i in 0..4 {
            mesh.begin_vertex();
            mesh.location_inline(i as f32, 0.0, 0.0);
            mesh.uv(i as f32, 0.0, 0);
            mesh.end_vertex();
        }

        mesh.end_tri_strip();

        assert_eq!(mesh.get_num_vertices(), 4);
        assert_eq!(mesh.get_num_polys(), 2);
    }

    #[test]
    fn test_triangle_fan() {
        let mut mesh = DynamicMeshClass::new(100, 300);

        mesh.begin_tri_fan();

        // Add 5 vertices to make 3 triangles in a fan
        for i in 0..5 {
            mesh.begin_vertex();
            mesh.location_inline(i as f32, i as f32, 0.0);
            mesh.uv(0.0, 0.0, 0);
            mesh.end_vertex();
        }

        mesh.end_tri_fan();

        assert_eq!(mesh.get_num_vertices(), 5);
        assert_eq!(mesh.get_num_polys(), 3);
    }

    #[test]
    fn test_vertex_translation() {
        let mut mesh = DynamicMeshClass::new(100, 300);

        mesh.begin_vertex();
        mesh.location(1.0, 2.0, 3.0);
        mesh.end_vertex();

        mesh.translate_vertices(Vec3::new(1.0, 1.0, 1.0));

        let (x, y, z) = mesh.get_vertex(0).unwrap();
        assert!((x - 2.0).abs() < 0.001);
        assert!((y - 3.0).abs() < 0.001);
        assert!((z - 4.0).abs() < 0.001);
    }
}
