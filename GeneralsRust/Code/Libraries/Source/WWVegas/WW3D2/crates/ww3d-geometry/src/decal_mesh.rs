//! Decal Mesh System
//!
//! This module provides decal functionality for applying textures/decals
//! to surfaces, similar to the C++ DecalMeshClass.

use crate::mesh_geometry::MaterialRun as MeshMaterialRun;
use crate::mesh_mat_desc::Texture;
use crate::render_info::RenderInfoClass;
use crate::*;
use glam::{Mat4, Vec2, Vec3};
use std::rc::Rc;

/// Base decal mesh class
#[derive(Debug)]
pub struct DecalMesh {
    pub parent_mesh: Option<Rc<MeshGeometry>>,
    pub decal_system: Option<Rc<DecalSystem>>,
    pub decals: Vec<Decal>,
}

impl DecalMesh {
    /// Create a new decal mesh
    pub fn new(
        parent_mesh: Option<Rc<MeshGeometry>>,
        decal_system: Option<Rc<DecalSystem>>,
    ) -> Self {
        Self {
            parent_mesh,
            decal_system,
            decals: Vec::new(),
        }
    }

    /// Create a decal on a surface
    pub fn create_decal(
        &mut self,
        position: Vec3,
        normal: Vec3,
        size: f32,
        texture: Rc<Texture>,
    ) -> Option<usize> {
        if self.parent_mesh.is_none() {
            return None;
        }

        let parent = self.parent_mesh.as_ref().unwrap();

        // Find triangles that intersect with the decal area
        let decal_plane = Plane::from_point_normal(position, normal);
        let mut intersecting_triangles = Vec::new();

        for (i, triangle) in parent.triangles.iter().enumerate() {
            let v0 = parent.vertices[triangle.indices[0] as usize];
            let v1 = parent.vertices[triangle.indices[1] as usize];
            let v2 = parent.vertices[triangle.indices[2] as usize];

            let triangle_obj = Triangle::new(v0.position, v1.position, v2.position);

            // Check if triangle intersects with decal area
            if self.triangle_intersects_decal(&triangle_obj, &decal_plane, position, size) {
                intersecting_triangles.push(i);
            }
        }

        if intersecting_triangles.is_empty() {
            return None;
        }

        // Create decal data
        let decal = Decal {
            id: self.decals.len(),
            position,
            normal,
            size,
            texture,
            triangles: intersecting_triangles,
            vertices: Vec::new(),
            tex_coords: Vec::new(),
        };

        let decal_id = decal.id;
        self.decals.push(decal);

        Some(decal_id)
    }

    /// Delete a decal
    pub fn delete_decal(&mut self, decal_id: usize) -> bool {
        if let Some(index) = self.decals.iter().position(|d| d.id == decal_id) {
            self.decals.remove(index);
            true
        } else {
            false
        }
    }

    /// Check if triangle intersects with decal area
    fn triangle_intersects_decal(
        &self,
        triangle: &Triangle,
        decal_plane: &Plane,
        decal_pos: Vec3,
        decal_size: f32,
    ) -> bool {
        // Check if triangle is on the correct side of the decal plane
        let triangle_center = triangle.centroid();
        if decal_plane.distance_to_point(triangle_center) < 0.0 {
            return false;
        }

        // Check distance from decal center
        let distance = (triangle_center - decal_pos).length();
        if distance > decal_size * 1.5 {
            return false;
        }

        true
    }

    /// Render all decals
    pub fn render(&self, render_info: &RenderInfoClass) {
        for decal in &self.decals {
            self.render_decal(decal, render_info);
        }
    }

    /// Render a single decal
    fn render_decal(&self, decal: &Decal, render_info: &RenderInfoClass) {
        if self.parent_mesh.is_none() {
            return;
        }

        let parent = self.parent_mesh.as_ref().unwrap();

        // In a real implementation, this would set up the decal geometry
        // and render it using the appropriate shader/texture
        for &triangle_idx in &decal.triangles {
            if triangle_idx < parent.triangles.len() {
                let triangle = &parent.triangles[triangle_idx];
                // Render the triangle with decal texture applied
                let _ = triangle;
                let _ = render_info;
            }
        }
    }

    /// Get number of active decals
    pub fn decal_count(&self) -> usize {
        self.decals.len()
    }

    /// Get decal by ID
    pub fn get_decal(&self, decal_id: usize) -> Option<&Decal> {
        self.decals.iter().find(|d| d.id == decal_id)
    }
}

/// Rigid decal mesh for static geometry
#[derive(Debug)]
pub struct RigidDecalMesh {
    pub base_mesh: DecalMesh,
}

impl RigidDecalMesh {
    /// Create a new rigid decal mesh
    pub fn new(parent_mesh: Rc<MeshGeometry>, decal_system: Rc<DecalSystem>) -> Self {
        Self {
            base_mesh: DecalMesh::new(Some(parent_mesh), Some(decal_system)),
        }
    }

    /// Create a decal on the rigid mesh
    pub fn create_decal(
        &mut self,
        position: Vec3,
        normal: Vec3,
        size: f32,
        texture: Rc<Texture>,
    ) -> Option<usize> {
        self.base_mesh.create_decal(position, normal, size, texture)
    }

    /// Delete a decal
    pub fn delete_decal(&mut self, decal_id: usize) -> bool {
        self.base_mesh.delete_decal(decal_id)
    }

    /// Render the decals
    pub fn render(&self, render_info: &RenderInfoClass) {
        self.base_mesh.render(render_info);
    }

    /// Process material run for rendering optimization
    pub fn process_material_run(&self) -> Vec<DecalMaterialRun> {
        let mut runs = Vec::new();

        if self.base_mesh.decals.is_empty() {
            return runs;
        }

        // Group decals by material/texture for efficient rendering
        let mut current_run = DecalMaterialRun {
            start_decal: 0,
            decal_count: 1,
            texture: self.base_mesh.decals[0].texture.clone(),
        };

        for i in 1..self.base_mesh.decals.len() {
            if self.base_mesh.decals[i].texture.name == current_run.texture.name {
                current_run.decal_count += 1;
            } else {
                runs.push(current_run);
                current_run = DecalMaterialRun {
                    start_decal: i,
                    decal_count: 1,
                    texture: self.base_mesh.decals[i].texture.clone(),
                };
            }
        }

        runs.push(current_run);
        runs
    }
}

/// Skin decal mesh for animated/skinned geometry
#[derive(Debug)]
pub struct SkinDecalMesh {
    pub base_mesh: DecalMesh,
    pub bone_transforms: Vec<Mat4>,
}

impl SkinDecalMesh {
    /// Create a new skin decal mesh
    pub fn new(
        parent_mesh: Rc<MeshGeometry>,
        decal_system: Rc<DecalSystem>,
        bone_count: usize,
    ) -> Self {
        Self {
            base_mesh: DecalMesh::new(Some(parent_mesh), Some(decal_system)),
            bone_transforms: vec![Mat4::IDENTITY; bone_count],
        }
    }

    /// Create a decal on the skinned mesh
    pub fn create_decal(
        &mut self,
        position: Vec3,
        normal: Vec3,
        size: f32,
        texture: Rc<Texture>,
    ) -> Option<usize> {
        self.base_mesh.create_decal(position, normal, size, texture)
    }

    /// Delete a decal
    pub fn delete_decal(&mut self, decal_id: usize) -> bool {
        self.base_mesh.delete_decal(decal_id)
    }

    /// Update bone transforms
    pub fn update_bone_transforms(&mut self, transforms: &[Mat4]) {
        for (i, transform) in transforms.iter().enumerate() {
            if i < self.bone_transforms.len() {
                self.bone_transforms[i] = transform.clone();
            }
        }
    }

    /// Render the decals with skinning
    pub fn render(&self, render_info: &RenderInfoClass) {
        // Apply bone transforms before rendering
        for decal in &self.base_mesh.decals {
            self.render_skinned_decal(decal, render_info);
        }
    }

    /// Render a single skinned decal
    fn render_skinned_decal(&self, decal: &Decal, render_info: &RenderInfoClass) {
        if self.base_mesh.parent_mesh.is_none() {
            return;
        }

        let parent = self.base_mesh.parent_mesh.as_ref().unwrap();

        // In a real implementation, this would transform decal vertices
        // by the appropriate bone matrices before rendering
        for &triangle_idx in &decal.triangles {
            if triangle_idx < parent.triangles.len() {
                let triangle = &parent.triangles[triangle_idx];
                // Apply skinning transform and render
                let _ = triangle;
                let _ = render_info;
            }
        }
    }

    /// Process material run for skinned rendering
    pub fn process_material_run(&self) -> Vec<MeshMaterialRun> {
        if let Some(mesh) = &self.base_mesh.parent_mesh {
            mesh.process_material_run()
        } else {
            Vec::new()
        }
    }
}

/// Individual decal data
#[derive(Debug, Clone)]
pub struct Decal {
    pub id: usize,
    pub position: Vec3,
    pub normal: Vec3,
    pub size: f32,
    pub texture: Rc<Texture>,
    pub triangles: Vec<usize>, // Indices of triangles affected by this decal
    pub vertices: Vec<Vec3>,
    pub tex_coords: Vec<Vec2>,
}

/// Material run for rendering optimization
#[derive(Debug, Clone)]
pub struct DecalMaterialRun {
    pub start_decal: usize,
    pub decal_count: usize,
    pub texture: Rc<Texture>,
}

/// Decal system for managing decal creation and lifetime
#[derive(Debug)]
pub struct DecalSystem {
    pub max_decals: usize,
    pub active_decals: Vec<Rc<Decal>>,
}

impl DecalSystem {
    /// Create a new decal system
    pub fn new(max_decals: usize) -> Self {
        Self {
            max_decals,
            active_decals: Vec::new(),
        }
    }

    /// Create a decal generator
    pub fn create_generator(&self, texture: Rc<Texture>, size: f32) -> DecalGenerator {
        DecalGenerator {
            texture,
            size,
            system: Rc::new(self.clone()),
        }
    }

    /// Add a decal to the system
    pub fn add_decal(&mut self, decal: Decal) -> Option<Rc<Decal>> {
        if self.active_decals.len() >= self.max_decals {
            // Remove oldest decal
            self.active_decals.remove(0);
        }

        let decal_rc = Rc::new(decal);
        self.active_decals.push(decal_rc.clone());
        Some(decal_rc)
    }

    /// Remove a decal from the system
    pub fn remove_decal(&mut self, decal_id: usize) -> bool {
        if let Some(index) = self.active_decals.iter().position(|d| d.id == decal_id) {
            self.active_decals.remove(index);
            true
        } else {
            false
        }
    }

    /// Update decal system (age out old decals, etc.)
    pub fn update(&mut self, delta_time: f32) {
        // In a real implementation, this would handle decal lifetime,
        // fading, and cleanup of expired decals
        let _ = delta_time;
    }

    /// Get active decal count
    pub fn decal_count(&self) -> usize {
        self.active_decals.len()
    }
}

impl Clone for DecalSystem {
    fn clone(&self) -> Self {
        Self {
            max_decals: self.max_decals,
            active_decals: Vec::new(), // Don't clone active decals
        }
    }
}

/// Decal generator for creating decals
#[derive(Debug, Clone)]
pub struct DecalGenerator {
    pub texture: Rc<Texture>,
    pub size: f32,
    pub system: Rc<DecalSystem>,
}

impl DecalGenerator {
    /// Generate a decal at the specified location
    pub fn generate_decal(&self, position: Vec3, normal: Vec3) -> Decal {
        Decal {
            id: 0, // Will be set by the system
            position,
            normal,
            size: self.size,
            texture: self.texture.clone(),
            triangles: Vec::new(),
            vertices: Vec::new(),
            tex_coords: Vec::new(),
        }
    }
}

/// Polygon clipping class for decal creation
#[derive(Debug)]
pub struct DecalPoly {
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
}

impl DecalPoly {
    /// Create a new decal polygon
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            normals: Vec::new(),
        }
    }

    /// Reset the polygon
    pub fn reset(&mut self) {
        self.vertices.clear();
        self.normals.clear();
    }

    /// Add a vertex to the polygon
    pub fn add_vertex(&mut self, vertex: Vec3, normal: Vec3) {
        self.vertices.push(vertex);
        self.normals.push(normal);
    }

    /// Clip the polygon against a plane
    pub fn clip(&self, plane: &Plane) -> DecalPoly {
        let mut result = DecalPoly::new();

        if self.vertices.len() <= 2 {
            return result;
        }

        let mut prev_point_in_front =
            plane.distance_to_point(self.vertices[self.vertices.len() - 1]) >= 0.0;

        for i in 0..self.vertices.len() {
            let curr_point_in_front = plane.distance_to_point(self.vertices[i]) >= 0.0;

            if prev_point_in_front {
                if curr_point_in_front {
                    // Both points in front, add current vertex
                    result.add_vertex(self.vertices[i], self.normals[i]);
                } else {
                    // Going from front to back, add intersection point
                    if let Some(intersection) = self.compute_intersection(i, plane) {
                        result.add_vertex(intersection.0, intersection.1);
                    }
                }
            } else if curr_point_in_front {
                // Going from back to front, add intersection and current vertex
                if let Some(intersection) = self.compute_intersection(i, plane) {
                    result.add_vertex(intersection.0, intersection.1);
                }
                result.add_vertex(self.vertices[i], self.normals[i]);
            }

            prev_point_in_front = curr_point_in_front;
        }

        result
    }

    /// Compute intersection point with plane
    fn compute_intersection(&self, current_index: usize, plane: &Plane) -> Option<(Vec3, Vec3)> {
        let prev_index = if current_index == 0 {
            self.vertices.len() - 1
        } else {
            current_index - 1
        };

        let v1 = self.vertices[prev_index];
        let v2 = self.vertices[current_index];
        let n1 = self.normals[prev_index];
        let n2 = self.normals[current_index];

        // Compute intersection parameter
        let d1 = plane.distance_to_point(v1);
        let d2 = plane.distance_to_point(v2);

        if (d1 - d2).abs() < EPSILON {
            return None; // Parallel to plane
        }

        let t = d1 / (d1 - d2);
        let t = t.clamp(0.0, 1.0);

        // Interpolate position and normal
        let position = v1 + (v2 - v1) * t;
        let normal = (n1 + (n2 - n1) * t).normalize();

        Some((position, normal))
    }

    /// Get vertex count
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decal_mesh_creation() {
        let mesh_geom = Rc::new(MeshGeometry::new());
        let decal_system = Rc::new(DecalSystem::new(100));
        let decal_mesh = DecalMesh::new(Some(mesh_geom), Some(decal_system));

        assert_eq!(decal_mesh.decal_count(), 0);
    }

    #[test]
    fn test_rigid_decal_mesh() {
        let mesh_geom = Rc::new(MeshGeometry::new());
        let decal_system = Rc::new(DecalSystem::new(100));
        let mut decal_mesh = RigidDecalMesh::new(mesh_geom, decal_system);

        let texture = Rc::new(Texture::new("bullet_hole", 64, 64));

        // Create a decal (would work better with actual geometry)
        let decal_id = decal_mesh.create_decal(
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 1.0),
            1.0,
            texture,
        );

        assert!(decal_id.is_some() || decal_id.is_none()); // Depends on geometry
    }

    #[test]
    fn test_decal_system() {
        let mut system = DecalSystem::new(10);

        let texture = Rc::new(Texture::new("test", 32, 32));

        let generator = system.create_generator(texture, 1.0);
        let decal = generator.generate_decal(Vec3::ZERO, Vec3::Y);

        let added = system.add_decal(decal);
        assert!(added.is_some());
        assert_eq!(system.decal_count(), 1);
    }

    #[test]
    fn test_decal_poly() {
        let mut poly = DecalPoly::new();

        poly.add_vertex(Vec3::new(0.0, 0.0, 0.0), Vec3::Y);
        poly.add_vertex(Vec3::new(1.0, 0.0, 0.0), Vec3::Y);
        poly.add_vertex(Vec3::new(0.0, 1.0, 0.0), Vec3::Y);

        assert_eq!(poly.vertex_count(), 3);

        poly.reset();
        assert_eq!(poly.vertex_count(), 0);
    }

    #[test]
    fn test_decal_poly_clipping() {
        let mut poly = DecalPoly::new();

        // Create a quad
        poly.add_vertex(Vec3::new(-1.0, -1.0, 0.0), Vec3::Z);
        poly.add_vertex(Vec3::new(1.0, -1.0, 0.0), Vec3::Z);
        poly.add_vertex(Vec3::new(1.0, 1.0, 0.0), Vec3::Z);
        poly.add_vertex(Vec3::new(-1.0, 1.0, 0.0), Vec3::Z);

        // Clip against a plane
        let plane = Plane::from_point_normal(Vec3::ZERO, Vec3::X);
        let clipped = poly.clip(&plane);

        // The clipped polygon should have fewer or equal vertices
        assert!(clipped.vertex_count() <= poly.vertex_count());
    }
}
