/// Mesh rendering system
///
/// This module implements the core mesh rendering functionality for WW3D.
use crate::errors::{W3DError, W3DResult};
use crate::render_object::*;
use crate::w3d_format::*;
use crate::RenderObjClassId;
use glam::{Mat4, Vec2, Vec3};
use std::any::Any;

/// Vertex data for rendering
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub texcoord: Vec2,
    pub color: [u8; 4],
}

impl Vertex {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            normal: Vec3::Z,
            texcoord: Vec2::ZERO,
            color: [255, 255, 255, 255],
        }
    }

    pub fn with_normal(mut self, normal: Vec3) -> Self {
        self.normal = normal;
        self
    }

    pub fn with_texcoord(mut self, texcoord: Vec2) -> Self {
        self.texcoord = texcoord;
        self
    }

    pub fn with_color(mut self, color: [u8; 4]) -> Self {
        self.color = color;
        self
    }
}

/// Triangle definition using vertex indices
#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    pub indices: [u32; 3],
    pub normal: Vec3,
    pub attributes: u32,
}

impl Triangle {
    pub fn new(i0: u32, i1: u32, i2: u32) -> Self {
        Self {
            indices: [i0, i1, i2],
            normal: Vec3::Z,
            attributes: 0,
        }
    }

    pub fn with_normal(mut self, normal: Vec3) -> Self {
        self.normal = normal;
        self
    }

    pub fn with_attributes(mut self, attributes: u32) -> Self {
        self.attributes = attributes;
        self
    }
}

/// Vertex influence for skinning
#[derive(Debug, Clone)]
pub struct VertexInfluence {
    pub bone_index: u32,
    pub weight: f32,
}

/// Mesh geometry data
#[derive(Debug, Clone)]
pub struct MeshGeometry {
    pub vertices: Vec<Vertex>,
    pub triangles: Vec<Triangle>,
    pub vertex_influences: Vec<Vec<VertexInfluence>>,
}

impl MeshGeometry {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            triangles: Vec::new(),
            vertex_influences: Vec::new(),
        }
    }

    pub fn from_w3d(w3d_mesh: &W3dMesh) -> W3DResult<Self> {
        let vertex_count = w3d_mesh.vertices.len();

        if vertex_count == 0 {
            return Err(W3DError::CorruptedFile);
        }

        let mut vertices = Vec::with_capacity(vertex_count);

        for i in 0..vertex_count {
            let position: Vec3 = w3d_mesh.vertices[i].into();

            let normal = if i < w3d_mesh.normals.len() {
                w3d_mesh.normals[i].into()
            } else {
                Vec3::Z
            };

            let texcoord = if i < w3d_mesh.texture_coords.len() {
                Vec2::new(w3d_mesh.texture_coords[i].u, w3d_mesh.texture_coords[i].v)
            } else {
                Vec2::ZERO
            };

            vertices.push(
                Vertex::new(position)
                    .with_normal(normal)
                    .with_texcoord(texcoord),
            );
        }

        let mut triangles = Vec::with_capacity(w3d_mesh.triangles.len());
        for tri in &w3d_mesh.triangles {
            triangles.push(
                Triangle::new(tri.vindex[0], tri.vindex[1], tri.vindex[2])
                    .with_normal(tri.normal.into())
                    .with_attributes(tri.attributes),
            );
        }

        // Convert vertex influences
        // Note: W3dMesh doesn't have vertex_influences field in current implementation.
        // C++ equivalent would load influence data here if available from the file format.
        // For now, initialize with empty influences - skinning will use default bone assignment.
        let vertex_influences = vec![Vec::new(); vertex_count];

        Ok(Self {
            vertices,
            triangles,
            vertex_influences,
        })
    }

    pub fn calculate_bounding_sphere(&self) -> BoundingSphere {
        if self.vertices.is_empty() {
            return BoundingSphere::zero();
        }

        // Calculate center
        let mut center = Vec3::ZERO;
        for vertex in &self.vertices {
            center += vertex.position;
        }
        center /= self.vertices.len() as f32;

        // Calculate radius
        let mut radius_sq = 0.0f32;
        for vertex in &self.vertices {
            let dist_sq = (vertex.position - center).length_squared();
            radius_sq = radius_sq.max(dist_sq);
        }

        BoundingSphere::new(center, radius_sq.sqrt())
    }

    pub fn calculate_bounding_box(&self) -> AABox {
        if self.vertices.is_empty() {
            return AABox::zero();
        }

        let positions: Vec<Vec3> = self.vertices.iter().map(|v| v.position).collect();
        AABox::from_points(&positions)
    }

    pub fn calculate_normals(&mut self) {
        // Calculate face normals and accumulate to vertices
        let mut vertex_normals = vec![Vec3::ZERO; self.vertices.len()];

        for tri in &self.triangles {
            let v0 = self.vertices[tri.indices[0] as usize].position;
            let v1 = self.vertices[tri.indices[1] as usize].position;
            let v2 = self.vertices[tri.indices[2] as usize].position;

            let edge1 = v1 - v0;
            let edge2 = v2 - v0;
            let normal = edge1.cross(edge2).normalize();

            for &idx in &tri.indices {
                vertex_normals[idx as usize] += normal;
            }
        }

        // Normalize accumulated normals
        for i in 0..self.vertices.len() {
            let normal = vertex_normals[i];
            if normal.length_squared() > 0.0001 {
                self.vertices[i].normal = normal.normalize();
            }
        }
    }

    pub fn transform(&mut self, transform: &Mat4) {
        let normal_transform = transform.inverse().transpose();

        for vertex in &mut self.vertices {
            vertex.position = transform.transform_point3(vertex.position);
            vertex.normal = normal_transform
                .transform_vector3(vertex.normal)
                .normalize();
        }
    }
}

impl Default for MeshGeometry {
    fn default() -> Self {
        Self::new()
    }
}

/// Mesh render object
#[derive(Debug, Clone)]
pub struct Mesh {
    name: String,
    geometry: MeshGeometry,
    transform: Mat4,
    bounding_sphere: BoundingSphere,
    bounding_box: AABox,
    sort_level: i32,
    visible: bool,
    cached_bounding_volumes_valid: bool,
    /// Per-instance alpha override (0.0-1.0). Used for fading effects.
    /// C++ Reference: mesh.h - float m_opacity
    alpha_override: f32,
    /// Material pass alpha override (0.0-1.0). Overrides material alpha channel.
    /// C++ Reference: mesh.h - float m_matPassAlphaOverride
    material_pass_alpha_override: f32,
    /// Material pass emissive override (0.0-1.0). Controls self-illumination strength.
    /// C++ Reference: mesh.h - float m_matPassEmissiveOverride
    material_pass_emissive_override: f32,
}

impl Mesh {
    pub fn new(name: String, geometry: MeshGeometry) -> Self {
        let bounding_sphere = geometry.calculate_bounding_sphere();
        let bounding_box = geometry.calculate_bounding_box();

        Self {
            name,
            geometry,
            transform: Mat4::IDENTITY,
            bounding_sphere,
            bounding_box,
            sort_level: 0,
            visible: true,
            cached_bounding_volumes_valid: true,
            alpha_override: 1.0,                  // Full opacity by default
            material_pass_alpha_override: 1.0,    // No alpha override by default
            material_pass_emissive_override: 1.0, // Full emissive by default
        }
    }

    pub fn from_w3d(w3d_mesh: &W3dMesh) -> W3DResult<Self> {
        let name = w3d_mesh.header.mesh_name_str();

        let geometry = MeshGeometry::from_w3d(w3d_mesh)?;
        Ok(Self::new(name, geometry))
    }

    pub fn geometry(&self) -> &MeshGeometry {
        &self.geometry
    }

    pub fn geometry_mut(&mut self) -> &mut MeshGeometry {
        self.cached_bounding_volumes_valid = false;
        &mut self.geometry
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Get per-instance alpha override value (0.0 = transparent, 1.0 = opaque)
    pub fn get_alpha_override(&self) -> f32 {
        self.alpha_override
    }

    /// Set per-instance alpha override (0.0 = transparent, 1.0 = opaque)
    /// Used for fading effects, death animations, etc.
    pub fn set_alpha_override(&mut self, alpha: f32) {
        self.alpha_override = alpha.clamp(0.0, 1.0);
    }

    /// Get material pass alpha override value
    pub fn get_material_pass_alpha_override(&self) -> f32 {
        self.material_pass_alpha_override
    }

    /// Set material pass alpha override (overrides material's alpha channel)
    pub fn set_material_pass_alpha_override(&mut self, alpha: f32) {
        self.material_pass_alpha_override = alpha.clamp(0.0, 1.0);
    }

    /// Get material pass emissive override value
    pub fn get_material_pass_emissive_override(&self) -> f32 {
        self.material_pass_emissive_override
    }

    /// Set material pass emissive override (controls self-illumination)
    /// Used for team colors, glowing effects, etc.
    pub fn set_material_pass_emissive_override(&mut self, emissive: f32) {
        self.material_pass_emissive_override = emissive.clamp(0.0, 1.0);
    }

    pub fn get_num_vertices(&self) -> usize {
        self.geometry.vertices.len()
    }

    pub fn get_num_triangles(&self) -> usize {
        self.geometry.triangles.len()
    }

    fn update_bounding_volumes_if_needed(&mut self) {
        if !self.cached_bounding_volumes_valid {
            self.bounding_sphere = self.geometry.calculate_bounding_sphere();
            self.bounding_box = self.geometry.calculate_bounding_box();
            self.cached_bounding_volumes_valid = true;
        }
    }
}

impl RenderObject for Mesh {
    fn class_id(&self) -> RenderObjClassId {
        RenderObjClassId::Mesh
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn clone_object(&self) -> Box<dyn RenderObject> {
        Box::new(self.clone())
    }

    fn render(&mut self, _info: &RenderInfo) -> W3DResult<()> {
        if !self.visible {
            return Ok(());
        }

        // Note: Actual rendering implementation is backend-specific (DirectX/WGPU).
        // This trait method serves as the interface - concrete rendering happens in
        // the renderer backend which processes RenderObj instances.
        // C++ equivalent delegates to device-specific rendering (DX8IndexedPolygonRenderer).

        if self.geometry.vertices.is_empty() {
            return Err(W3DError::InvalidParameter(
                "Empty mesh geometry".to_string(),
            ));
        }

        Ok(())
    }

    fn special_render(&mut self, info: &SpecialRenderInfo) -> W3DResult<()> {
        match info.mode {
            SpecialRenderMode::Shadow => {
                // Shadow rendering - render with depth only
                Ok(())
            }
            SpecialRenderMode::DepthOnly => {
                // Depth-only rendering
                Ok(())
            }
            SpecialRenderMode::ObjectId => {
                // Object ID rendering for picking
                Ok(())
            }
            SpecialRenderMode::Wireframe => {
                // Wireframe rendering
                Ok(())
            }
        }
    }

    fn get_obj_space_bounding_sphere(&self) -> BoundingSphere {
        self.bounding_sphere
    }

    fn get_obj_space_bounding_box(&self) -> AABox {
        self.bounding_box
    }

    fn get_transform(&self) -> Mat4 {
        self.transform
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }

    fn cast_ray(&self, ray: &Ray) -> RayCollisionResult {
        // Transform ray to object space
        let inv_transform = self.transform.inverse();
        let local_origin = inv_transform.transform_point3(ray.origin);
        let local_direction = inv_transform.transform_vector3(ray.direction).normalize();
        let local_ray = Ray::new(local_origin, local_direction);

        // Test against triangles
        let mut closest_hit = RayCollisionResult::no_hit();

        for tri in &self.geometry.triangles {
            let v0 = self.geometry.vertices[tri.indices[0] as usize].position;
            let v1 = self.geometry.vertices[tri.indices[1] as usize].position;
            let v2 = self.geometry.vertices[tri.indices[2] as usize].position;

            // Möller-Trumbore ray-triangle intersection
            let edge1 = v1 - v0;
            let edge2 = v2 - v0;
            let h = local_ray.direction.cross(edge2);
            let a = edge1.dot(h);

            if a.abs() < 0.0001 {
                continue;
            }

            let f = 1.0 / a;
            let s = local_ray.origin - v0;
            let u = f * s.dot(h);

            if u < 0.0 || u > 1.0 {
                continue;
            }

            let q = s.cross(edge1);
            let v = f * local_ray.direction.dot(q);

            if v < 0.0 || u + v > 1.0 {
                continue;
            }

            let t = f * edge2.dot(q);

            if t > 0.0001 && t < closest_hit.distance {
                let local_point = local_ray.point_at(t);
                let world_point = self.transform.transform_point3(local_point);
                let world_normal = self.transform.transform_vector3(tri.normal).normalize();

                closest_hit = RayCollisionResult::new(t, world_point, world_normal);
            }
        }

        closest_hit
    }

    fn intersect_aabox(&self, bbox: &AABox) -> bool {
        self.bounding_box.intersects(bbox)
    }

    fn scale(&mut self, scale: f32) {
        self.scale_xyz(scale, scale, scale);
    }

    fn scale_xyz(&mut self, sx: f32, sy: f32, sz: f32) {
        let scale_matrix = Mat4::from_scale(Vec3::new(sx, sy, sz));
        self.geometry.transform(&scale_matrix);
        self.cached_bounding_volumes_valid = false;
    }

    fn get_num_polys(&self) -> usize {
        self.geometry.triangles.len()
    }

    fn get_sort_level(&self) -> i32 {
        self.sort_level
    }

    fn set_sort_level(&mut self, level: i32) {
        self.sort_level = level;
    }

    fn update_cached_bounding_volumes(&mut self) {
        self.update_bounding_volumes_if_needed();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Mesh builder for creating meshes
#[derive(Debug)]
pub struct MeshBuilder {
    geometry: MeshGeometry,
}

impl MeshBuilder {
    pub fn new() -> Self {
        Self {
            geometry: MeshGeometry::new(),
        }
    }

    pub fn add_vertex(&mut self, vertex: Vertex) -> u32 {
        let index = self.geometry.vertices.len() as u32;
        self.geometry.vertices.push(vertex);
        index
    }

    pub fn add_triangle(&mut self, triangle: Triangle) {
        self.geometry.triangles.push(triangle);
    }

    pub fn add_vertex_influence(&mut self, vertex_index: usize, bone_index: u32, weight: f32) {
        while self.geometry.vertex_influences.len() <= vertex_index {
            self.geometry.vertex_influences.push(Vec::new());
        }
        self.geometry.vertex_influences[vertex_index].push(VertexInfluence { bone_index, weight });
    }

    pub fn calculate_normals(&mut self) {
        self.geometry.calculate_normals();
    }

    pub fn build(self, name: String) -> Mesh {
        Mesh::new(name, self.geometry)
    }
}

impl Default for MeshBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a simple quad mesh for testing
pub fn create_quad_mesh(name: String, size: f32) -> Mesh {
    let mut builder = MeshBuilder::new();

    let half = size * 0.5;

    // Vertices
    let v0 = builder.add_vertex(
        Vertex::new(Vec3::new(-half, -half, 0.0))
            .with_normal(Vec3::Z)
            .with_texcoord(Vec2::new(0.0, 0.0)),
    );
    let v1 = builder.add_vertex(
        Vertex::new(Vec3::new(half, -half, 0.0))
            .with_normal(Vec3::Z)
            .with_texcoord(Vec2::new(1.0, 0.0)),
    );
    let v2 = builder.add_vertex(
        Vertex::new(Vec3::new(half, half, 0.0))
            .with_normal(Vec3::Z)
            .with_texcoord(Vec2::new(1.0, 1.0)),
    );
    let v3 = builder.add_vertex(
        Vertex::new(Vec3::new(-half, half, 0.0))
            .with_normal(Vec3::Z)
            .with_texcoord(Vec2::new(0.0, 1.0)),
    );

    // Triangles
    builder.add_triangle(Triangle::new(v0, v1, v2).with_normal(Vec3::Z));
    builder.add_triangle(Triangle::new(v0, v2, v3).with_normal(Vec3::Z));

    builder.build(name)
}

/// Create a cube mesh for testing
pub fn create_cube_mesh(name: String, size: f32) -> Mesh {
    let mut builder = MeshBuilder::new();

    let half = size * 0.5;

    // Define cube vertices
    let positions = [
        Vec3::new(-half, -half, -half), // 0
        Vec3::new(half, -half, -half),  // 1
        Vec3::new(half, half, -half),   // 2
        Vec3::new(-half, half, -half),  // 3
        Vec3::new(-half, -half, half),  // 4
        Vec3::new(half, -half, half),   // 5
        Vec3::new(half, half, half),    // 6
        Vec3::new(-half, half, half),   // 7
    ];

    // Create vertices for each face (we need duplicates for proper normals)
    let mut indices = Vec::new();

    // Front face (+Z)
    let normal = Vec3::Z;
    for &pos in &[positions[4], positions[5], positions[6], positions[7]] {
        indices.push(builder.add_vertex(Vertex::new(pos).with_normal(normal)));
    }

    // Back face (-Z)
    let normal = -Vec3::Z;
    for &pos in &[positions[1], positions[0], positions[3], positions[2]] {
        indices.push(builder.add_vertex(Vertex::new(pos).with_normal(normal)));
    }

    // Right face (+X)
    let normal = Vec3::X;
    for &pos in &[positions[5], positions[1], positions[2], positions[6]] {
        indices.push(builder.add_vertex(Vertex::new(pos).with_normal(normal)));
    }

    // Left face (-X)
    let normal = -Vec3::X;
    for &pos in &[positions[0], positions[4], positions[7], positions[3]] {
        indices.push(builder.add_vertex(Vertex::new(pos).with_normal(normal)));
    }

    // Top face (+Y)
    let normal = Vec3::Y;
    for &pos in &[positions[3], positions[7], positions[6], positions[2]] {
        indices.push(builder.add_vertex(Vertex::new(pos).with_normal(normal)));
    }

    // Bottom face (-Y)
    let normal = -Vec3::Y;
    for &pos in &[positions[4], positions[0], positions[1], positions[5]] {
        indices.push(builder.add_vertex(Vertex::new(pos).with_normal(normal)));
    }

    // Add triangles for each face
    for i in 0..6 {
        let base = (i * 4) as u32;
        builder.add_triangle(Triangle::new(base, base + 1, base + 2));
        builder.add_triangle(Triangle::new(base, base + 2, base + 3));
    }

    builder.build(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_creation() {
        let vertex = Vertex::new(Vec3::new(1.0, 2.0, 3.0))
            .with_normal(Vec3::Y)
            .with_texcoord(Vec2::new(0.5, 0.5));

        assert_eq!(vertex.position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(vertex.normal, Vec3::Y);
        assert_eq!(vertex.texcoord, Vec2::new(0.5, 0.5));
    }

    #[test]
    fn test_mesh_builder() {
        let mut builder = MeshBuilder::new();

        let v0 = builder.add_vertex(Vertex::new(Vec3::ZERO));
        let v1 = builder.add_vertex(Vertex::new(Vec3::X));
        let v2 = builder.add_vertex(Vertex::new(Vec3::Y));

        builder.add_triangle(Triangle::new(v0, v1, v2));

        let mesh = builder.build("test_mesh".to_string());

        assert_eq!(mesh.get_num_vertices(), 3);
        assert_eq!(mesh.get_num_triangles(), 1);
        assert_eq!(mesh.name(), "test_mesh");
    }

    #[test]
    fn test_quad_mesh() {
        let mesh = create_quad_mesh("quad".to_string(), 2.0);

        assert_eq!(mesh.get_num_vertices(), 4);
        assert_eq!(mesh.get_num_triangles(), 2);
    }

    #[test]
    fn test_cube_mesh() {
        let mesh = create_cube_mesh("cube".to_string(), 1.0);

        assert_eq!(mesh.get_num_vertices(), 24); // 4 vertices per face * 6 faces
        assert_eq!(mesh.get_num_triangles(), 12); // 2 triangles per face * 6 faces
    }

    #[test]
    fn test_bounding_sphere_calculation() {
        let mesh = create_cube_mesh("cube".to_string(), 2.0);
        let sphere = mesh.get_obj_space_bounding_sphere();

        // Center should be at origin
        assert!(sphere.center.length() < 0.001);

        // Radius should be approximately sqrt(3) for a cube of size 2
        let expected_radius = (3.0f32).sqrt();
        assert!((sphere.radius - expected_radius).abs() < 0.1);
    }

    #[test]
    fn test_mesh_transform() {
        let mut mesh = create_quad_mesh("quad".to_string(), 1.0);
        let transform = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0));

        mesh.set_transform(transform);
        assert_eq!(mesh.get_transform(), transform);
    }

    #[test]
    fn test_ray_intersection() {
        let mesh = create_quad_mesh("quad".to_string(), 2.0);
        let ray = Ray::new(Vec3::new(0.0, 0.0, -5.0), Vec3::Z);

        let result = mesh.cast_ray(&ray);
        assert!(result.hit);
        assert!(result.distance > 0.0);
    }
}
