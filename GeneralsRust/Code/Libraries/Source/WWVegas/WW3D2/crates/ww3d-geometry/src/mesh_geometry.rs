//! Mesh Geometry Module
//!
//! This module provides comprehensive mesh geometry functionality,
//! matching the original C++ WW3D mesh system including vertex data,
//! triangle lists, mesh optimization, and geometric operations.

use crate::*;
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3, Vec4};
use std::collections::HashMap;

/// Vertex structure with position, normal, and texture coordinates
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct MeshVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub tex_coords: Vec2,
}

impl MeshVertex {
    pub fn new(position: Vec3, normal: Vec3, tex_coords: Vec2) -> Self {
        Self {
            position,
            normal,
            tex_coords,
        }
    }
}

/// Triangle structure for mesh faces
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct MeshTriangle {
    pub indices: [u32; 3], // Vertex indices
    pub material_index: u32,
}

impl MeshTriangle {
    pub fn new(a: u32, b: u32, c: u32, material_index: u32) -> Self {
        Self {
            indices: [a, b, c],
            material_index,
        }
    }
}

/// Material run for efficient rendering
#[derive(Debug, Clone)]
pub struct MaterialRun {
    pub material: MeshMaterial,
    pub start_triangle: usize,
    pub triangle_count: usize,
}

/// Mesh geometry structure
#[derive(Debug, Clone)]
pub struct MeshGeometry {
    pub mesh_name: Option<String>,
    pub user_text: Option<String>,
    pub flags: u32,
    pub sort_level: i32,
    pub w3d_attributes: u32,
    pub vertices: Vec<MeshVertex>,
    pub triangles: Vec<MeshTriangle>,
    pub materials: Vec<MeshMaterial>,
    pub poly_surface_types: Vec<u32>,
    pub plane_equations: Vec<Vec4>,
    pub bounding_box: AABox,
    pub bounding_sphere: Sphere,
    // Optional WW3D extended data
    pub vertex_shade_indices: Option<Vec<u32>>, // parity with W3D vertex shade indices
    pub vertex_influences: Option<Vec<[u8; 4]>>, // packed bone indices (weights kept elsewhere)
    pub poly_count: usize,
    pub vertex_count: usize,
    pub cull_tree: Option<AABTree>,
}

impl MeshGeometry {
    pub fn new() -> Self {
        Self {
            mesh_name: None,
            user_text: None,
            flags: 0,
            sort_level: 0,
            w3d_attributes: 0,
            vertices: Vec::new(),
            triangles: Vec::new(),
            materials: Vec::new(),
            poly_surface_types: Vec::new(),
            plane_equations: Vec::new(),
            bounding_box: AABox::new(Vec3::ZERO, Vec3::ZERO),
            bounding_sphere: Sphere::new(Vec3::ZERO, 0.0),
            vertex_shade_indices: None,
            vertex_influences: None,
            poly_count: 0,
            vertex_count: 0,
            cull_tree: None,
        }
    }

    /// Returns true if a cull tree is already cached.
    pub fn has_cull_tree(&self) -> bool {
        self.cull_tree.is_some()
    }

    /// Ensure a cull tree exists and return an immutable reference to it.
    pub fn ensure_cull_tree(&mut self) -> &AABTree {
        if self.cull_tree.is_none() {
            self.rebuild_cull_tree();
        }
        self.cull_tree.as_ref().unwrap()
    }

    /// Ensure a cull tree exists and return a mutable reference to it.
    pub fn ensure_cull_tree_mut(&mut self) -> &mut AABTree {
        if self.cull_tree.is_none() {
            self.rebuild_cull_tree();
        }
        self.cull_tree.as_mut().unwrap()
    }

    /// Add a vertex to the mesh
    pub fn add_vertex(&mut self, vertex: MeshVertex) {
        self.vertices.push(vertex);
        self.vertex_count = self.vertices.len();
        self.update_bounds();
        self.invalidate_cull_tree();
    }

    /// Add a triangle to the mesh
    pub fn add_triangle(&mut self, triangle: MeshTriangle) {
        self.triangles.push(triangle);
        self.poly_surface_types.push(0);
        self.plane_equations.push(Vec4::ZERO);
        self.poly_count = self.triangles.len();
        self.invalidate_cull_tree();
    }

    /// Add a material to the mesh
    pub fn add_material(&mut self, material: MeshMaterial) {
        self.materials.push(material);
    }

    /// Set mesh name (matches C++ MeshGeometryClass::Set_Name semantics)
    pub fn set_mesh_name<S: Into<String>>(&mut self, name: S) {
        self.mesh_name = Some(name.into());
    }

    /// Get mesh name
    pub fn mesh_name(&self) -> Option<&str> {
        self.mesh_name.as_deref()
    }

    /// Set user text blob
    pub fn set_user_text<S: Into<String>>(&mut self, text: S) {
        self.user_text = Some(text.into());
    }

    /// Get user text
    pub fn user_text(&self) -> Option<&str> {
        self.user_text.as_deref()
    }

    /// Assign surface type metadata for a polygon
    pub fn set_polygon_surface_type(&mut self, index: usize, surface_type: u32) {
        if index < self.poly_surface_types.len() {
            self.poly_surface_types[index] = surface_type;
            self.invalidate_cull_tree();
        }
    }

    /// Read surface type metadata for a polygon
    pub fn polygon_surface_type(&self, index: usize) -> Option<u32> {
        self.poly_surface_types.get(index).copied()
    }

    /// Compute plane equations (Vec4 form) for every polygon
    pub fn compute_plane_equations(&mut self) {
        if self.triangles.is_empty() {
            self.plane_equations.clear();
            return;
        }

        if self.plane_equations.len() != self.triangles.len() {
            self.plane_equations
                .resize(self.triangles.len(), Vec4::ZERO);
        }

        for (i, triangle) in self.triangles.iter().enumerate() {
            let v0 = self.vertices[triangle.indices[0] as usize].position;
            let v1 = self.vertices[triangle.indices[1] as usize].position;
            let v2 = self.vertices[triangle.indices[2] as usize].position;

            let edge1 = v1 - v0;
            let edge2 = v2 - v0;
            let normal = edge1.cross(edge2);

            if normal.length_squared() > EPSILON * EPSILON {
                let n = normal.normalize();
                let d = -n.dot(v0);
                self.plane_equations[i] = Vec4::new(n.x, n.y, n.z, d);
            } else {
                self.plane_equations[i] = Vec4::ZERO;
            }
        }

        self.invalidate_cull_tree();
    }

    /// Access computed plane equations
    pub fn plane_equations(&self) -> &[Vec4] {
        &self.plane_equations
    }

    fn invalidate_cull_tree(&mut self) {
        self.cull_tree = None;
    }

    fn rebuild_cull_tree(&mut self) {
        let mut builder = AABTreeBuilder::new();
        builder.build_aabtree(self);
        self.cull_tree = Some(builder.export());
    }

    /// Update bounding volumes
    pub fn update_bounds(&mut self) {
        if self.vertices.is_empty() {
            self.bounding_box = AABox::new(Vec3::ZERO, Vec3::ZERO);
            self.bounding_sphere = Sphere::new(Vec3::ZERO, 0.0);
            return;
        }

        // Calculate AABB
        let mut min = self.vertices[0].position;
        let mut max = self.vertices[0].position;

        for vertex in &self.vertices {
            min = min.min(vertex.position);
            max = max.max(vertex.position);
        }

        let center = (min + max) / 2.0;
        let extent = (max - min) / 2.0;
        self.bounding_box = AABox::new(center, extent);

        // Calculate bounding sphere
        let mut max_distance_squared: f32 = 0.0;
        for vertex in &self.vertices {
            let distance_squared = (vertex.position - center).length_squared();
            max_distance_squared = max_distance_squared.max(distance_squared);
        }

        self.bounding_sphere = Sphere::new(center, max_distance_squared.sqrt());
        self.invalidate_cull_tree();
    }

    /// Get vertex count
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Get triangle count
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// Compute vertex normals
    pub fn compute_normals(&mut self) {
        // Reset all normals to zero
        for vertex in &mut self.vertices {
            vertex.normal = Vec3::ZERO;
        }

        // Accumulate face normals
        for triangle in &self.triangles {
            let v0 = self.vertices[triangle.indices[0] as usize];
            let v1 = self.vertices[triangle.indices[1] as usize];
            let v2 = self.vertices[triangle.indices[2] as usize];

            let edge1 = v1.position - v0.position;
            let edge2 = v2.position - v0.position;
            let face_normal = edge1.cross(edge2).normalize();

            // Add face normal to each vertex
            self.vertices[triangle.indices[0] as usize].normal += face_normal;
            self.vertices[triangle.indices[1] as usize].normal += face_normal;
            self.vertices[triangle.indices[2] as usize].normal += face_normal;
        }

        // Normalize all vertex normals
        for vertex in &mut self.vertices {
            if vertex.normal.length_squared() > EPSILON {
                vertex.normal = vertex.normal.normalize();
            }
        }
    }

    /// Optimize mesh for rendering (vertex cache optimization)
    pub fn optimize_for_rendering(&mut self) {
        // Implement vertex cache optimization algorithm
        // This is a simplified version - real implementation would use more sophisticated algorithms

        let mut new_triangles = Vec::new();
        let mut new_plane_equations = Vec::new();
        let mut new_surface_types = Vec::new();
        let _used_vertices = vec![false; self.vertices.len()];
        let mut vertex_scores = vec![0.0; self.vertices.len()];

        // Calculate vertex scores based on connectivity
        for triangle in &self.triangles {
            for &vertex_index in &triangle.indices {
                vertex_scores[vertex_index as usize] += 1.0;
            }
        }

        // Sort triangles by vertex scores (highest first)
        let mut triangle_indices: Vec<usize> = (0..self.triangles.len()).collect();
        let triangle_indices_clone = triangle_indices.clone();
        triangle_indices.sort_by(|&a, &b| {
            let score_a = triangle_indices_clone[a..]
                .iter()
                .map(|&i| {
                    vertex_scores[self.triangles[i].indices[0] as usize]
                        + vertex_scores[self.triangles[i].indices[1] as usize]
                        + vertex_scores[self.triangles[i].indices[2] as usize]
                })
                .sum::<f32>();
            let score_b = triangle_indices_clone[b..]
                .iter()
                .map(|&i| {
                    vertex_scores[self.triangles[i].indices[0] as usize]
                        + vertex_scores[self.triangles[i].indices[1] as usize]
                        + vertex_scores[self.triangles[i].indices[2] as usize]
                })
                .sum::<f32>();
            score_b.partial_cmp(&score_a).unwrap()
        });

        // Reorder triangles
        for &index in &triangle_indices {
            new_triangles.push(self.triangles[index]);
            new_plane_equations.push(
                self.plane_equations
                    .get(index)
                    .copied()
                    .unwrap_or(Vec4::ZERO),
            );
            new_surface_types.push(self.poly_surface_types.get(index).copied().unwrap_or(0));
        }

        self.triangles = new_triangles;
        self.plane_equations = new_plane_equations;
        self.poly_surface_types = new_surface_types;
        self.poly_count = self.triangles.len();
        self.invalidate_cull_tree();
    }

    /// Generate tangent space for normal mapping
    pub fn generate_tangents(&mut self) {
        // Initialize tangents and bitangents
        let mut tangents = vec![Vec3::ZERO; self.vertices.len()];
        let mut bitangents = vec![Vec3::ZERO; self.vertices.len()];

        for triangle in &self.triangles {
            let i0 = triangle.indices[0] as usize;
            let i1 = triangle.indices[1] as usize;
            let i2 = triangle.indices[2] as usize;

            let v0 = self.vertices[i0];
            let v1 = self.vertices[i1];
            let v2 = self.vertices[i2];

            let edge1 = v1.position - v0.position;
            let edge2 = v2.position - v0.position;

            let delta_uv1 = v1.tex_coords - v0.tex_coords;
            let delta_uv2 = v2.tex_coords - v0.tex_coords;

            let f = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv2.x * delta_uv1.y);

            let tangent = Vec3::new(
                f * (delta_uv2.y * edge1.x - delta_uv1.y * edge2.x),
                f * (delta_uv2.y * edge1.y - delta_uv1.y * edge2.y),
                f * (delta_uv2.y * edge1.z - delta_uv1.y * edge2.z),
            );

            let bitangent = Vec3::new(
                f * (-delta_uv2.x * edge1.x + delta_uv1.x * edge2.x),
                f * (-delta_uv2.x * edge1.y + delta_uv1.x * edge2.y),
                f * (-delta_uv2.x * edge1.z + delta_uv1.x * edge2.z),
            );

            tangents[i0] += tangent;
            tangents[i1] += tangent;
            tangents[i2] += tangent;

            bitangents[i0] += bitangent;
            bitangents[i1] += bitangent;
            bitangents[i2] += bitangent;
        }

        // Orthogonalize tangents with normals and normalize
        for i in 0..self.vertices.len() {
            let normal = self.vertices[i].normal;
            let tangent = tangents[i];

            // Gram-Schmidt orthogonalization
            let _ortho_tangent = (tangent - normal * normal.dot(tangent)).normalize();

            // Store tangent in vertex (we could extend MeshVertex to include tangent/bitangent)
            // For now, we'll skip storing them as the original WW3D might not have had this
        }
    }

    /// Subdivide mesh using Loop subdivision scheme
    pub fn subdivide(&mut self, iterations: usize) {
        for _ in 0..iterations {
            self.subdivide_once();
        }
    }

    /// Perform one iteration of Loop subdivision
    fn subdivide_once(&mut self) {
        let old_vertex_count = self.vertices.len();
        let _old_triangle_count = self.triangles.len();

        // Create edge map to track new vertices
        let mut edge_map = HashMap::new();
        let mut new_vertices = self.vertices.clone();
        let mut new_triangles = Vec::new();
        let mut new_surface_types = Vec::new();

        // Function to get or create vertex at edge midpoint
        let mut get_edge_vertex = |i0: usize, i1: usize| -> usize {
            let edge_key = if i0 < i1 { (i0, i1) } else { (i1, i0) };

            if let Some(&vertex_index) = edge_map.get(&edge_key) {
                vertex_index
            } else {
                // Create new vertex at edge midpoint
                let v0 = self.vertices[i0];
                let v1 = self.vertices[i1];
                let new_position = (v0.position + v1.position) / 2.0;
                let new_normal = (v0.normal + v1.normal).normalize();
                let new_tex_coords = (v0.tex_coords + v1.tex_coords) / 2.0;

                let new_vertex = MeshVertex::new(new_position, new_normal, new_tex_coords);
                new_vertices.push(new_vertex);
                let new_index = new_vertices.len() - 1;
                edge_map.insert(edge_key, new_index);
                new_index
            }
        };

        // Subdivide each triangle into 4 smaller triangles
        for (poly_index, triangle) in self.triangles.iter().enumerate() {
            let i0 = triangle.indices[0] as usize;
            let i1 = triangle.indices[1] as usize;
            let i2 = triangle.indices[2] as usize;

            let surface_type = self
                .poly_surface_types
                .get(poly_index)
                .copied()
                .unwrap_or(0);

            // Get edge vertices
            let i01 = get_edge_vertex(i0, i1);
            let i12 = get_edge_vertex(i1, i2);
            let i20 = get_edge_vertex(i2, i0);

            // Create 4 new triangles
            new_triangles.push(MeshTriangle::new(
                i0 as u32,
                i01 as u32,
                i20 as u32,
                triangle.material_index,
            ));
            new_surface_types.push(surface_type);
            new_triangles.push(MeshTriangle::new(
                i01 as u32,
                i1 as u32,
                i12 as u32,
                triangle.material_index,
            ));
            new_surface_types.push(surface_type);
            new_triangles.push(MeshTriangle::new(
                i20 as u32,
                i12 as u32,
                i2 as u32,
                triangle.material_index,
            ));
            new_surface_types.push(surface_type);
            new_triangles.push(MeshTriangle::new(
                i01 as u32,
                i12 as u32,
                i20 as u32,
                triangle.material_index,
            ));
            new_surface_types.push(surface_type);
        }

        // Update vertex positions using Loop subdivision weights
        for i in 0..old_vertex_count {
            let vertex = &self.vertices[i];

            // Find neighboring vertices
            let mut neighbors = Vec::new();
            for triangle in &self.triangles {
                for j in 0..3 {
                    if triangle.indices[j] as usize == i {
                        let next_j = (j + 1) % 3;
                        let prev_j = (j + 2) % 3;
                        neighbors.push(triangle.indices[next_j] as usize);
                        neighbors.push(triangle.indices[prev_j] as usize);
                    }
                }
            }

            neighbors.sort();
            neighbors.dedup();

            let n = neighbors.len() as f32;
            if n > 0.0 {
                // Loop subdivision vertex update rule
                let beta = if n > 3.0 { 3.0 / (8.0 * n) } else { 3.0 / 16.0 };

                let mut new_position = vertex.position * (1.0 - n * beta);
                for &neighbor_index in &neighbors {
                    new_position += self.vertices[neighbor_index].position * beta;
                }

                new_vertices[i].position = new_position;
            }
        }

        self.vertices = new_vertices;
        self.triangles = new_triangles;
        self.poly_surface_types = new_surface_types;
        self.plane_equations = vec![Vec4::ZERO; self.triangles.len()];
        self.vertex_count = self.vertices.len();
        self.poly_count = self.triangles.len();
        self.update_bounds();
        self.invalidate_cull_tree();
    }

    /// Weld vertices that are close together
    pub fn weld_vertices(&mut self, threshold: f32) {
        let mut vertex_map = HashMap::new();
        let mut new_vertices = Vec::new();
        let mut index_map = vec![0; self.vertices.len()];

        for (i, vertex) in self.vertices.iter().enumerate() {
            let position_key = (
                (vertex.position.x / threshold).round() as i32,
                (vertex.position.y / threshold).round() as i32,
                (vertex.position.z / threshold).round() as i32,
            );

            if let Some(&existing_index) = vertex_map.get(&position_key) {
                index_map[i] = existing_index;
            } else {
                let new_index = new_vertices.len();
                new_vertices.push(*vertex);
                vertex_map.insert(position_key, new_index);
                index_map[i] = new_index;
            }
        }

        // Update triangle indices
        for triangle in &mut self.triangles {
            triangle.indices[0] = index_map[triangle.indices[0] as usize] as u32;
            triangle.indices[1] = index_map[triangle.indices[1] as usize] as u32;
            triangle.indices[2] = index_map[triangle.indices[2] as usize] as u32;
        }

        self.vertices = new_vertices;
        self.vertex_count = self.vertices.len();
        self.plane_equations = vec![Vec4::ZERO; self.triangles.len()];
        if self.poly_surface_types.len() != self.triangles.len() {
            self.poly_surface_types.resize(self.triangles.len(), 0);
        }
        self.poly_count = self.triangles.len();
        self.update_bounds();
        self.invalidate_cull_tree();
    }

    /// Remove degenerate triangles
    pub fn remove_degenerate_triangles(&mut self) {
        let mut new_triangles = Vec::new();
        let mut new_surface_types = Vec::new();
        let mut new_plane_equations = Vec::new();

        for (i, triangle) in self.triangles.iter().enumerate() {
            let v0 = self.vertices[triangle.indices[0] as usize];
            let v1 = self.vertices[triangle.indices[1] as usize];
            let v2 = self.vertices[triangle.indices[2] as usize];

            let edge1 = v1.position - v0.position;
            let edge2 = v2.position - v0.position;
            let cross = edge1.cross(edge2);

            if cross.length_squared() > EPSILON * EPSILON {
                new_triangles.push(*triangle);
                new_surface_types.push(self.poly_surface_types.get(i).copied().unwrap_or(0));
                new_plane_equations
                    .push(self.plane_equations.get(i).copied().unwrap_or(Vec4::ZERO));
            }
        }

        self.triangles = new_triangles;
        self.poly_surface_types = new_surface_types;
        self.plane_equations = new_plane_equations;
        self.poly_count = self.triangles.len();
        self.compute_plane_equations();
        self.invalidate_cull_tree();
    }

    /// Flip triangle winding order
    pub fn flip_winding(&mut self) {
        for triangle in &mut self.triangles {
            triangle.indices.swap(1, 2);
        }
        self.compute_plane_equations();
    }

    /// Generate wireframe representation
    pub fn generate_wireframe(&self) -> Vec<LineSegment> {
        let mut lines = Vec::new();
        let mut edge_set = std::collections::HashSet::new();

        for triangle in &self.triangles {
            let edges = [
                ((triangle.indices[0], triangle.indices[1])),
                ((triangle.indices[1], triangle.indices[2])),
                ((triangle.indices[2], triangle.indices[0])),
            ];

            for (i0, i1) in edges {
                let edge_key = if i0 < i1 { (i0, i1) } else { (i1, i0) };

                if edge_set.insert(edge_key) {
                    let v0 = self.vertices[i0 as usize].position;
                    let v1 = self.vertices[i1 as usize].position;
                    lines.push(LineSegment::new(v0, v1));
                }
            }
        }

        lines
    }
}

/// Mesh material structure
#[derive(Debug, Clone, PartialEq)]
pub struct MeshMaterial {
    pub name: String,
    pub diffuse_color: Vec3,
    pub specular_color: Vec3,
    pub shininess: f32,
    pub diffuse_texture: Option<String>,
    pub normal_texture: Option<String>,
    pub specular_texture: Option<String>,
}

impl MeshMaterial {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            diffuse_color: Vec3::new(0.8, 0.8, 0.8),
            specular_color: Vec3::new(1.0, 1.0, 1.0),
            shininess: 32.0,
            diffuse_texture: None,
            normal_texture: None,
            specular_texture: None,
        }
    }
}

/// Mesh statistics
#[derive(Debug, Clone)]
pub struct MeshStats {
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub material_count: usize,
    pub bounding_box_volume: f32,
    pub surface_area: f32,
}

impl MeshGeometry {
    pub fn get_stats(&self) -> MeshStats {
        let bounding_box_volume = self.bounding_box.extent.x
            * self.bounding_box.extent.y
            * self.bounding_box.extent.z
            * 8.0;

        let surface_area = self
            .triangles
            .iter()
            .map(|triangle| {
                let v0 = self.vertices[triangle.indices[0] as usize].position;
                let v1 = self.vertices[triangle.indices[1] as usize].position;
                let v2 = self.vertices[triangle.indices[2] as usize].position;

                let edge1 = v1 - v0;
                let edge2 = v2 - v0;
                edge1.cross(edge2).length() * 0.5
            })
            .sum();

        MeshStats {
            vertex_count: self.vertex_count(),
            triangle_count: self.triangle_count(),
            material_count: self.materials.len(),
            bounding_box_volume,
            surface_area,
        }
    }

    /// Process material run for rendering optimization
    pub fn process_material_run(&self) -> Vec<MaterialRun> {
        // Group triangles by material for efficient rendering
        let mut material_runs = Vec::new();
        let mut current_material = None;
        let mut run_start = 0;

        for (i, triangle) in self.triangles.iter().enumerate() {
            let material_index = triangle.material_index as usize;
            let material = if material_index < self.materials.len() {
                Some(&self.materials[material_index])
            } else {
                None
            };

            if current_material != material {
                // End previous run
                if let Some(mat) = current_material {
                    material_runs.push(MaterialRun {
                        material: mat.clone(),
                        start_triangle: run_start,
                        triangle_count: i - run_start,
                    });
                }

                // Start new run
                current_material = material;
                run_start = i;
            }
        }

        // End final run
        if let Some(mat) = current_material {
            material_runs.push(MaterialRun {
                material: mat.clone(),
                start_triangle: run_start,
                triangle_count: self.triangles.len() - run_start,
            });
        }

        material_runs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_creation() {
        let mut mesh = MeshGeometry::new();

        let v0 = MeshVertex::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec2::new(0.0, 0.0),
        );
        let v1 = MeshVertex::new(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec2::new(1.0, 0.0),
        );
        let v2 = MeshVertex::new(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec2::new(0.0, 1.0),
        );

        mesh.add_vertex(v0);
        mesh.add_vertex(v1);
        mesh.add_vertex(v2);

        let triangle = MeshTriangle::new(0, 1, 2, 0);
        mesh.add_triangle(triangle);

        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.triangle_count(), 1);
    }

    #[test]
    fn test_mesh_bounds() {
        let mut mesh = MeshGeometry::new();

        mesh.add_vertex(MeshVertex::new(
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::ZERO,
            Vec2::ZERO,
        ));
        mesh.add_vertex(MeshVertex::new(
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::ZERO,
            Vec2::ZERO,
        ));

        mesh.update_bounds();

        assert_eq!(mesh.bounding_box.center, Vec3::ZERO);
        assert_eq!(mesh.bounding_box.extent, Vec3::new(1.0, 1.0, 1.0));
        assert_eq!(mesh.bounding_sphere.center, Vec3::ZERO);
        assert!((mesh.bounding_sphere.radius - 1.732).abs() < 0.01); // sqrt(3) ≈ 1.732
    }

    #[test]
    fn test_normal_computation() {
        let mut mesh = MeshGeometry::new();

        // Create a simple quad
        mesh.add_vertex(MeshVertex::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::ZERO,
            Vec2::ZERO,
        ));
        mesh.add_vertex(MeshVertex::new(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::ZERO,
            Vec2::ZERO,
        ));
        mesh.add_vertex(MeshVertex::new(
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::ZERO,
            Vec2::ZERO,
        ));
        mesh.add_vertex(MeshVertex::new(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::ZERO,
            Vec2::ZERO,
        ));

        mesh.add_triangle(MeshTriangle::new(0, 1, 2, 0));
        mesh.add_triangle(MeshTriangle::new(0, 2, 3, 0));

        mesh.compute_normals();
        mesh.compute_plane_equations();

        // All vertices should have normal pointing up (0, 0, 1)
        for vertex in &mesh.vertices {
            assert!((vertex.normal - Vec3::new(0.0, 0.0, 1.0)).length() < EPSILON);
        }
    }

    #[test]
    fn test_weld_vertices() {
        let mut mesh = MeshGeometry::new();

        // Add vertices that are very close together
        mesh.add_vertex(MeshVertex::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::ZERO,
            Vec2::ZERO,
        ));
        mesh.add_vertex(MeshVertex::new(
            Vec3::new(0.001, 0.0, 0.0),
            Vec3::ZERO,
            Vec2::ZERO,
        ));
        mesh.add_vertex(MeshVertex::new(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::ZERO,
            Vec2::ZERO,
        ));

        mesh.add_triangle(MeshTriangle::new(0, 1, 2, 0));

        mesh.weld_vertices(0.01);

        assert_eq!(mesh.vertex_count(), 2); // Should weld first two vertices
        assert_eq!(mesh.triangles[0].indices, [0, 0, 1]); // Indices should be updated
    }

    #[test]
    fn test_cull_tree_invalidation_and_rebuild() {
        let mut mesh = MeshGeometry::new();

        mesh.add_vertex(MeshVertex::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::ZERO,
            Vec2::ZERO,
        ));
        mesh.add_vertex(MeshVertex::new(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::ZERO,
            Vec2::ZERO,
        ));
        mesh.add_vertex(MeshVertex::new(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::ZERO,
            Vec2::ZERO,
        ));
        mesh.add_triangle(MeshTriangle::new(0, 1, 2, 0));

        assert!(!mesh.has_cull_tree());
        let node_count = mesh.ensure_cull_tree().get_node_count();
        assert!(node_count > 0);

        mesh.add_vertex(MeshVertex::new(
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::ZERO,
            Vec2::ZERO,
        ));

        assert!(!mesh.has_cull_tree());
        let rebuilt = mesh.ensure_cull_tree();
        assert!(rebuilt.get_node_count() > 0);
    }
}
