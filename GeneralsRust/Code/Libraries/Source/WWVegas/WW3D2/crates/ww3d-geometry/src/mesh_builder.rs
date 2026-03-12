//! Procedural Mesh Builder
//!
//! This module provides functionality for procedural mesh generation,
//! including primitives, complex shapes, and mesh construction utilities.

use crate::mesh_geometry::{MeshMaterial, MeshTriangle, MeshVertex};
use crate::*;
use glam::{Vec2, Vec3, Vec4};

/// Procedural mesh builder
#[derive(Debug, Clone)]
pub struct MeshBuilder {
    pub vertices: Vec<MeshVertex>,
    pub triangles: Vec<MeshTriangle>,
    pub materials: Vec<MeshMaterial>,
    pub current_material_index: u32,
}

impl MeshBuilder {
    /// Create a new mesh builder
    pub fn new() -> Self {
        let mut builder = Self {
            vertices: Vec::new(),
            triangles: Vec::new(),
            materials: Vec::new(),
            current_material_index: 0,
        };
        builder.materials.push(MeshMaterial::new("default"));
        builder
    }

    /// Set the current material for subsequent operations
    pub fn set_material(&mut self, material_index: u32) {
        self.ensure_material_slot(material_index as usize);
        self.current_material_index = material_index;
    }

    /// Add a vertex
    pub fn add_vertex(&mut self, position: Vec3, normal: Vec3, tex_coords: Vec2) -> u32 {
        let vertex = MeshVertex::new(position, normal, tex_coords);
        self.vertices.push(vertex);
        (self.vertices.len() - 1) as u32
    }

    /// Add a triangle
    pub fn add_triangle(&mut self, v0: u32, v1: u32, v2: u32) {
        self.ensure_material_slot(self.current_material_index as usize);
        let triangle = MeshTriangle::new(v0, v1, v2, self.current_material_index);
        self.triangles.push(triangle);
    }

    /// Add a quad (two triangles)
    pub fn add_quad(&mut self, v0: u32, v1: u32, v2: u32, v3: u32) {
        self.add_triangle(v0, v1, v2);
        self.add_triangle(v0, v2, v3);
    }

    /// Build and return the mesh geometry
    pub fn build(self) -> MeshGeometry {
        let mut mesh = MeshGeometry::new();
        mesh.vertices = self.vertices;
        mesh.triangles = self.triangles;
        mesh.materials = self.materials;
        mesh.vertex_count = mesh.vertices.len();
        mesh.poly_count = mesh.triangles.len();
        mesh.poly_surface_types = vec![0; mesh.triangles.len()];
        mesh.plane_equations = vec![Vec4::ZERO; mesh.triangles.len()];
        mesh.update_bounds();
        mesh.compute_normals();
        mesh.compute_plane_equations();
        mesh
    }

    /// Append a material and return the index assigned to it.
    pub fn add_material(&mut self, material: MeshMaterial) -> u32 {
        let index = self.materials.len() as u32;
        self.materials.push(material);
        index
    }

    /// Borrow the currently selected material mutably.
    pub fn current_material_mut(&mut self) -> &mut MeshMaterial {
        let index = self.current_material_index as usize;
        self.ensure_material_slot(index);
        &mut self.materials[index]
    }

    /// Borrow a specific material mutably, expanding the list if required.
    pub fn material_mut(&mut self, index: u32) -> Option<&mut MeshMaterial> {
        self.ensure_material_slot(index as usize);
        self.materials.get_mut(index as usize)
    }

    fn ensure_material_slot(&mut self, index: usize) {
        if index >= self.materials.len() {
            let missing = index + 1 - self.materials.len();
            for i in 0..missing {
                let name = format!("material_{}", self.materials.len() + i);
                self.materials.push(MeshMaterial::new(&name));
            }
        }
    }

    /// Create a cube mesh
    pub fn create_cube(size: f32) -> MeshGeometry {
        let mut builder = Self::new();
        {
            let material = builder.current_material_mut();
            material.name = "cube".to_string();
            material.diffuse_color = Vec3::splat(0.8);
            material.specular_color = Vec3::splat(0.25);
            material.shininess = 32.0;
        }
        let half_size = size / 2.0;

        // Define vertices
        let positions = [
            Vec3::new(-half_size, -half_size, -half_size), // 0: left-bottom-back
            Vec3::new(half_size, -half_size, -half_size),  // 1: right-bottom-back
            Vec3::new(half_size, half_size, -half_size),   // 2: right-top-back
            Vec3::new(-half_size, half_size, -half_size),  // 3: left-top-back
            Vec3::new(-half_size, -half_size, half_size),  // 4: left-bottom-front
            Vec3::new(half_size, -half_size, half_size),   // 5: right-bottom-front
            Vec3::new(half_size, half_size, half_size),    // 6: right-top-front
            Vec3::new(-half_size, half_size, half_size),   // 7: left-top-front
        ];

        // Add vertices with normals and texture coordinates
        let vertex_indices = [
            builder.add_vertex(positions[0], Vec3::new(-1.0, 0.0, 0.0), Vec2::new(0.0, 0.0)), // Left
            builder.add_vertex(positions[1], Vec3::new(1.0, 0.0, 0.0), Vec2::new(1.0, 0.0)), // Right
            builder.add_vertex(positions[2], Vec3::new(0.0, 1.0, 0.0), Vec2::new(1.0, 1.0)), // Top
            builder.add_vertex(positions[3], Vec3::new(0.0, -1.0, 0.0), Vec2::new(0.0, 1.0)), // Bottom
            builder.add_vertex(positions[4], Vec3::new(0.0, 0.0, -1.0), Vec2::new(0.0, 0.0)), // Back
            builder.add_vertex(positions[5], Vec3::new(0.0, 0.0, 1.0), Vec2::new(1.0, 0.0)), // Front
            builder.add_vertex(positions[6], Vec3::new(0.0, 0.0, 1.0), Vec2::new(1.0, 1.0)), // Front
            builder.add_vertex(positions[7], Vec3::new(0.0, 0.0, -1.0), Vec2::new(0.0, 1.0)), // Back
        ];

        // Front face
        builder.add_quad(
            vertex_indices[4],
            vertex_indices[5],
            vertex_indices[6],
            vertex_indices[7],
        );
        // Back face
        builder.add_quad(
            vertex_indices[1],
            vertex_indices[0],
            vertex_indices[3],
            vertex_indices[2],
        );
        // Left face
        builder.add_quad(
            vertex_indices[0],
            vertex_indices[4],
            vertex_indices[7],
            vertex_indices[3],
        );
        // Right face
        builder.add_quad(
            vertex_indices[5],
            vertex_indices[1],
            vertex_indices[2],
            vertex_indices[6],
        );
        // Top face
        builder.add_quad(
            vertex_indices[7],
            vertex_indices[6],
            vertex_indices[2],
            vertex_indices[3],
        );
        // Bottom face
        builder.add_quad(
            vertex_indices[4],
            vertex_indices[0],
            vertex_indices[1],
            vertex_indices[5],
        );

        builder.build()
    }

    /// Create a sphere mesh
    pub fn create_sphere(radius: f32, stacks: usize, slices: usize) -> MeshGeometry {
        let mut builder = Self::new();
        {
            let material = builder.current_material_mut();
            material.name = "sphere".to_string();
            material.diffuse_color = Vec3::new(0.7, 0.75, 0.85);
            material.specular_color = Vec3::splat(0.4);
            material.shininess = 48.0;
        }

        // Generate vertices
        for i in 0..=stacks {
            let phi = PI * i as f32 / stacks as f32;
            for j in 0..=slices {
                let theta = 2.0 * PI * j as f32 / slices as f32;

                let x = radius * phi.sin() * theta.cos();
                let y = radius * phi.cos();
                let z = radius * phi.sin() * theta.sin();

                let position = Vec3::new(x, y, z);
                let normal = position.normalize();
                let tex_coords = Vec2::new(j as f32 / slices as f32, i as f32 / stacks as f32);

                builder.add_vertex(position, normal, tex_coords);
            }
        }

        // Generate triangles
        for i in 0..stacks {
            for j in 0..slices {
                let first = (i * (slices + 1) + j) as u32;
                let second = first + slices as u32 + 1;

                builder.add_triangle(first, second, first + 1);
                builder.add_triangle(second, second + 1, first + 1);
            }
        }

        builder.build()
    }

    /// Create a cylinder mesh
    pub fn create_cylinder(radius: f32, height: f32, slices: usize) -> MeshGeometry {
        let mut builder = Self::new();
        {
            let material = builder.current_material_mut();
            material.name = "cylinder".to_string();
            material.diffuse_color = Vec3::new(0.75, 0.7, 0.65);
            material.specular_color = Vec3::splat(0.35);
            material.shininess = 24.0;
        }
        let half_height = height / 2.0;

        // Generate side vertices
        for i in 0..=slices {
            let angle = 2.0 * PI * i as f32 / slices as f32;
            let x = radius * angle.cos();
            let z = radius * angle.sin();

            // Bottom vertex
            let bottom_pos = Vec3::new(x, -half_height, z);
            let side_normal = Vec3::new(x, 0.0, z).normalize();
            builder.add_vertex(
                bottom_pos,
                side_normal,
                Vec2::new(i as f32 / slices as f32, 0.0),
            );

            // Top vertex
            let top_pos = Vec3::new(x, half_height, z);
            builder.add_vertex(
                top_pos,
                side_normal,
                Vec2::new(i as f32 / slices as f32, 1.0),
            );
        }

        // Generate center vertices for caps
        let bottom_center = builder.add_vertex(
            Vec3::new(0.0, -half_height, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Vec2::new(0.5, 0.5),
        );
        let top_center = builder.add_vertex(
            Vec3::new(0.0, half_height, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec2::new(0.5, 0.5),
        );

        // Generate side triangles
        for i in 0..slices {
            let bottom1 = (i * 2) as u32;
            let top1 = bottom1 + 1;
            let bottom2 = ((i + 1) % slices * 2) as u32;
            let top2 = bottom2 + 1;

            builder.add_quad(bottom1, bottom2, top2, top1);
        }

        // Generate cap triangles
        for i in 0..slices {
            let bottom1 = (i * 2) as u32;
            let bottom2 = ((i + 1) % slices * 2) as u32;

            builder.add_triangle(bottom_center, bottom1, bottom2);

            let top1 = bottom1 + 1;
            let top2 = bottom2 + 1;

            builder.add_triangle(top_center, top2, top1);
        }

        builder.build()
    }

    /// Create a plane mesh
    pub fn create_plane(width: f32, height: f32, subdivisions: usize) -> MeshGeometry {
        let mut builder = Self::new();
        {
            let material = builder.current_material_mut();
            material.name = "plane".to_string();
            material.diffuse_color = Vec3::new(0.6, 0.65, 0.7);
            material.specular_color = Vec3::splat(0.15);
            material.shininess = 12.0;
        }
        let half_width = width / 2.0;
        let half_height = height / 2.0;

        // Generate vertices
        for i in 0..=subdivisions {
            for j in 0..=subdivisions {
                let x = -half_width + width * i as f32 / subdivisions as f32;
                let z = -half_height + height * j as f32 / subdivisions as f32;
                let y = 0.0;

                let position = Vec3::new(x, y, z);
                let normal = Vec3::new(0.0, 1.0, 0.0);
                let tex_coords = Vec2::new(
                    i as f32 / subdivisions as f32,
                    j as f32 / subdivisions as f32,
                );

                builder.add_vertex(position, normal, tex_coords);
            }
        }

        // Generate triangles
        for i in 0..subdivisions {
            for j in 0..subdivisions {
                let top_left = (i * (subdivisions + 1) + j) as u32;
                let top_right = top_left + 1;
                let bottom_left = ((i + 1) * (subdivisions + 1) + j) as u32;
                let bottom_right = bottom_left + 1;

                builder.add_triangle(top_left, bottom_left, top_right);
                builder.add_triangle(top_right, bottom_left, bottom_right);
            }
        }

        builder.build()
    }

    /// Create a torus (donut) mesh
    pub fn create_torus(
        outer_radius: f32,
        inner_radius: f32,
        outer_segments: usize,
        inner_segments: usize,
    ) -> MeshGeometry {
        let mut builder = Self::new();
        {
            let material = builder.current_material_mut();
            material.name = "torus".to_string();
            material.diffuse_color = Vec3::new(0.85, 0.6, 0.4);
            material.specular_color = Vec3::splat(0.3);
            material.shininess = 40.0;
        }

        // Generate vertices
        for i in 0..outer_segments {
            let outer_angle = 2.0 * PI * i as f32 / outer_segments as f32;
            let cos_outer = outer_angle.cos();
            let sin_outer = outer_angle.sin();

            for j in 0..inner_segments {
                let inner_angle = 2.0 * PI * j as f32 / inner_segments as f32;
                let cos_inner = inner_angle.cos();
                let sin_inner = inner_angle.sin();

                let x = (outer_radius + inner_radius * cos_inner) * cos_outer;
                let y = inner_radius * sin_inner;
                let z = (outer_radius + inner_radius * cos_inner) * sin_outer;

                let position = Vec3::new(x, y, z);

                // Calculate normal
                let center_x = outer_radius * cos_outer;
                let center_z = outer_radius * sin_outer;
                let normal = Vec3::new(x - center_x, y, z - center_z).normalize();

                let tex_coords = Vec2::new(
                    i as f32 / outer_segments as f32,
                    j as f32 / inner_segments as f32,
                );

                builder.add_vertex(position, normal, tex_coords);
            }
        }

        // Generate triangles
        for i in 0..outer_segments {
            for j in 0..inner_segments {
                let current = (i * inner_segments + j) as u32;
                let next_i = ((i + 1) % outer_segments * inner_segments + j) as u32;
                let next_j = (i * inner_segments + (j + 1) % inner_segments) as u32;
                let next_both =
                    ((i + 1) % outer_segments * inner_segments + (j + 1) % inner_segments) as u32;

                builder.add_triangle(current, next_i, next_j);
                builder.add_triangle(next_j, next_i, next_both);
            }
        }

        builder.build()
    }

    /// Extrude a 2D shape along a path
    pub fn extrude_along_path(shape: &[Vec2], path: &[Vec3], up_vector: Vec3) -> MeshGeometry {
        let mut builder = Self::new();
        {
            let material = builder.current_material_mut();
            material.name = "extrusion".to_string();
            material.diffuse_color = Vec3::splat(0.9);
            material.specular_color = Vec3::splat(0.2);
        }
        let shape_len = shape.len();
        let path_len = path.len();

        // Generate vertices
        for i in 0..path_len {
            let position = path[i];
            let direction = if i < path_len - 1 {
                (path[i + 1] - position).normalize()
            } else if i > 0 {
                (position - path[i - 1]).normalize()
            } else {
                Vec3::new(0.0, 1.0, 0.0)
            };

            let right = direction.cross(up_vector).normalize();
            let up = right.cross(direction).normalize();

            for j in 0..shape_len {
                let local_pos = shape[j];
                let world_pos = position + right * local_pos.x + up * local_pos.y;

                // Calculate normal (simplified)
                let normal = up;

                let tex_coords = Vec2::new(j as f32 / shape_len as f32, i as f32 / path_len as f32);

                builder.add_vertex(world_pos, normal, tex_coords);
            }
        }

        // Generate triangles
        for i in 0..path_len - 1 {
            for j in 0..shape_len {
                let current = (i * shape_len + j) as u32;
                let next_j = (i * shape_len + (j + 1) % shape_len) as u32;
                let next_i = ((i + 1) * shape_len + j) as u32;
                let next_both = ((i + 1) * shape_len + (j + 1) % shape_len) as u32;

                builder.add_triangle(current, next_i, next_j);
                builder.add_triangle(next_j, next_i, next_both);
            }
        }

        builder.build()
    }

    /// Create a procedural terrain mesh
    pub fn create_terrain(
        width: f32,
        height: f32,
        resolution: usize,
        height_function: impl Fn(f32, f32) -> f32,
    ) -> MeshGeometry {
        let mut builder = Self::new();
        let half_width = width / 2.0;
        let half_height = height / 2.0;

        // Generate vertices
        for i in 0..=resolution {
            for j in 0..=resolution {
                let x = -half_width + width * i as f32 / resolution as f32;
                let z = -half_height + height * j as f32 / resolution as f32;
                let y = height_function(x, z);

                let position = Vec3::new(x, y, z);
                let normal = Vec3::new(0.0, 1.0, 0.0); // Simplified normal
                let tex_coords =
                    Vec2::new(i as f32 / resolution as f32, j as f32 / resolution as f32);

                builder.add_vertex(position, normal, tex_coords);
            }
        }

        // Generate triangles
        for i in 0..resolution {
            for j in 0..resolution {
                let top_left = (i * (resolution + 1) + j) as u32;
                let top_right = top_left + 1;
                let bottom_left = ((i + 1) * (resolution + 1) + j) as u32;
                let bottom_right = bottom_left + 1;

                builder.add_triangle(top_left, bottom_left, top_right);
                builder.add_triangle(top_right, bottom_left, bottom_right);
            }
        }

        let mut mesh = builder.build();
        mesh.compute_normals(); // Compute proper normals for terrain
        mesh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cube_creation() {
        let cube = MeshBuilder::create_cube(2.0);
        assert_eq!(cube.vertex_count(), 8); // 8 vertices for a cube
        assert_eq!(cube.triangle_count(), 12); // 6 faces * 2 triangles
    }

    #[test]
    fn test_sphere_creation() {
        let sphere = MeshBuilder::create_sphere(1.0, 8, 8);
        assert!(sphere.vertex_count() > 0);
        assert!(sphere.triangle_count() > 0);
    }

    #[test]
    fn test_plane_creation() {
        let plane = MeshBuilder::create_plane(10.0, 10.0, 4);
        assert_eq!(plane.vertex_count(), 25); // 5x5 grid
        assert_eq!(plane.triangle_count(), 32); // (4x4) * 2 triangles per quad
    }

    #[test]
    fn test_mesh_builder_basic() {
        let mut builder = MeshBuilder::new();

        let v0 = builder.add_vertex(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec2::ZERO,
        );
        let v1 = builder.add_vertex(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec2::new(1.0, 0.0),
        );
        let v2 = builder.add_vertex(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec2::new(0.0, 1.0),
        );

        builder.add_triangle(v0, v1, v2);

        let mesh = builder.build();
        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.triangle_count(), 1);
    }
}
