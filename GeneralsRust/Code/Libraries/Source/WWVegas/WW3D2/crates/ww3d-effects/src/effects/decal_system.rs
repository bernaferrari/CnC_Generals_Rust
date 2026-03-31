/*
**	Command & Conquer Generals Zero Hour(tm) Rust Port
**	Copyright 2025
**
**	This implements the WW3D Decal System for projecting textures onto geometry
**	Port of decalsys.cpp/h and decalmsh.cpp/h
**
**	C++ Reference: /GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/
**	  - decalsys.h:    Lines 1-269  (DecalSystemClass, DecalGeneratorClass, MultiFixedPoolDecalSystemClass)
**	  - decalsys.cpp:  Lines 1-441  (Implementation of decal system logic)
**	  - decalmsh.h:    Lines 1-256  (DecalMeshClass, RigidDecalMeshClass, SkinDecalMeshClass)
**	  - decalmsh.cpp:  Lines 1-1059 (Implementation of decal mesh clipping and rendering)
*/

use glam::{Mat4, Vec2, Vec3};
use std::sync::{Arc, Mutex, OnceLock};
use thiserror::Error;

/// Errors that can occur in the decal system
#[derive(Error, Debug)]
pub enum DecalError {
    #[error("Invalid decal ID")]
    InvalidId,
    #[error("System not initialized")]
    NotInitialized,
    #[error("Pool full")]
    PoolFull,
    #[error("Mesh not found")]
    MeshNotFound,
}

pub type DecalResult<T> = Result<T, DecalError>;

/// Unique identifier for decals
pub type DecalId = u32;

//------------------------------------------------------------------------------
// PROJECTOR CLASS
// Base class for projecting textures onto geometry
//------------------------------------------------------------------------------

/// OBBox (Oriented Bounding Box) for decal bounds
#[derive(Debug, Clone)]
pub struct OBBox {
    pub center: Vec3,
    pub basis: Mat4,  // Rotation/orientation
    pub extent: Vec3, // Half-extents along each axis
}

impl OBBox {
    pub fn new(center: Vec3, basis: Mat4, extent: Vec3) -> Self {
        Self {
            center,
            basis,
            extent,
        }
    }
}

/// Projector base class - handles coordinate transforms and projection
#[derive(Debug, Clone)]
pub struct Projector {
    /// World transform of the projector
    pub transform: Mat4,
    /// Projection matrix (orthographic or perspective)
    pub projection: Mat4,
    /// Oriented bounding box in local space
    pub bounds: OBBox,
}

impl Projector {
    pub fn new(transform: Mat4, projection: Mat4, bounds: OBBox) -> Self {
        Self {
            transform,
            projection,
            bounds,
        }
    }

    /// Compute texture coordinate for a world-space vertex
    pub fn compute_texture_coordinate(&self, world_pos: Vec3, mesh_transform: &Mat4) -> Vec3 {
        // Transform: obj -> world -> texture
        // Vproj = Projection * WorldToTexture * MeshToWorld * Vobj

        let world_to_texture = self.transform.inverse();
        let combined = self.projection * world_to_texture * *mesh_transform;
        let proj_pos = combined.project_point3(world_pos);

        // Return as (s, t, q) where s/q and t/q give final UV
        proj_pos
    }

    /// Get bounding box in world space
    pub fn get_world_bounding_box(&self) -> OBBox {
        let world_center = self.transform.transform_point3(self.bounds.center);
        let world_basis = self.transform * self.bounds.basis;
        OBBox {
            center: world_center,
            basis: world_basis,
            extent: self.bounds.extent,
        }
    }
}

//------------------------------------------------------------------------------
// PLANE CLASS
// Used for polygon clipping
//------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct Plane {
    pub normal: Vec3,
    pub d: f32, // distance from origin
}

impl Plane {
    pub fn new(normal: Vec3, point: Vec3) -> Self {
        let n = normal.normalize();
        Self {
            normal: n,
            d: n.dot(point),
        }
    }

    /// Test if a point is in front of the plane
    pub fn in_front(&self, point: Vec3) -> bool {
        self.normal.dot(point) - self.d > 0.0001
    }

    /// Classify point relative to plane
    pub fn classify_point(&self, point: Vec3) -> i32 {
        let dist = self.normal.dot(point) - self.d;
        if dist > 0.0001 {
            1 // Front
        } else if dist < -0.0001 {
            -1 // Back
        } else {
            0 // On plane
        }
    }

    /// Compute intersection parameter between two points
    pub fn compute_intersection(&self, p0: Vec3, p1: Vec3) -> f32 {
        let d0 = self.normal.dot(p0) - self.d;
        let d1 = self.normal.dot(p1) - self.d;

        if (d1 - d0).abs() < 0.00001 {
            return 0.0;
        }

        -d0 / (d1 - d0)
    }
}

//------------------------------------------------------------------------------
// DECAL VERTEX
// Vertex data for decal polygons during clipping
//------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DecalVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub tex_coord: Vec2,
    pub color: u32,
}

impl DecalVertex {
    pub fn new(position: Vec3, normal: Vec3) -> Self {
        Self {
            position,
            normal,
            tex_coord: Vec2::ZERO,
            color: 0xFFFFFFFF,
        }
    }

    /// Linear interpolation between two vertices
    pub fn lerp(v0: &Self, v1: &Self, t: f32) -> Self {
        // Linear interpolation of all vertex attributes including color (RGBA32)
        // Extract RGBA components from u32 (ARGB format)
        let a0 = ((v0.color >> 24) & 0xFF) as f32;
        let r0 = ((v0.color >> 16) & 0xFF) as f32;
        let g0 = ((v0.color >> 8) & 0xFF) as f32;
        let b0 = (v0.color & 0xFF) as f32;

        let a1 = ((v1.color >> 24) & 0xFF) as f32;
        let r1 = ((v1.color >> 16) & 0xFF) as f32;
        let g1 = ((v1.color >> 8) & 0xFF) as f32;
        let b1 = (v1.color & 0xFF) as f32;

        // Interpolate each component
        let a = (a0 + (a1 - a0) * t) as u32;
        let r = (r0 + (r1 - r0) * t) as u32;
        let g = (g0 + (g1 - g0) * t) as u32;
        let b = (b0 + (b1 - b0) * t) as u32;

        // Reconstruct u32 color
        let color = (a << 24) | (r << 16) | (g << 8) | b;

        Self {
            position: v0.position.lerp(v1.position, t),
            normal: v0.normal.lerp(v1.normal, t).normalize(),
            tex_coord: v0.tex_coord.lerp(v1.tex_coord, t),
            color,
        }
    }
}

//------------------------------------------------------------------------------
// DECAL POLYGON
// Temporary polygon structure used during clipping
//------------------------------------------------------------------------------

const MAX_DECAL_VERTS: usize = 24;

#[derive(Debug, Clone)]
pub struct DecalPolygon {
    pub vertices: Vec<DecalVertex>,
}

impl DecalPolygon {
    pub fn new() -> Self {
        Self {
            vertices: Vec::with_capacity(MAX_DECAL_VERTS),
        }
    }

    pub fn add_vertex(&mut self, vertex: DecalVertex) {
        if self.vertices.len() < MAX_DECAL_VERTS {
            self.vertices.push(vertex);
        }
    }

    pub fn reset(&mut self) {
        self.vertices.clear();
    }

    /// Sutherland-Hodgeman polygon clipping algorithm
    /// C++ Reference: decalmsh.cpp lines 55-123 (DecalPolyClass::Clip)
    pub fn clip(&self, plane: &Plane) -> DecalPolygon {
        let mut result = DecalPolygon::new();

        if self.vertices.len() < 3 {
            return result;
        }

        let mut prev_idx = self.vertices.len() - 1;
        let mut prev_in_front = plane.in_front(self.vertices[prev_idx].position);

        for i in 0..self.vertices.len() {
            let cur_in_front = plane.in_front(self.vertices[i].position);

            if prev_in_front {
                if cur_in_front {
                    // Both in front - emit current vertex
                    result.add_vertex(self.vertices[i].clone());
                } else {
                    // Previous in front, current behind - emit intersection
                    let t = plane.compute_intersection(
                        self.vertices[prev_idx].position,
                        self.vertices[i].position,
                    );
                    let int_vertex =
                        DecalVertex::lerp(&self.vertices[prev_idx], &self.vertices[i], t);
                    result.add_vertex(int_vertex);
                }
            } else {
                if cur_in_front {
                    // Previous behind, current in front - emit intersection and current
                    let t = plane.compute_intersection(
                        self.vertices[prev_idx].position,
                        self.vertices[i].position,
                    );
                    let int_vertex =
                        DecalVertex::lerp(&self.vertices[prev_idx], &self.vertices[i], t);
                    result.add_vertex(int_vertex);
                    result.add_vertex(self.vertices[i].clone());
                }
                // Both behind - emit nothing
            }

            prev_in_front = cur_in_front;
            prev_idx = i;
        }

        result
    }
}

//------------------------------------------------------------------------------
// DECAL MATERIAL
// Material/texture settings for a decal
//------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DecalMaterial {
    pub texture_name: String,
    pub shader_name: String,
    pub blend_mode: BlendMode,
    pub two_sided: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlendMode {
    Opaque,
    AlphaBlend,
    Additive,
    Multiplicative,
}

impl Default for DecalMaterial {
    fn default() -> Self {
        Self {
            texture_name: String::new(),
            shader_name: String::from("default"),
            blend_mode: BlendMode::AlphaBlend,
            two_sided: false,
        }
    }
}

//------------------------------------------------------------------------------
// DECAL GENERATOR
// Main class for generating decals on meshes
//------------------------------------------------------------------------------

/// DecalGenerator - encapsulates all information needed to generate a decal
/// C++ Reference: decalsys.h lines 93-168 (DecalGeneratorClass)
pub struct DecalGenerator {
    /// Unique ID for this decal
    pub decal_id: DecalId,
    /// Projector for coordinate transforms
    pub projector: Projector,
    /// Material settings
    pub material: DecalMaterial,
    /// Backface culling threshold
    /// C++ Reference: decalsys.h lines 117-118 (Get_Backface_Threshhold)
    pub backface_threshold: f32,
    /// Whether to apply to translucent meshes
    /// C++ Reference: decalsys.h lines 125-126 (Apply_To_Translucent_Meshes)
    pub apply_to_translucent: bool,
    /// List of meshes that were affected
    /// C++ Reference: decalsys.h line 165 (MeshList)
    pub affected_meshes: Vec<String>,
}

impl DecalGenerator {
    pub fn new(decal_id: DecalId, projector: Projector, material: DecalMaterial) -> Self {
        Self {
            decal_id,
            projector,
            material,
            backface_threshold: 0.0,
            apply_to_translucent: false,
            affected_meshes: Vec::new(),
        }
    }

    pub fn add_affected_mesh(&mut self, mesh_name: String) {
        if !self.affected_meshes.contains(&mesh_name) {
            self.affected_meshes.push(mesh_name);
        }
    }

    /// Compute clipping planes from the projector bounds
    pub fn get_clipping_planes(&self, mesh_transform: &Mat4) -> Vec<Plane> {
        let mut planes = Vec::with_capacity(6);

        // Transform bounds to mesh local space
        let _world_to_mesh = mesh_transform.inverse();
        let bounds = &self.projector.bounds;

        // Create 4 clipping planes from the oriented bounding box sides
        // (we skip near/far for now as they're handled by projection)

        let basis = bounds.basis;
        let x_axis = basis.x_axis.truncate();
        let y_axis = basis.y_axis.truncate();
        let center = bounds.center;
        let extent = bounds.extent;

        // +X plane
        let point = center + x_axis * extent.x;
        planes.push(Plane::new(-x_axis, point));

        // -X plane
        let point = center - x_axis * extent.x;
        planes.push(Plane::new(x_axis, point));

        // +Y plane
        let point = center + y_axis * extent.y;
        planes.push(Plane::new(-y_axis, point));

        // -Y plane
        let point = center - y_axis * extent.y;
        planes.push(Plane::new(y_axis, point));

        planes
    }
}

//------------------------------------------------------------------------------
// DECAL STRUCTURE
// Represents one decal instance in a mesh
//------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Decal {
    pub decal_id: DecalId,
    pub vertex_start: usize,
    pub vertex_count: usize,
    pub face_start: usize,
    pub face_count: usize,
}

//------------------------------------------------------------------------------
// DECAL MESH
// Base trait for meshes that support decals
//------------------------------------------------------------------------------

pub trait DecalMesh: Send + Sync {
    /// Create a decal on this mesh
    fn create_decal(
        &mut self,
        generator: &mut DecalGenerator,
        affected_polygons: &[u32],
    ) -> DecalResult<bool>;

    /// Delete a decal by ID
    fn delete_decal(&mut self, decal_id: DecalId) -> DecalResult<bool>;

    /// Get the number of decals on this mesh
    fn decal_count(&self) -> usize;

    /// Get the decal ID at the given index
    fn get_decal_id(&self, index: usize) -> Option<DecalId>;

    /// Render all decals on this mesh
    fn render(&self);

    /// Get the decal mesh geometry for rendering (RigidDecalMesh specific)
    fn get_geometry(&self) -> (&[Vec3], &[Vec3], &[Vec2], &[u32]) {
        // Default implementation returns empty slices
        (&[], &[], &[], &[])
    }

    /// Get material information for a specific triangle
    fn get_triangle_material(&self, _triangle_idx: usize) -> Option<(&str, &str)> {
        // Default implementation returns None
        None
    }

    /// Get the decal mesh geometry references for rendering (SkinDecalMesh specific)
    fn get_geometry_refs(&self) -> (&[u32], &[Vec2], &[u32]) {
        // Default implementation returns empty slices
        (&[], &[], &[])
    }
}

//------------------------------------------------------------------------------
// RIGID DECAL MESH
// Decal mesh for static (non-skinned) geometry
// C++ Reference: decalmsh.h lines 86-143, decalmsh.cpp lines 173-643
//------------------------------------------------------------------------------

#[derive(Debug)]
pub struct RigidDecalMesh {
    pub parent_mesh_name: String,
    pub decals: Vec<Decal>,

    // Geometry data
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub tex_coords: Vec<Vec2>,
    pub colors: Vec<u32>,
    pub indices: Vec<u32>,

    // Material data (per-triangle)
    pub textures: Vec<String>,
    pub shaders: Vec<String>,

    // Parent mesh reference for accessing geometry
    pub parent_vertices: Vec<Vec3>,
    pub parent_normals: Vec<Vec3>,
    pub parent_indices: Vec<u32>,
}

impl RigidDecalMesh {
    pub fn new(parent_mesh_name: String) -> Self {
        Self {
            parent_mesh_name,
            decals: Vec::new(),
            vertices: Vec::new(),
            normals: Vec::new(),
            tex_coords: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
            textures: Vec::new(),
            shaders: Vec::new(),
            parent_vertices: Vec::new(),
            parent_normals: Vec::new(),
            parent_indices: Vec::new(),
        }
    }

    /// Set the parent mesh geometry to reference for decal projection
    pub fn set_parent_geometry(
        &mut self,
        vertices: Vec<Vec3>,
        normals: Vec<Vec3>,
        indices: Vec<u32>,
    ) {
        self.parent_vertices = vertices;
        self.parent_normals = normals;
        self.parent_indices = indices;
    }

    /// Process a triangle and clip it to the decal bounds
    /// C++ Reference: decalmsh.cpp lines 361-552 (RigidDecalMeshClass::Create_Decal)
    /// This implements the core clipping algorithm from lines 445-517
    fn clip_triangle(
        &mut self,
        generator: &DecalGenerator,
        tri_verts: &[Vec3; 3],
        tri_normals: &[Vec3; 3],
        mesh_transform: &Mat4,
        zbias_offset: Vec3,
    ) -> bool {
        // Create initial polygon from triangle
        let mut poly = DecalPolygon::new();
        for i in 0..3 {
            let vertex = DecalVertex::new(tri_verts[i] + zbias_offset, tri_normals[i]);
            poly.add_vertex(vertex);
        }

        // Clip against all bounding planes
        let planes = generator.get_clipping_planes(mesh_transform);
        for plane in &planes {
            poly = poly.clip(plane);
            if poly.vertices.len() < 3 {
                return false; // Clipped away completely
            }
        }

        // Add clipped polygon to mesh as triangle fan
        if poly.vertices.len() >= 3 {
            self.add_polygon_as_triangles(&poly, generator, mesh_transform);
            true
        } else {
            false
        }
    }

    /// Convert polygon to triangle fan and add to mesh
    fn add_polygon_as_triangles(
        &mut self,
        poly: &DecalPolygon,
        generator: &DecalGenerator,
        mesh_transform: &Mat4,
    ) {
        let first_vert_idx = self.vertices.len();

        // Add all vertices
        for vert in &poly.vertices {
            self.vertices.push(vert.position);
            self.normals.push(vert.normal);

            // Compute texture coordinates using projector
            let tex_coord = generator
                .projector
                .compute_texture_coordinate(vert.position, mesh_transform);
            self.tex_coords.push(Vec2::new(tex_coord.x, tex_coord.y));
            self.colors.push(vert.color);
        }

        // Create triangle fan (0, 1, 2), (0, 2, 3), (0, 3, 4), etc.
        for i in 1..(poly.vertices.len() - 1) {
            self.indices.push(first_vert_idx as u32);
            self.indices.push((first_vert_idx + i) as u32);
            self.indices.push((first_vert_idx + i + 1) as u32);

            // Store material info per triangle
            self.textures.push(generator.material.texture_name.clone());
            self.shaders.push(generator.material.shader_name.clone());
        }
    }
}

impl DecalMesh for RigidDecalMesh {
    fn create_decal(
        &mut self,
        generator: &mut DecalGenerator,
        affected_polygons: &[u32],
    ) -> DecalResult<bool> {
        if affected_polygons.is_empty() {
            return Ok(false);
        }

        if self.parent_vertices.is_empty() || self.parent_indices.is_empty() {
            return Err(DecalError::MeshNotFound);
        }

        let decal_start_face = self.indices.len() / 3;
        let decal_start_vert = self.vertices.len();

        // Mesh is in object space, decal projector is in world space
        let mesh_transform = Mat4::IDENTITY;
        // Small bias to prevent z-fighting
        let zbias_offset = generator.projector.bounds.basis.z_axis.truncate() * 0.001;

        let mut added_any = false;

        // Process each affected polygon
        for &poly_idx in affected_polygons {
            // Get actual triangle vertices from parent mesh
            let tri_idx = poly_idx as usize;
            if tri_idx * 3 + 2 >= self.parent_indices.len() {
                continue; // Invalid polygon index
            }

            let idx0 = self.parent_indices[tri_idx * 3] as usize;
            let idx1 = self.parent_indices[tri_idx * 3 + 1] as usize;
            let idx2 = self.parent_indices[tri_idx * 3 + 2] as usize;

            if idx0 >= self.parent_vertices.len()
                || idx1 >= self.parent_vertices.len()
                || idx2 >= self.parent_vertices.len()
            {
                continue; // Invalid vertex indices
            }

            let tri_verts = [
                self.parent_vertices[idx0],
                self.parent_vertices[idx1],
                self.parent_vertices[idx2],
            ];

            // Get normals if available, otherwise compute from triangle
            let tri_normals = if idx0 < self.parent_normals.len()
                && idx1 < self.parent_normals.len()
                && idx2 < self.parent_normals.len()
            {
                [
                    self.parent_normals[idx0],
                    self.parent_normals[idx1],
                    self.parent_normals[idx2],
                ]
            } else {
                // Compute face normal
                let edge1 = tri_verts[1] - tri_verts[0];
                let edge2 = tri_verts[2] - tri_verts[0];
                let normal = edge1.cross(edge2).normalize_or_zero();
                [normal, normal, normal]
            };

            if self.clip_triangle(
                generator,
                &tri_verts,
                &tri_normals,
                &mesh_transform,
                zbias_offset,
            ) {
                added_any = true;
            }
        }

        if added_any {
            let decal = Decal {
                decal_id: generator.decal_id,
                vertex_start: decal_start_vert,
                vertex_count: self.vertices.len() - decal_start_vert,
                face_start: decal_start_face,
                face_count: (self.indices.len() / 3) - decal_start_face,
            };
            self.decals.push(decal);
            generator.add_affected_mesh(self.parent_mesh_name.clone());
        }

        Ok(added_any)
    }

    fn delete_decal(&mut self, decal_id: DecalId) -> DecalResult<bool> {
        // Find the decal
        let decal_idx = self.decals.iter().position(|d| d.decal_id == decal_id);

        if let Some(idx) = decal_idx {
            // Copy values we need before borrowing mutably
            let vert_start = self.decals[idx].vertex_start;
            let vert_count = self.decals[idx].vertex_count;
            let face_start = self.decals[idx].face_start * 3; // Convert to index count
            let face_count = self.decals[idx].face_count * 3;
            let decal_face_start = self.decals[idx].face_start;
            let decal_face_count = self.decals[idx].face_count;

            self.vertices.drain(vert_start..(vert_start + vert_count));
            self.normals.drain(vert_start..(vert_start + vert_count));
            self.tex_coords.drain(vert_start..(vert_start + vert_count));
            self.colors.drain(vert_start..(vert_start + vert_count));
            self.indices.drain(face_start..(face_start + face_count));

            // Re-index remaining triangles
            for idx in &mut self.indices {
                if *idx > vert_start as u32 {
                    *idx -= vert_count as u32;
                }
            }

            // Remove material data
            self.textures
                .drain(decal_face_start..(decal_face_start + decal_face_count));
            self.shaders
                .drain(decal_face_start..(decal_face_start + decal_face_count));

            // Update subsequent decals
            for d in self.decals.iter_mut().skip(idx + 1) {
                d.vertex_start -= vert_count;
                d.face_start -= decal_face_count;
            }

            self.decals.remove(idx);
            Ok(true)
        } else {
            Err(DecalError::InvalidId)
        }
    }

    fn decal_count(&self) -> usize {
        self.decals.len()
    }

    fn get_decal_id(&self, index: usize) -> Option<DecalId> {
        self.decals.get(index).map(|d| d.decal_id)
    }

    fn render(&self) {
        // Rendering is handled by the mesh system
        // The decal geometry (vertices, normals, tex_coords, indices) is ready
        // to be submitted to the renderer as a standard mesh

        // In a complete implementation, this would:
        // 1. Create a temporary MeshClass instance from the decal geometry
        // 2. Queue it to the renderer with the appropriate material
        // 3. Apply the decal blend mode (usually alpha blend or multiply)
    }

    /// Get the decal mesh geometry for rendering
    fn get_geometry(&self) -> (&[Vec3], &[Vec3], &[Vec2], &[u32]) {
        (
            &self.vertices,
            &self.normals,
            &self.tex_coords,
            &self.indices,
        )
    }

    /// Get material information for a specific triangle
    fn get_triangle_material(&self, triangle_idx: usize) -> Option<(&str, &str)> {
        if triangle_idx < self.textures.len() {
            Some((&self.textures[triangle_idx], &self.shaders[triangle_idx]))
        } else {
            None
        }
    }
}

//------------------------------------------------------------------------------
// SKIN DECAL MESH
// Decal mesh for skinned/animated geometry
// C++ Reference: decalmsh.h lines 151-207, decalmsh.cpp lines 657-1059
//------------------------------------------------------------------------------

#[derive(Debug)]
pub struct SkinDecalMesh {
    pub parent_mesh_name: String,
    pub decals: Vec<Decal>,

    // Indirected vertex indices (references to parent mesh vertices)
    pub parent_vertex_indices: Vec<u32>,
    pub tex_coords: Vec<Vec2>,
    pub colors: Vec<u32>,
    pub indices: Vec<u32>,

    // Material data (per-triangle)
    pub textures: Vec<String>,
    pub shaders: Vec<String>,

    // Parent mesh reference for accessing geometry
    pub parent_vertices: Vec<Vec3>,
    pub parent_normals: Vec<Vec3>,
    pub parent_triangle_indices: Vec<u32>,
}

impl SkinDecalMesh {
    pub fn new(parent_mesh_name: String) -> Self {
        Self {
            parent_mesh_name,
            decals: Vec::new(),
            parent_vertex_indices: Vec::new(),
            tex_coords: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
            textures: Vec::new(),
            shaders: Vec::new(),
            parent_vertices: Vec::new(),
            parent_normals: Vec::new(),
            parent_triangle_indices: Vec::new(),
        }
    }

    /// Set the parent mesh geometry to reference for decal projection
    pub fn set_parent_geometry(
        &mut self,
        vertices: Vec<Vec3>,
        normals: Vec<Vec3>,
        indices: Vec<u32>,
    ) {
        self.parent_vertices = vertices;
        self.parent_normals = normals;
        self.parent_triangle_indices = indices;
    }
}

impl DecalMesh for SkinDecalMesh {
    fn create_decal(
        &mut self,
        generator: &mut DecalGenerator,
        affected_polygons: &[u32],
    ) -> DecalResult<bool> {
        if affected_polygons.is_empty() {
            return Ok(false);
        }

        if self.parent_vertices.is_empty() || self.parent_triangle_indices.is_empty() {
            return Err(DecalError::MeshNotFound);
        }

        let decal_start_face = self.indices.len() / 3;
        let decal_start_vert = self.parent_vertex_indices.len();

        // For skin meshes, we store references to parent vertices
        // The actual vertex positions are computed dynamically during rendering

        let mesh_transform = Mat4::IDENTITY; // World space for skins

        for &poly_idx in affected_polygons {
            // Get actual triangle vertex indices from parent mesh
            let tri_idx = poly_idx as usize;
            if tri_idx * 3 + 2 >= self.parent_triangle_indices.len() {
                continue; // Invalid polygon index
            }

            let idx0 = self.parent_triangle_indices[tri_idx * 3];
            let idx1 = self.parent_triangle_indices[tri_idx * 3 + 1];
            let idx2 = self.parent_triangle_indices[tri_idx * 3 + 2];

            if idx0 as usize >= self.parent_vertices.len()
                || idx1 as usize >= self.parent_vertices.len()
                || idx2 as usize >= self.parent_vertices.len()
            {
                continue; // Invalid vertex indices
            }

            let tri_indices = [idx0, idx1, idx2];
            let first_vert_idx = self.parent_vertex_indices.len();

            // Add parent vertex references
            for &vert_idx in &tri_indices {
                self.parent_vertex_indices.push(vert_idx);

                // Compute UV for this vertex position using actual vertex data
                let world_pos = self.parent_vertices[vert_idx as usize];
                let tex_coord = generator
                    .projector
                    .compute_texture_coordinate(world_pos, &mesh_transform);
                self.tex_coords.push(Vec2::new(tex_coord.x, tex_coord.y));
                self.colors.push(0xFFFFFFFF);
            }

            // Add triangle
            self.indices.push(first_vert_idx as u32);
            self.indices.push((first_vert_idx + 1) as u32);
            self.indices.push((first_vert_idx + 2) as u32);

            // Store material
            self.textures.push(generator.material.texture_name.clone());
            self.shaders.push(generator.material.shader_name.clone());
        }

        let decal = Decal {
            decal_id: generator.decal_id,
            vertex_start: decal_start_vert,
            vertex_count: self.parent_vertex_indices.len() - decal_start_vert,
            face_start: decal_start_face,
            face_count: (self.indices.len() / 3) - decal_start_face,
        };

        self.decals.push(decal);
        generator.add_affected_mesh(self.parent_mesh_name.clone());

        Ok(true)
    }

    fn delete_decal(&mut self, decal_id: DecalId) -> DecalResult<bool> {
        // Find the decal
        let decal_idx = self.decals.iter().position(|d| d.decal_id == decal_id);

        if let Some(idx) = decal_idx {
            // Copy values we need before borrowing mutably
            let vert_start = self.decals[idx].vertex_start;
            let vert_count = self.decals[idx].vertex_count;
            let face_start = self.decals[idx].face_start * 3;
            let face_count = self.decals[idx].face_count * 3;
            let decal_face_start = self.decals[idx].face_start;
            let decal_face_count = self.decals[idx].face_count;

            self.parent_vertex_indices
                .drain(vert_start..(vert_start + vert_count));
            self.tex_coords.drain(vert_start..(vert_start + vert_count));
            self.colors.drain(vert_start..(vert_start + vert_count));
            self.indices.drain(face_start..(face_start + face_count));

            // Re-index remaining triangles
            for idx in &mut self.indices {
                if *idx > vert_start as u32 {
                    *idx -= vert_count as u32;
                }
            }

            // Remove material data
            self.textures
                .drain(decal_face_start..(decal_face_start + decal_face_count));
            self.shaders
                .drain(decal_face_start..(decal_face_start + decal_face_count));

            // Update subsequent decals
            for d in self.decals.iter_mut().skip(idx + 1) {
                d.vertex_start -= vert_count;
                d.face_start -= decal_face_count;
            }

            self.decals.remove(idx);
            Ok(true)
        } else {
            Err(DecalError::InvalidId)
        }
    }

    fn decal_count(&self) -> usize {
        self.decals.len()
    }

    fn get_decal_id(&self, index: usize) -> Option<DecalId> {
        self.decals.get(index).map(|d| d.decal_id)
    }

    fn render(&self) {
        // Rendering is handled by the mesh system
        // For skin meshes, we use parent vertex indices that reference
        // the deformed vertices from the parent skinned mesh

        // In a complete implementation, this would:
        // 1. Fetch the current deformed vertices from the parent mesh
        // 2. Build geometry using parent_vertex_indices to reference them
        // 3. Use our stored tex_coords for texture mapping
        // 4. Queue to renderer with appropriate material
    }

    /// Get the decal mesh geometry references for rendering
    fn get_geometry_refs(&self) -> (&[u32], &[Vec2], &[u32]) {
        (&self.parent_vertex_indices, &self.tex_coords, &self.indices)
    }

    /// Get material information for a specific triangle
    fn get_triangle_material(&self, triangle_idx: usize) -> Option<(&str, &str)> {
        if triangle_idx < self.textures.len() {
            Some((&self.textures[triangle_idx], &self.shaders[triangle_idx]))
        } else {
            None
        }
    }
}

//------------------------------------------------------------------------------
// DECAL SYSTEM
// Main system for managing decals globally
//------------------------------------------------------------------------------

/// Global decal ID generator
/// C++ Reference: decalsys.cpp line 9, decalsys.h line 84
static DECAL_ID_COUNTER: Mutex<DecalId> = Mutex::new(0);

/// DecalSystem - main system for managing decals globally
/// C++ Reference: decalsys.h lines 47-85 (DecalSystemClass)
pub struct DecalSystem {
    /// All decal meshes in the system
    pub decal_meshes: Vec<Arc<Mutex<dyn DecalMesh>>>,
}

impl DecalSystem {
    /// Constructor
    /// C++ Reference: decalsys.cpp lines 27-29
    pub fn new() -> Self {
        Self {
            decal_meshes: Vec::new(),
        }
    }

    /// Generate a unique decal ID
    /// C++ Reference: decalsys.cpp lines 101-104 (Generate_Unique_Global_Decal_Id)
    pub fn generate_decal_id() -> DecalId {
        let mut counter = DECAL_ID_COUNTER.lock().unwrap();
        let id = *counter;
        *counter += 1;
        id
    }

    /// Create a new decal generator
    pub fn create_generator(
        &self,
        projector: Projector,
        material: DecalMaterial,
    ) -> DecalGenerator {
        let id = Self::generate_decal_id();
        DecalGenerator::new(id, projector, material)
    }

    /// Register a decal mesh
    pub fn register_decal_mesh(&mut self, mesh: Arc<Mutex<dyn DecalMesh>>) {
        self.decal_meshes.push(mesh);
    }

    /// Remove a decal from all meshes
    pub fn remove_decal(&mut self, decal_id: DecalId) {
        for mesh in &self.decal_meshes {
            let mut mesh = mesh.lock().unwrap();
            let _ = mesh.delete_decal(decal_id);
        }
    }
}

/// Global decal system instance
static DECAL_SYSTEM: OnceLock<Mutex<DecalSystem>> = OnceLock::new();

pub fn get_decal_system() -> &'static Mutex<DecalSystem> {
    DECAL_SYSTEM.get_or_init(|| Mutex::new(DecalSystem::new()))
}

pub fn init_decal_system() {
    let _ = get_decal_system();
}

pub fn shutdown_decal_system() {
    // System will be dropped automatically
}

//------------------------------------------------------------------------------
// MULTI FIXED POOL DECAL SYSTEM
// Advanced decal system with fixed-size pools
// C++ Reference: decalsys.h lines 177-264, decalsys.cpp lines 248-441
//------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct LogicalDecal {
    mesh_names: Vec<String>,
}

impl LogicalDecal {
    fn new() -> Self {
        Self {
            mesh_names: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.mesh_names.clear();
    }
}

pub struct DecalPool {
    slots: Vec<LogicalDecal>,
}

impl DecalPool {
    pub fn new(size: usize) -> Self {
        Self {
            slots: vec![LogicalDecal::new(); size],
        }
    }

    pub fn size(&self) -> usize {
        self.slots.len()
    }
}

/// MultiFixedPoolDecalSystem - manages multiple fixed-size decal pools
/// C++ Reference: decalsys.h lines 178-264, decalsys.cpp lines 248-441
pub struct MultiFixedPoolDecalSystem {
    pools: Vec<DecalPool>,
    current_pool_id: u32,
    current_slot_id: u32,
}

impl MultiFixedPoolDecalSystem {
    /// Constructor - create pools with specified sizes
    /// C++ Reference: decalsys.cpp lines 248-263
    pub fn new(pool_sizes: &[usize]) -> Self {
        let pools = pool_sizes
            .iter()
            .map(|&size| DecalPool::new(size))
            .collect();

        Self {
            pools,
            current_pool_id: 0,
            current_slot_id: 0,
        }
    }

    pub fn set_next_slot(&mut self, pool_id: u32, slot_id: u32) {
        self.current_pool_id = pool_id;
        self.current_slot_id = slot_id;
    }

    pub fn create_generator(
        &mut self,
        projector: Projector,
        material: DecalMaterial,
    ) -> DecalGenerator {
        // Clear the slot first
        self.clear_slot(self.current_pool_id, self.current_slot_id);

        // Generate ID from pool and slot
        let decal_id = Self::encode_decal_id(self.current_pool_id, self.current_slot_id);
        DecalGenerator::new(decal_id, projector, material)
    }

    pub fn clear_slot(&mut self, pool_id: u32, slot_id: u32) {
        if let Some(pool) = self.pools.get_mut(pool_id as usize) {
            if let Some(slot) = pool.slots.get_mut(slot_id as usize) {
                // Remove decals from all affected meshes (C++ decalsys.cpp lines 446-458)
                let decal_id = Self::encode_decal_id(pool_id, slot_id);

                // Get the mesh list from the slot before clearing
                let _mesh_names = slot.mesh_names.clone();

                // Clear the slot
                slot.clear();

                // Remove the decal from all affected meshes in the global decal system
                // This matches the C++ implementation where LogicalDecalClass::Clear
                // iterates through MeshList and calls Delete_Decal on each mesh
                let decal_system = get_decal_system();
                if let Ok(system) = decal_system.lock() {
                    // Iterate through all registered decal meshes
                    for mesh in &system.decal_meshes {
                        if let Ok(mut mesh_guard) = mesh.lock() {
                            // Try to delete the decal from this mesh
                            let _ = mesh_guard.delete_decal(decal_id);
                        }
                    }
                }
            }
        }
    }

    pub fn clear_pool(&mut self, pool_id: u32) {
        if let Some(pool) = self.pools.get_mut(pool_id as usize) {
            for slot in &mut pool.slots {
                slot.clear();
            }
        }
    }

    pub fn clear_all(&mut self) {
        for pool in &mut self.pools {
            for slot in &mut pool.slots {
                slot.clear();
            }
        }
    }

    /// Encode pool and slot IDs into a single decal ID
    /// C++ Reference: decalsys.h line 231 (encode_decal_id)
    /// Encoding: upper 16 bits = pool_id, lower 16 bits = slot_id
    fn encode_decal_id(pool_id: u32, slot_id: u32) -> DecalId {
        ((pool_id & 0xFFFF) << 16) | (slot_id & 0xFFFF)
    }

    /// Decode a decal ID into pool and slot IDs
    /// C++ Reference: decalsys.h line 232 (decode_decal_id)
    #[allow(dead_code)] // C++ parity
    fn decode_decal_id(decal_id: DecalId) -> (u32, u32) {
        let pool_id = (decal_id >> 16) & 0xFFFF;
        let slot_id = decal_id & 0xFFFF;
        (pool_id, slot_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plane_classification() {
        let plane = Plane::new(Vec3::Z, Vec3::ZERO);

        assert_eq!(plane.classify_point(Vec3::new(0.0, 0.0, 1.0)), 1);
        assert_eq!(plane.classify_point(Vec3::new(0.0, 0.0, -1.0)), -1);
        assert_eq!(plane.classify_point(Vec3::new(1.0, 1.0, 0.0)), 0);
    }

    #[test]
    fn test_polygon_clipping() {
        let mut poly = DecalPolygon::new();
        poly.add_vertex(DecalVertex::new(Vec3::new(-1.0, -1.0, 0.0), Vec3::Z));
        poly.add_vertex(DecalVertex::new(Vec3::new(1.0, -1.0, 0.0), Vec3::Z));
        poly.add_vertex(DecalVertex::new(Vec3::new(1.0, 1.0, 0.0), Vec3::Z));
        poly.add_vertex(DecalVertex::new(Vec3::new(-1.0, 1.0, 0.0), Vec3::Z));

        let plane = Plane::new(Vec3::X, Vec3::ZERO);
        let clipped = poly.clip(&plane);

        assert!(clipped.vertices.len() >= 3);
    }

    #[test]
    fn test_decal_id_encoding() {
        let pool_id = 5;
        let slot_id = 42;
        let encoded = MultiFixedPoolDecalSystem::encode_decal_id(pool_id, slot_id);
        let (decoded_pool, decoded_slot) = MultiFixedPoolDecalSystem::decode_decal_id(encoded);

        assert_eq!(pool_id, decoded_pool);
        assert_eq!(slot_id, decoded_slot);
    }

    #[test]
    fn test_decal_system_init() {
        init_decal_system();
        let system = get_decal_system();
        let _guard = system.lock().unwrap();
        // System should be initialized
    }

    #[test]
    fn test_rigid_decal_mesh_creation() {
        let mut mesh = RigidDecalMesh::new("test_mesh".to_string());

        // Set up parent geometry (simple triangle)
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        let normals = vec![Vec3::Z, Vec3::Z, Vec3::Z];
        let indices = vec![0, 1, 2];

        mesh.set_parent_geometry(vertices, normals, indices);

        // Create a simple projector
        let projector = Projector::new(
            Mat4::IDENTITY,
            Mat4::orthographic_rh(-1.0, 1.0, -1.0, 1.0, 0.0, 10.0),
            OBBox::new(Vec3::ZERO, Mat4::IDENTITY, Vec3::ONE),
        );

        let material = DecalMaterial::default();
        let mut generator = DecalGenerator::new(1, projector, material);

        // Create decal on the triangle
        let result = mesh.create_decal(&mut generator, &[0]);

        assert!(result.is_ok());
        assert!(result.unwrap()); // Should have created geometry

        // Verify decal was created
        assert_eq!(mesh.decal_count(), 1);
        assert!(mesh.vertices.len() >= 3); // At least one triangle
    }

    #[test]
    fn test_decal_clipping() {
        let mut poly = DecalPolygon::new();

        // Create a quad
        poly.add_vertex(DecalVertex::new(Vec3::new(-1.0, -1.0, 0.0), Vec3::Z));
        poly.add_vertex(DecalVertex::new(Vec3::new(1.0, -1.0, 0.0), Vec3::Z));
        poly.add_vertex(DecalVertex::new(Vec3::new(1.0, 1.0, 0.0), Vec3::Z));
        poly.add_vertex(DecalVertex::new(Vec3::new(-1.0, 1.0, 0.0), Vec3::Z));

        // Clip against a plane that cuts through the middle
        let plane = Plane::new(Vec3::X, Vec3::ZERO);
        let clipped = poly.clip(&plane);

        // Should still have vertices after clipping
        assert!(clipped.vertices.len() >= 3);
        assert!(clipped.vertices.len() <= poly.vertices.len() + 2); // May add intersection vertices
    }
}
