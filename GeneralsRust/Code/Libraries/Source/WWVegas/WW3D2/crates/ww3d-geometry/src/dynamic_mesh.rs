//! Dynamic Mesh Model
//!
//! This module provides dynamic mesh functionality that can be modified
//! at runtime, similar to the C++ DynamicMeshModel class.

use crate::mesh_geometry::{MeshTriangle, MeshVertex};
use crate::mesh_mat_desc::{Texture, VertexMaterial};
use crate::render_info::RenderInfoClass;
use crate::*;
use glam::{Vec2, Vec3};
use std::cell::RefCell;
use std::rc::Rc;

/// Dynamic mesh model that can be modified at runtime
#[derive(Debug)]
pub struct DynamicMeshModel {
    pub mesh_geometry: MeshGeometry,
    pub max_polygons: usize,
    pub max_vertices: usize,
    pub current_polygon_count: usize,
    pub current_vertex_count: usize,
    pub material_desc: Rc<RefCell<MeshMaterialDesc>>,
    pub material_info: Rc<RefCell<MaterialInfo>>,
    pub vertex_buffer: Option<DynamicVertexBuffer>,
    pub index_buffer: Option<DynamicIndexBuffer>,
}

impl DynamicMeshModel {
    /// Create a new dynamic mesh model
    pub fn new(max_polygons: usize, max_vertices: usize) -> Self {
        let material_info = Rc::new(RefCell::new(MaterialInfo::new()));
        let material_desc = Rc::new(RefCell::new(MeshMaterialDesc::new(
            max_polygons,
            max_vertices,
        )));

        Self {
            mesh_geometry: MeshGeometry::new(),
            max_polygons,
            max_vertices,
            current_polygon_count: 0,
            current_vertex_count: 0,
            material_desc,
            material_info,
            vertex_buffer: None,
            index_buffer: None,
        }
    }

    /// Create a new dynamic mesh model with custom material info
    pub fn with_material_info(
        max_polygons: usize,
        max_vertices: usize,
        material_info: Rc<RefCell<MaterialInfo>>,
    ) -> Self {
        let material_desc = Rc::new(RefCell::new(MeshMaterialDesc::new(
            max_polygons,
            max_vertices,
        )));

        Self {
            mesh_geometry: MeshGeometry::new(),
            max_polygons,
            max_vertices,
            current_polygon_count: 0,
            current_vertex_count: 0,
            material_desc,
            material_info,
            vertex_buffer: None,
            index_buffer: None,
        }
    }

    /// Reset the mesh geometry
    pub fn reset(&mut self) {
        self.reset_geometry(self.max_polygons, self.max_vertices);
    }

    /// Reset geometry with new counts
    pub fn reset_geometry(&mut self, max_polygons: usize, max_vertices: usize) {
        self.max_polygons = max_polygons;
        self.max_vertices = max_vertices;
        self.current_polygon_count = 0;
        self.current_vertex_count = 0;

        self.mesh_geometry = MeshGeometry::new();
        self.material_desc = Rc::new(RefCell::new(MeshMaterialDesc::new(
            max_polygons,
            max_vertices,
        )));

        // Reset vertex and index buffers
        self.vertex_buffer = Some(DynamicVertexBuffer::new(max_vertices));
        self.index_buffer = Some(DynamicIndexBuffer::new(max_polygons * 3));
    }

    /// Set current polygon and vertex counts
    pub fn set_counts(&mut self, polygon_count: usize, vertex_count: usize) {
        self.current_polygon_count = polygon_count.min(self.max_polygons);
        self.current_vertex_count = vertex_count.min(self.max_vertices);
    }

    /// Add a vertex to the mesh
    pub fn add_vertex(&mut self, position: Vec3, normal: Vec3, tex_coords: Vec2) -> usize {
        if self.current_vertex_count >= self.max_vertices {
            return 0; // Cannot add more vertices
        }

        let vertex = MeshVertex::new(position, normal, tex_coords);
        self.mesh_geometry.add_vertex(vertex);

        if let Some(ref mut vb) = self.vertex_buffer {
            vb.update_vertex(self.current_vertex_count, &vertex);
        }

        self.current_vertex_count += 1;
        self.current_vertex_count - 1
    }

    /// Add a triangle to the mesh
    pub fn add_triangle(&mut self, v0: usize, v1: usize, v2: usize) {
        if self.current_polygon_count >= self.max_polygons {
            return; // Cannot add more polygons
        }

        if v0 >= self.current_vertex_count
            || v1 >= self.current_vertex_count
            || v2 >= self.current_vertex_count
        {
            return; // Invalid vertex indices
        }

        let triangle = MeshTriangle::new(v0 as u32, v1 as u32, v2 as u32, 0);
        self.mesh_geometry.add_triangle(triangle);

        if let Some(ref mut ib) = self.index_buffer {
            let base_index = self.current_polygon_count * 3;
            ib.update_index(base_index, v0 as u32);
            ib.update_index(base_index + 1, v1 as u32);
            ib.update_index(base_index + 2, v2 as u32);
        }

        self.current_polygon_count += 1;
    }

    /// Add a quad (two triangles)
    pub fn add_quad(&mut self, v0: usize, v1: usize, v2: usize, v3: usize) {
        self.add_triangle(v0, v1, v2);
        self.add_triangle(v0, v2, v3);
    }

    /// Update vertex position
    pub fn update_vertex_position(&mut self, index: usize, position: Vec3) {
        if index < self.mesh_geometry.vertices.len() {
            self.mesh_geometry.vertices[index].position = position;

            if let Some(ref mut vb) = self.vertex_buffer {
                vb.update_vertex_position(index, position);
            }

            self.mesh_geometry.update_bounds();
        }
    }

    /// Update vertex normal
    pub fn update_vertex_normal(&mut self, index: usize, normal: Vec3) {
        if index < self.mesh_geometry.vertices.len() {
            self.mesh_geometry.vertices[index].normal = normal;

            if let Some(ref mut vb) = self.vertex_buffer {
                vb.update_vertex_normal(index, normal);
            }
        }
    }

    /// Update vertex texture coordinates
    pub fn update_vertex_tex_coords(&mut self, index: usize, tex_coords: Vec2) {
        if index < self.mesh_geometry.vertices.len() {
            self.mesh_geometry.vertices[index].tex_coords = tex_coords;

            if let Some(ref mut vb) = self.vertex_buffer {
                vb.update_vertex_tex_coords(index, tex_coords);
            }
        }
    }

    /// Set single material for all polygons
    pub fn set_single_material(&mut self, material: &VertexMaterial, pass: usize) {
        self.material_desc
            .borrow_mut()
            .set_single_material(material, pass);
    }

    /// Set single texture for all polygons
    pub fn set_single_texture(&mut self, texture: &Texture, pass: usize, stage: usize) {
        self.material_desc
            .borrow_mut()
            .set_single_texture(texture, pass, stage);
    }

    /// Set material for specific polygon
    pub fn set_polygon_material(
        &mut self,
        polygon_index: usize,
        material: &VertexMaterial,
        pass: usize,
    ) {
        self.material_desc
            .borrow_mut()
            .set_polygon_material(polygon_index, material, pass);
    }

    /// Set texture for specific polygon
    pub fn set_polygon_texture(
        &mut self,
        polygon_index: usize,
        texture: &Texture,
        pass: usize,
        stage: usize,
    ) {
        self.material_desc
            .borrow_mut()
            .set_polygon_texture(polygon_index, texture, pass, stage);
    }

    /// Get vertex buffer for rendering
    pub fn get_vertex_buffer(&self) -> Option<&DynamicVertexBuffer> {
        self.vertex_buffer.as_ref()
    }

    /// Get index buffer for rendering
    pub fn get_index_buffer(&self) -> Option<&DynamicIndexBuffer> {
        self.index_buffer.as_ref()
    }

    /// Render the mesh
    pub fn render(&self, render_info: &RenderInfoClass) {
        // Update vertex and index buffers if needed
        if let Some(ref vb) = self.vertex_buffer {
            if let Some(ref ib) = self.index_buffer {
                // In a real implementation, this would submit to the GPU
                // For now, we'll just mark the buffers as needing update
                let _ = vb;
                let _ = ib;
                let _ = render_info;
            }
        }
    }

    /// Compute plane equations for polygons
    pub fn compute_plane_equations(&mut self) {
        for triangle in &mut self.mesh_geometry.triangles {
            let v0 = self.mesh_geometry.vertices[triangle.indices[0] as usize];
            let v1 = self.mesh_geometry.vertices[triangle.indices[1] as usize];
            let v2 = self.mesh_geometry.vertices[triangle.indices[2] as usize];

            // Compute triangle normal
            let edge1 = v1.position - v0.position;
            let edge2 = v2.position - v0.position;
            let normal = edge1.cross(edge2).normalize();

            // Compute distance from origin
            let distance = -normal.dot(v0.position);

            // Store plane equation (in a real implementation, this might be stored per triangle)
            let _plane = Plane::new(normal, distance);
        }
    }

    /// Compute vertex normals
    pub fn compute_vertex_normals(&mut self) {
        self.mesh_geometry.compute_normals();
        self.mesh_geometry.compute_plane_equations();
    }

    /// Compute bounds
    pub fn compute_bounds(&mut self) {
        self.mesh_geometry.update_bounds();
    }

    /// Get current polygon count
    pub fn polygon_count(&self) -> usize {
        self.current_polygon_count
    }

    /// Get current vertex count
    pub fn vertex_count(&self) -> usize {
        self.current_vertex_count
    }

    /// Get maximum polygon count
    pub fn max_polygon_count(&self) -> usize {
        self.max_polygons
    }

    /// Get maximum vertex count
    pub fn max_vertex_count(&self) -> usize {
        self.max_vertices
    }
}

impl Clone for DynamicMeshModel {
    fn clone(&self) -> Self {
        Self {
            mesh_geometry: self.mesh_geometry.clone(),
            max_polygons: self.max_polygons,
            max_vertices: self.max_vertices,
            current_polygon_count: self.current_polygon_count,
            current_vertex_count: self.current_vertex_count,
            material_desc: Rc::new(RefCell::new(self.material_desc.borrow().clone())),
            material_info: Rc::new(RefCell::new(self.material_info.borrow().clone())),
            vertex_buffer: self.vertex_buffer.clone(),
            index_buffer: self.index_buffer.clone(),
        }
    }
}

/// Dynamic vertex buffer for efficient GPU updates
#[derive(Debug, Clone)]
pub struct DynamicVertexBuffer {
    pub vertices: Vec<MeshVertex>,
    pub max_vertices: usize,
    pub dirty: bool,
}

impl DynamicVertexBuffer {
    pub fn new(max_vertices: usize) -> Self {
        Self {
            vertices: Vec::with_capacity(max_vertices),
            max_vertices,
            dirty: false,
        }
    }

    pub fn update_vertex(&mut self, index: usize, vertex: &MeshVertex) {
        if index >= self.vertices.len() {
            self.vertices
                .resize(index + 1, MeshVertex::new(Vec3::ZERO, Vec3::Y, Vec2::ZERO));
        }
        self.vertices[index] = *vertex;
        self.dirty = true;
    }

    pub fn update_vertex_position(&mut self, index: usize, position: Vec3) {
        if index < self.vertices.len() {
            self.vertices[index].position = position;
            self.dirty = true;
        }
    }

    pub fn update_vertex_normal(&mut self, index: usize, normal: Vec3) {
        if index < self.vertices.len() {
            self.vertices[index].normal = normal;
            self.dirty = true;
        }
    }

    pub fn update_vertex_tex_coords(&mut self, index: usize, tex_coords: Vec2) {
        if index < self.vertices.len() {
            self.vertices[index].tex_coords = tex_coords;
            self.dirty = true;
        }
    }

    pub fn clear_dirty_flag(&mut self) {
        self.dirty = false;
    }
}

/// Dynamic index buffer for efficient GPU updates
#[derive(Debug, Clone)]
pub struct DynamicIndexBuffer {
    pub indices: Vec<u32>,
    pub max_indices: usize,
    pub dirty: bool,
}

impl DynamicIndexBuffer {
    pub fn new(max_indices: usize) -> Self {
        Self {
            indices: Vec::with_capacity(max_indices),
            max_indices,
            dirty: false,
        }
    }

    pub fn update_index(&mut self, index: usize, value: u32) {
        if index >= self.indices.len() {
            self.indices.resize(index + 1, 0);
        }
        self.indices[index] = value;
        self.dirty = true;
    }

    pub fn update_indices(&mut self, start_index: usize, values: &[u32]) {
        for (i, &value) in values.iter().enumerate() {
            self.update_index(start_index + i, value);
        }
    }

    pub fn clear_dirty_flag(&mut self) {
        self.dirty = false;
    }
}

/// Mesh material description
#[derive(Debug, Clone)]
pub struct MeshMaterialDesc {
    pub max_polygons: usize,
    pub max_vertices: usize,
    pub polygon_materials: Vec<Option<Rc<VertexMaterial>>>,
    pub vertex_materials: Vec<Option<Rc<VertexMaterial>>>,
    pub polygon_textures: Vec<Vec<Option<Rc<Texture>>>>,
    pub vertex_textures: Vec<Vec<Option<Rc<Texture>>>>,
    pub pass_count: usize,
}

impl MeshMaterialDesc {
    pub fn new(max_polygons: usize, max_vertices: usize) -> Self {
        Self {
            max_polygons,
            max_vertices,
            polygon_materials: vec![None; max_polygons],
            vertex_materials: vec![None; max_vertices],
            polygon_textures: vec![vec![None; 8]; max_polygons], // 8 texture stages
            vertex_textures: vec![vec![None; 8]; max_vertices],
            pass_count: 1,
        }
    }

    pub fn set_single_material(&mut self, material: &VertexMaterial, _pass: usize) {
        let material_rc = Rc::new(material.clone());
        for mat in &mut self.polygon_materials {
            *mat = Some(material_rc.clone());
        }
        for mat in &mut self.vertex_materials {
            *mat = Some(material_rc.clone());
        }
    }

    pub fn set_single_texture(&mut self, texture: &Texture, _pass: usize, stage: usize) {
        let texture_rc = Rc::new(texture.clone());
        for poly_textures in &mut self.polygon_textures {
            if stage < poly_textures.len() {
                poly_textures[stage] = Some(texture_rc.clone());
            }
        }
        for vert_textures in &mut self.vertex_textures {
            if stage < vert_textures.len() {
                vert_textures[stage] = Some(texture_rc.clone());
            }
        }
    }

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

    pub fn set_pass_count(&mut self, passes: usize) {
        self.pass_count = passes;
    }

    pub fn get_pass_count(&self) -> usize {
        self.pass_count
    }
}

/// Material info structure
#[derive(Debug, Clone)]
pub struct MaterialInfo {
    pub vertex_materials: Vec<Rc<VertexMaterial>>,
    pub textures: Vec<Rc<Texture>>,
}

impl Default for MaterialInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl MaterialInfo {
    pub fn new() -> Self {
        Self {
            vertex_materials: Vec::new(),
            textures: Vec::new(),
        }
    }

    pub fn add_vertex_material(&mut self, material: VertexMaterial) {
        self.vertex_materials.push(Rc::new(material));
    }

    pub fn add_texture(&mut self, texture: Texture) {
        self.textures.push(Rc::new(texture));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_mesh_creation() {
        let mesh = DynamicMeshModel::new(100, 100);
        assert_eq!(mesh.max_polygons, 100);
        assert_eq!(mesh.max_vertices, 100);
        assert_eq!(mesh.polygon_count(), 0);
        assert_eq!(mesh.vertex_count(), 0);
    }

    #[test]
    fn test_dynamic_mesh_add_geometry() {
        let mut mesh = DynamicMeshModel::new(10, 10);

        let v0 = mesh.add_vertex(Vec3::new(0.0, 0.0, 0.0), Vec3::Y, Vec2::ZERO);
        let v1 = mesh.add_vertex(Vec3::new(1.0, 0.0, 0.0), Vec3::Y, Vec2::new(1.0, 0.0));
        let v2 = mesh.add_vertex(Vec3::new(0.0, 1.0, 0.0), Vec3::Y, Vec2::new(0.0, 1.0));

        mesh.add_triangle(v0, v1, v2);

        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.polygon_count(), 1);
    }

    #[test]
    fn test_dynamic_mesh_reset() {
        let mut mesh = DynamicMeshModel::new(10, 10);

        mesh.add_vertex(Vec3::ZERO, Vec3::Y, Vec2::ZERO);
        mesh.reset();

        assert_eq!(mesh.vertex_count(), 0);
        assert_eq!(mesh.polygon_count(), 0);
    }

    #[test]
    fn test_dynamic_vertex_buffer() {
        let mut vb = DynamicVertexBuffer::new(10);
        let vertex = MeshVertex::new(Vec3::ZERO, Vec3::Y, Vec2::ZERO);

        vb.update_vertex(0, &vertex);
        assert!(vb.dirty);

        vb.clear_dirty_flag();
        assert!(!vb.dirty);
    }

    #[test]
    fn test_dynamic_index_buffer() {
        let mut ib = DynamicIndexBuffer::new(30); // 10 triangles * 3

        ib.update_index(0, 0);
        ib.update_index(1, 1);
        ib.update_index(2, 2);

        assert!(ib.dirty);
        assert_eq!(ib.indices[0], 0);
        assert_eq!(ib.indices[1], 1);
        assert_eq!(ib.indices[2], 2);
    }
}
