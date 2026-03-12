//! Decal System for projecting textures onto geometry
//!
//! Port of decalsys.cpp and decalmsh.cpp
//! Implements decal projection, clipping, and lifetime management.

use glam::{Mat3, Mat4, Vec2, Vec3, Vec4};
use std::collections::HashMap;
use std::sync::Arc;

/// Decal ID type
pub type DecalId = u32;

/// Decal material properties
#[derive(Debug, Clone)]
pub struct DecalMaterial {
    /// Texture ID
    pub texture_id: u64,
    /// Blend mode
    pub blend_mode: DecalBlendMode,
    /// Color tint
    pub color_tint: Vec4,
    /// UV scale
    pub uv_scale: Vec2,
    /// UV offset
    pub uv_offset: Vec2,
}

impl DecalMaterial {
    /// Create new decal material
    pub fn new(texture_id: u64) -> Self {
        Self {
            texture_id,
            blend_mode: DecalBlendMode::Alpha,
            color_tint: Vec4::ONE,
            uv_scale: Vec2::ONE,
            uv_offset: Vec2::ZERO,
        }
    }
}

/// Decal blend modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecalBlendMode {
    /// Alpha blending
    Alpha,
    /// Additive blending
    Additive,
    /// Multiplicative blending
    Multiply,
    /// Opaque (no blending)
    Opaque,
}

/// Decal structure
/// Port of C++ DecalClass (decalsys.cpp lines 172-201)
#[derive(Debug, Clone)]
pub struct Decal {
    /// Unique decal ID (decalsys.cpp line 150-153)
    pub id: DecalId,
    /// Decal position in world space
    pub position: Vec3,
    /// Decal surface normal
    pub normal: Vec3,
    /// Decal size (width, height)
    pub size: Vec2,
    /// Decal orientation (right vector)
    pub right: Vec3,
    /// Decal orientation (up vector)
    pub up: Vec3,
    /// Material properties
    pub material: DecalMaterial,
    /// Lifetime remaining (seconds)
    pub lifetime: f32,
    /// Fade time before death (seconds)
    pub fade_time: f32,
    /// Current opacity (0.0 to 1.0)
    pub opacity: f32,
    /// Creation time
    pub creation_time: f32,
}

impl Decal {
    /// Create new decal
    /// Port of DecalGeneratorClass constructor (decalsys.cpp lines 172-183)
    pub fn new(
        id: DecalId,
        position: Vec3,
        normal: Vec3,
        size: Vec2,
        material: DecalMaterial,
        lifetime: f32,
    ) -> Self {
        // Generate orthonormal basis from normal
        let (right, up) = Self::generate_basis(normal);

        Self {
            id,
            position,
            normal,
            size,
            right,
            up,
            material,
            lifetime,
            fade_time: 2.0, // Default 2 second fade
            opacity: 1.0,
            creation_time: 0.0,
        }
    }

    /// Generate orthonormal basis from normal vector
    fn generate_basis(normal: Vec3) -> (Vec3, Vec3) {
        // Choose a vector that's not parallel to normal
        let temp = if normal.y.abs() > 0.9 {
            Vec3::X
        } else {
            Vec3::Y
        };

        let right = temp.cross(normal).normalize();
        let up = normal.cross(right).normalize();

        (right, up)
    }

    /// Get decal projection matrix (world to decal space)
    /// Port of decalsys.cpp Set_Mesh_Transform (lines 261-290)
    pub fn get_projection_matrix(&self) -> Mat4 {
        // Build transform from decal space to world space
        let rotation = Mat3::from_cols(self.right, self.up, self.normal);
        let transform = Mat4::from_rotation_translation(
            glam::Quat::from_mat3(&rotation),
            self.position,
        );

        // Invert to get world to decal space
        transform.inverse()
    }

    /// Get decal bounds in local space
    pub fn get_local_bounds(&self) -> (Vec3, Vec3) {
        let half_size = Vec3::new(self.size.x * 0.5, self.size.y * 0.5, 0.1);
        (-half_size, half_size)
    }

    /// Update decal lifetime
    /// Port of decalsys.cpp Update (lines 250-280)
    pub fn update(&mut self, delta_time: f32) {
        self.lifetime -= delta_time;

        // Calculate opacity based on fade time
        if self.lifetime < self.fade_time {
            self.opacity = (self.lifetime / self.fade_time).max(0.0);
        }
    }

    /// Check if decal is expired
    pub fn is_expired(&self) -> bool {
        self.lifetime <= 0.0
    }

    /// Get current alpha value
    pub fn get_alpha(&self) -> f32 {
        self.opacity * self.material.color_tint.w
    }
}

/// Clipped decal polygon
#[derive(Debug, Clone)]
pub struct DecalPolygon {
    /// Vertex positions in world space
    pub positions: Vec<Vec3>,
    /// Vertex normals
    pub normals: Vec<Vec3>,
    /// Vertex UV coordinates
    pub uvs: Vec<Vec2>,
    /// Vertex colors
    pub colors: Vec<Vec4>,
}

impl DecalPolygon {
    /// Create new empty decal polygon
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: Vec::new(),
        }
    }

    /// Add vertex
    pub fn add_vertex(&mut self, position: Vec3, normal: Vec3, uv: Vec2, color: Vec4) {
        self.positions.push(position);
        self.normals.push(normal);
        self.uvs.push(uv);
        self.colors.push(color);
    }

    /// Get vertex count
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Check if polygon is valid
    pub fn is_valid(&self) -> bool {
        self.vertex_count() >= 3
    }
}

impl Default for DecalPolygon {
    fn default() -> Self {
        Self::new()
    }
}

/// Decal projection and clipping
pub struct DecalProjector;

impl DecalProjector {
    /// Project decal onto triangle
    /// Port of decalmsh.cpp decal projection logic (lines 120-200)
    pub fn project_onto_triangle(
        decal: &Decal,
        tri_vertices: [Vec3; 3],
        tri_normals: [Vec3; 3],
    ) -> Option<DecalPolygon> {
        let projection = decal.get_projection_matrix();

        // Check backface culling
        let avg_normal = (tri_normals[0] + tri_normals[1] + tri_normals[2]) / 3.0;
        let dot = avg_normal.dot(decal.normal);

        // Backface threshold (decalsys.cpp line 175)
        if dot < 0.0 {
            return None; // Triangle facing away
        }

        // Transform triangle to decal space
        let local_verts: Vec<Vec3> = tri_vertices
            .iter()
            .map(|&v| projection.transform_point3(v))
            .collect();

        // Get decal bounds
        let (min_bounds, max_bounds) = decal.get_local_bounds();

        // Clip triangle against decal box
        let mut clipped = Self::clip_triangle_to_box(
            local_verts[0],
            local_verts[1],
            local_verts[2],
            min_bounds,
            max_bounds,
        )?;

        // Transform back to world space
        let inv_projection = projection.inverse();
        for vert in &mut clipped {
            *vert = inv_projection.transform_point3(*vert);
        }

        // Generate UV coordinates
        let mut polygon = DecalPolygon::new();
        for vert in clipped {
            let local_pos = projection.transform_point3(vert);

            // Map to UV space [0, 1]
            let u = (local_pos.x - min_bounds.x) / (max_bounds.x - min_bounds.x);
            let v = (local_pos.y - min_bounds.y) / (max_bounds.y - min_bounds.y);

            let uv = Vec2::new(u, v) * decal.material.uv_scale + decal.material.uv_offset;

            // Interpolate normal
            let normal = Self::interpolate_normal(&tri_vertices, &tri_normals, vert);

            // Apply decal color and opacity
            let color = decal.material.color_tint * decal.opacity;

            polygon.add_vertex(vert, normal, uv, color);
        }

        Some(polygon)
    }

    /// Clip triangle to axis-aligned box
    /// Sutherland-Hodgman clipping algorithm
    fn clip_triangle_to_box(
        v0: Vec3,
        v1: Vec3,
        v2: Vec3,
        min_bounds: Vec3,
        max_bounds: Vec3,
    ) -> Option<Vec<Vec3>> {
        let mut polygon = vec![v0, v1, v2];

        // Clip against each plane
        polygon = Self::clip_polygon_to_plane(&polygon, Vec3::X, min_bounds.x, true)?;
        polygon = Self::clip_polygon_to_plane(&polygon, Vec3::X, max_bounds.x, false)?;
        polygon = Self::clip_polygon_to_plane(&polygon, Vec3::Y, min_bounds.y, true)?;
        polygon = Self::clip_polygon_to_plane(&polygon, Vec3::Y, max_bounds.y, false)?;
        polygon = Self::clip_polygon_to_plane(&polygon, Vec3::Z, min_bounds.z, true)?;
        polygon = Self::clip_polygon_to_plane(&polygon, Vec3::Z, max_bounds.z, false)?;

        if polygon.len() >= 3 {
            Some(polygon)
        } else {
            None
        }
    }

    /// Clip polygon to plane
    fn clip_polygon_to_plane(
        polygon: &[Vec3],
        normal: Vec3,
        distance: f32,
        greater_than: bool,
    ) -> Option<Vec<Vec3>> {
        let mut output = Vec::new();

        for i in 0..polygon.len() {
            let v0 = polygon[i];
            let v1 = polygon[(i + 1) % polygon.len()];

            let d0 = normal.dot(v0) - distance;
            let d1 = normal.dot(v1) - distance;

            let inside0 = if greater_than { d0 >= 0.0 } else { d0 <= 0.0 };
            let inside1 = if greater_than { d1 >= 0.0 } else { d1 <= 0.0 };

            if inside0 {
                output.push(v0);
            }

            // Edge crosses plane
            if inside0 != inside1 {
                let t = d0 / (d0 - d1);
                let intersection = v0 + (v1 - v0) * t;
                output.push(intersection);
            }
        }

        if output.len() >= 3 {
            Some(output)
        } else {
            None
        }
    }

    /// Interpolate normal at point on triangle
    fn interpolate_normal(positions: &[Vec3; 3], normals: &[Vec3; 3], point: Vec3) -> Vec3 {
        // Barycentric interpolation
        let v0 = positions[1] - positions[0];
        let v1 = positions[2] - positions[0];
        let v2 = point - positions[0];

        let d00 = v0.dot(v0);
        let d01 = v0.dot(v1);
        let d11 = v1.dot(v1);
        let d20 = v2.dot(v0);
        let d21 = v2.dot(v1);

        let denom = d00 * d11 - d01 * d01;
        if denom.abs() < 0.0001 {
            return normals[0];
        }

        let v = (d11 * d20 - d01 * d21) / denom;
        let w = (d00 * d21 - d01 * d20) / denom;
        let u = 1.0 - v - w;

        (normals[0] * u + normals[1] * v + normals[2] * w).normalize()
    }
}

/// Decal system manager
/// Port of DecalSystemClass (decalsys.cpp lines 76-153)
pub struct DecalSystem {
    /// Active decals
    decals: HashMap<DecalId, Decal>,
    /// Next decal ID (decalsys.cpp line 58)
    next_id: DecalId,
    /// Maximum decals
    max_decals: usize,
}

impl DecalSystem {
    /// Create new decal system
    pub fn new(max_decals: usize) -> Self {
        Self {
            decals: HashMap::new(),
            next_id: 0,
            max_decals,
        }
    }

    /// Generate unique decal ID
    /// Port of Generate_Unique_Global_Decal_Id (decalsys.cpp lines 150-153)
    fn generate_decal_id(&mut self) -> DecalId {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    /// Add decal to system
    /// Port of Lock_Decal_Generator/Unlock_Decal_Generator (decalsys.cpp lines 109-134)
    pub fn add_decal(&mut self, mut decal: Decal) -> DecalId {
        // Enforce max decals (remove oldest if needed)
        if self.decals.len() >= self.max_decals {
            self.remove_oldest_decal();
        }

        let id = self.generate_decal_id();
        decal.id = id;

        self.decals.insert(id, decal);
        id
    }

    /// Create and add decal
    pub fn create_decal(
        &mut self,
        position: Vec3,
        normal: Vec3,
        size: Vec2,
        material: DecalMaterial,
        lifetime: f32,
    ) -> DecalId {
        let id = self.generate_decal_id();
        let decal = Decal::new(id, position, normal, size, material, lifetime);
        self.decals.insert(id, decal);
        id
    }

    /// Remove decal by ID
    /// Port of Decal_Mesh_Destroyed (decalsys.cpp lines 356-364)
    pub fn remove_decal(&mut self, id: DecalId) -> Option<Decal> {
        self.decals.remove(&id)
    }

    /// Remove oldest decal
    fn remove_oldest_decal(&mut self) {
        if let Some(oldest_id) = self
            .decals
            .iter()
            .min_by(|a, b| a.1.creation_time.partial_cmp(&b.1.creation_time).unwrap())
            .map(|(id, _)| *id)
        {
            self.decals.remove(&oldest_id);
        }
    }

    /// Update all decals
    /// Port of Update (decalsys.cpp lines 250-280)
    pub fn update(&mut self, delta_time: f32) {
        // Update lifetimes
        for decal in self.decals.values_mut() {
            decal.update(delta_time);
        }

        // Remove expired decals
        self.decals.retain(|_, decal| !decal.is_expired());
    }

    /// Get decal by ID
    pub fn get_decal(&self, id: DecalId) -> Option<&Decal> {
        self.decals.get(&id)
    }

    /// Get all active decals
    pub fn decals(&self) -> impl Iterator<Item = &Decal> {
        self.decals.values()
    }

    /// Clear all decals
    /// Port of Clear_All_Decals (decalsys.cpp lines 384-393)
    pub fn clear_all(&mut self) {
        self.decals.clear();
    }

    /// Get decal count
    pub fn decal_count(&self) -> usize {
        self.decals.len()
    }
}

impl Default for DecalSystem {
    fn default() -> Self {
        Self::new(256) // Default 256 max decals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decal_creation() {
        let material = DecalMaterial::new(1);
        let decal = Decal::new(
            0,
            Vec3::ZERO,
            Vec3::Y,
            Vec2::new(2.0, 2.0),
            material,
            10.0,
        );

        assert_eq!(decal.id, 0);
        assert_eq!(decal.position, Vec3::ZERO);
        assert_eq!(decal.lifetime, 10.0);
        assert_eq!(decal.opacity, 1.0);
    }

    #[test]
    fn test_decal_update() {
        let material = DecalMaterial::new(1);
        let mut decal = Decal::new(
            0,
            Vec3::ZERO,
            Vec3::Y,
            Vec2::new(2.0, 2.0),
            material,
            5.0,
        );

        decal.fade_time = 2.0;
        decal.update(3.0);

        assert_eq!(decal.lifetime, 2.0);
        assert_eq!(decal.opacity, 1.0); // Still above fade threshold

        decal.update(1.0);
        assert_eq!(decal.lifetime, 1.0);
        assert_eq!(decal.opacity, 0.5); // In fade period
    }

    #[test]
    fn test_decal_expiration() {
        let material = DecalMaterial::new(1);
        let mut decal = Decal::new(
            0,
            Vec3::ZERO,
            Vec3::Y,
            Vec2::new(2.0, 2.0),
            material,
            1.0,
        );

        assert!(!decal.is_expired());

        decal.update(2.0);
        assert!(decal.is_expired());
    }

    #[test]
    fn test_decal_system_creation() {
        let system = DecalSystem::new(100);
        assert_eq!(system.decal_count(), 0);
        assert_eq!(system.max_decals, 100);
    }

    #[test]
    fn test_decal_system_add_remove() {
        let mut system = DecalSystem::new(10);
        let material = DecalMaterial::new(1);

        let id = system.create_decal(
            Vec3::ZERO,
            Vec3::Y,
            Vec2::new(1.0, 1.0),
            material,
            10.0,
        );

        assert_eq!(system.decal_count(), 1);
        assert!(system.get_decal(id).is_some());

        system.remove_decal(id);
        assert_eq!(system.decal_count(), 0);
    }

    #[test]
    fn test_decal_system_max_limit() {
        let mut system = DecalSystem::new(3);
        let material = DecalMaterial::new(1);

        // Add 5 decals (should only keep 3)
        for _ in 0..5 {
            system.create_decal(Vec3::ZERO, Vec3::Y, Vec2::ONE, material.clone(), 10.0);
        }

        assert_eq!(system.decal_count(), 3);
    }

    #[test]
    fn test_decal_system_update() {
        let mut system = DecalSystem::new(10);
        let material = DecalMaterial::new(1);

        system.create_decal(Vec3::ZERO, Vec3::Y, Vec2::ONE, material, 1.0);

        assert_eq!(system.decal_count(), 1);

        system.update(2.0);
        assert_eq!(system.decal_count(), 0); // Expired
    }

    #[test]
    fn test_decal_polygon_creation() {
        let mut poly = DecalPolygon::new();
        poly.add_vertex(Vec3::ZERO, Vec3::Y, Vec2::ZERO, Vec4::ONE);
        poly.add_vertex(Vec3::X, Vec3::Y, Vec2::X, Vec4::ONE);
        poly.add_vertex(Vec3::Z, Vec3::Y, Vec2::Y, Vec4::ONE);

        assert_eq!(poly.vertex_count(), 3);
        assert!(poly.is_valid());
    }

    #[test]
    fn test_decal_projection_matrix() {
        let material = DecalMaterial::new(1);
        let decal = Decal::new(
            0,
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::Y,
            Vec2::new(2.0, 2.0),
            material,
            10.0,
        );

        let proj = decal.get_projection_matrix();

        // Point at decal center should transform to near origin
        let local = proj.transform_point3(decal.position);
        assert!(local.length() < 0.1);
    }

    #[test]
    fn test_decal_blend_modes() {
        assert_ne!(DecalBlendMode::Alpha, DecalBlendMode::Additive);
        assert_eq!(DecalBlendMode::Alpha, DecalBlendMode::Alpha);
    }
}
