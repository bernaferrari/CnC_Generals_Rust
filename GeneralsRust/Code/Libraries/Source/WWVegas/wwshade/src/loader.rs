//! Shader Mesh Loader
//!
//! This module provides prototype loaders for shader meshes and legacy mesh conversion.
//! It handles loading 3D mesh data from W3D format and converting legacy meshes to
//! the modern shader-based format.

use async_trait::async_trait;
use std::sync::Arc;

use crate::error::{ShdError, ShdResult};
use crate::interface::RenderInfo;

/// Chunk types for W3D file format
mod chunk_types {
    pub const W3D_CHUNK_SHDMESH: u32 = 0x0000C000;
    pub const W3D_CHUNK_MESH: u32 = 0x00000000;
    pub const W3D_CHUNK_VERTICES: u32 = 0x00000002;
    pub const W3D_CHUNK_TRIANGLES: u32 = 0x00000020;
    pub const W3D_CHUNK_MATERIALS: u32 = 0x00000030;
}

/// 3D vertex structure
#[derive(Debug, Clone, PartialEq)]
pub struct Vertex3D {
    pub position: glam::Vec3,
    pub normal: glam::Vec3,
    pub uv: glam::Vec2,
    pub color: u32,
    pub tangent: Option<glam::Vec3>,
    pub binormal: Option<glam::Vec3>,
}

impl Default for Vertex3D {
    fn default() -> Self {
        Self {
            position: glam::Vec3::ZERO,
            normal: glam::Vec3::Y, // Default up normal
            uv: glam::Vec2::ZERO,
            color: 0xFFFFFFFF, // White
            tangent: None,
            binormal: None,
        }
    }
}

/// Triangle index structure
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TriIndex {
    pub indices: [u16; 3],
}

impl TriIndex {
    pub fn new(a: u16, b: u16, c: u16) -> Self {
        Self { indices: [a, b, c] }
    }
}

/// Material properties for mesh surfaces
#[derive(Debug, Clone)]
pub struct MaterialProperties {
    pub name: String,
    pub diffuse_color: glam::Vec4,
    pub specular_color: glam::Vec3,
    pub specular_power: f32,
    pub opacity: f32,
    pub texture_name: Option<String>,
    pub normal_map_name: Option<String>,
    pub surface_type: i32,
}

impl Default for MaterialProperties {
    fn default() -> Self {
        Self {
            name: "DefaultMaterial".to_string(),
            diffuse_color: glam::Vec4::ONE,
            specular_color: glam::Vec3::splat(0.2),
            specular_power: 32.0,
            opacity: 1.0,
            texture_name: None,
            normal_map_name: None,
            surface_type: 0,
        }
    }
}

/// Mesh geometry data
#[derive(Debug, Clone, Default)]
pub struct MeshGeometry {
    pub vertices: Vec<Vertex3D>,
    pub indices: Vec<TriIndex>,
    pub material: MaterialProperties,
    pub name: String,
    pub transform: glam::Mat4,
    pub bounding_box: Option<(glam::Vec3, glam::Vec3)>, // min, max
}

impl MeshGeometry {
    /// Create a new empty mesh geometry
    pub fn new(name: String) -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            material: MaterialProperties::default(),
            name,
            transform: glam::Mat4::IDENTITY,
            bounding_box: None,
        }
    }

    /// Calculate bounding box from vertices
    pub fn calculate_bounding_box(&mut self) {
        if self.vertices.is_empty() {
            self.bounding_box = None;
            return;
        }

        let mut min = self.vertices[0].position;
        let mut max = self.vertices[0].position;

        for vertex in &self.vertices[1..] {
            min = min.min(vertex.position);
            max = max.max(vertex.position);
        }

        self.bounding_box = Some((min, max));
    }

    /// Calculate vertex normals if they're missing
    pub fn calculate_normals(&mut self) {
        // Reset all normals to zero
        for vertex in &mut self.vertices {
            vertex.normal = glam::Vec3::ZERO;
        }

        // Accumulate face normals
        for triangle in &self.indices {
            let v0 = &self.vertices[triangle.indices[0] as usize];
            let v1 = &self.vertices[triangle.indices[1] as usize];
            let v2 = &self.vertices[triangle.indices[2] as usize];

            let edge1 = v1.position - v0.position;
            let edge2 = v2.position - v0.position;
            let face_normal = edge1.cross(edge2);

            // Add to each vertex normal
            for &index in &triangle.indices {
                self.vertices[index as usize].normal += face_normal;
            }
        }

        // Normalize all normals
        for vertex in &mut self.vertices {
            vertex.normal = vertex.normal.normalize_or_zero();
        }
    }

    /// Calculate tangent space vectors for bump mapping
    pub fn calculate_tangent_space(&mut self) {
        let vertex_count = self.vertices.len();
        let mut tan1 = vec![glam::Vec3::ZERO; vertex_count];
        let mut tan2 = vec![glam::Vec3::ZERO; vertex_count];

        // Calculate tangent and binormal for each triangle
        for triangle in &self.indices {
            let i1 = triangle.indices[0] as usize;
            let i2 = triangle.indices[1] as usize;
            let i3 = triangle.indices[2] as usize;

            let v1 = &self.vertices[i1];
            let v2 = &self.vertices[i2];
            let v3 = &self.vertices[i3];

            let x1 = v2.position.x - v1.position.x;
            let x2 = v3.position.x - v1.position.x;
            let y1 = v2.position.y - v1.position.y;
            let y2 = v3.position.y - v1.position.y;
            let z1 = v2.position.z - v1.position.z;
            let z2 = v3.position.z - v1.position.z;

            let s1 = v2.uv.x - v1.uv.x;
            let s2 = v3.uv.x - v1.uv.x;
            let t1 = v2.uv.y - v1.uv.y;
            let t2 = v3.uv.y - v1.uv.y;

            let r = 1.0 / (s1 * t2 - s2 * t1);
            if r.is_finite() {
                let sdir = glam::Vec3::new(
                    (t2 * x1 - t1 * x2) * r,
                    (t2 * y1 - t1 * y2) * r,
                    (t2 * z1 - t1 * z2) * r,
                );
                let tdir = glam::Vec3::new(
                    (s1 * x2 - s2 * x1) * r,
                    (s1 * y2 - s2 * y1) * r,
                    (s1 * z2 - s2 * z1) * r,
                );

                tan1[i1] += sdir;
                tan1[i2] += sdir;
                tan1[i3] += sdir;

                tan2[i1] += tdir;
                tan2[i2] += tdir;
                tan2[i3] += tdir;
            }
        }

        // Calculate final tangent and binormal for each vertex
        for (i, vertex) in self.vertices.iter_mut().enumerate() {
            let n = vertex.normal;
            let t = tan1[i];

            // Gram-Schmidt orthogonalize
            let tangent = (t - n * n.dot(t)).normalize_or_zero();

            // Calculate handedness
            let handedness = if n.cross(t).dot(tan2[i]) < 0.0 {
                -1.0
            } else {
                1.0
            };
            let binormal = n.cross(tangent) * handedness;

            vertex.tangent = Some(tangent);
            vertex.binormal = Some(binormal);
        }
    }

    /// Validate mesh geometry
    pub fn validate(&self) -> ShdResult<()> {
        if self.vertices.is_empty() {
            return Err(ShdError::InvalidConfig("Mesh has no vertices".to_string()));
        }

        if self.indices.is_empty() {
            return Err(ShdError::InvalidConfig("Mesh has no triangles".to_string()));
        }

        // Check that all indices are valid
        let vertex_count = self.vertices.len() as u16;
        for (i, triangle) in self.indices.iter().enumerate() {
            for &index in &triangle.indices {
                if index >= vertex_count {
                    return Err(ShdError::InvalidConfig(format!(
                        "Triangle {} has invalid vertex index {}",
                        i, index
                    )));
                }
            }
        }

        Ok(())
    }
}

/// Prototype class for 3D objects
#[derive(Debug)]
pub struct PrototypeClass {
    pub name: String,
    pub mesh_data: Arc<MeshGeometry>,
    pub render_info: RenderInfo,
}

impl PrototypeClass {
    pub fn new(name: String, mesh_data: MeshGeometry) -> Self {
        Self {
            name,
            mesh_data: Arc::new(mesh_data),
            render_info: RenderInfo::default(),
        }
    }
}

/// Chunk-based data loading interface
#[async_trait]
pub trait ChunkLoader: Send + Sync {
    /// Load data from a chunk
    async fn load_chunk(&mut self, chunk_type: u32, data: &[u8]) -> ShdResult<()>;

    /// Get the loaded mesh geometry
    fn get_mesh(&self) -> ShdResult<MeshGeometry>;
}

/// Basic chunk loader implementation
#[derive(Debug)]
pub struct BasicChunkLoader {
    mesh: MeshGeometry,
    vertices_loaded: bool,
    triangles_loaded: bool,
}

#[async_trait]
impl ChunkLoader for BasicChunkLoader {
    async fn load_chunk(&mut self, chunk_type: u32, data: &[u8]) -> ShdResult<()> {
        match chunk_type {
            chunk_types::W3D_CHUNK_VERTICES => {
                self.load_vertices(data)?;
                self.vertices_loaded = true;
            }
            chunk_types::W3D_CHUNK_TRIANGLES => {
                self.load_triangles(data)?;
                self.triangles_loaded = true;
            }
            chunk_types::W3D_CHUNK_MATERIALS => {
                self.load_materials(data)?;
            }
            _ => {
                // Skip unknown chunks
            }
        }
        Ok(())
    }

    fn get_mesh(&self) -> ShdResult<MeshGeometry> {
        if !self.vertices_loaded {
            return Err(ShdError::InvalidConfig("No vertices loaded".to_string()));
        }

        if !self.triangles_loaded {
            return Err(ShdError::InvalidConfig("No triangles loaded".to_string()));
        }

        let mesh = self.mesh.clone();
        mesh.validate()?;
        Ok(mesh)
    }
}

impl BasicChunkLoader {
    pub fn new(name: String) -> Self {
        Self {
            mesh: MeshGeometry::new(name),
            vertices_loaded: false,
            triangles_loaded: false,
        }
    }

    fn load_vertices(&mut self, data: &[u8]) -> ShdResult<()> {
        if data.len() < 4 {
            return Err(ShdError::FormatError(
                "Invalid vertex chunk size".to_string(),
            ));
        }

        let vertex_count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let expected_size = 4 + vertex_count * 32; // 32 bytes per vertex (pos + normal + uv)

        if data.len() < expected_size {
            return Err(ShdError::FormatError(format!(
                "Vertex chunk too small: expected {}, got {}",
                expected_size,
                data.len()
            )));
        }

        self.mesh.vertices.clear();
        self.mesh.vertices.reserve(vertex_count);

        let mut offset = 4;
        for _ in 0..vertex_count {
            let pos_x = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let pos_y = f32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let pos_z = f32::from_le_bytes([
                data[offset + 8],
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
            ]);

            let norm_x = f32::from_le_bytes([
                data[offset + 12],
                data[offset + 13],
                data[offset + 14],
                data[offset + 15],
            ]);
            let norm_y = f32::from_le_bytes([
                data[offset + 16],
                data[offset + 17],
                data[offset + 18],
                data[offset + 19],
            ]);
            let norm_z = f32::from_le_bytes([
                data[offset + 20],
                data[offset + 21],
                data[offset + 22],
                data[offset + 23],
            ]);

            let uv_u = f32::from_le_bytes([
                data[offset + 24],
                data[offset + 25],
                data[offset + 26],
                data[offset + 27],
            ]);
            let uv_v = f32::from_le_bytes([
                data[offset + 28],
                data[offset + 29],
                data[offset + 30],
                data[offset + 31],
            ]);

            self.mesh.vertices.push(Vertex3D {
                position: glam::Vec3::new(pos_x, pos_y, pos_z),
                normal: glam::Vec3::new(norm_x, norm_y, norm_z),
                uv: glam::Vec2::new(uv_u, uv_v),
                color: 0xFFFFFFFF,
                tangent: None,
                binormal: None,
            });

            offset += 32;
        }

        Ok(())
    }

    fn load_triangles(&mut self, data: &[u8]) -> ShdResult<()> {
        if data.len() < 4 {
            return Err(ShdError::FormatError(
                "Invalid triangle chunk size".to_string(),
            ));
        }

        let triangle_count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let expected_size = 4 + triangle_count * 6; // 6 bytes per triangle (3 u16 indices)

        if data.len() < expected_size {
            return Err(ShdError::FormatError(format!(
                "Triangle chunk too small: expected {}, got {}",
                expected_size,
                data.len()
            )));
        }

        self.mesh.indices.clear();
        self.mesh.indices.reserve(triangle_count);

        let mut offset = 4;
        for _ in 0..triangle_count {
            let a = u16::from_le_bytes([data[offset], data[offset + 1]]);
            let b = u16::from_le_bytes([data[offset + 2], data[offset + 3]]);
            let c = u16::from_le_bytes([data[offset + 4], data[offset + 5]]);

            self.mesh.indices.push(TriIndex::new(a, b, c));
            offset += 6;
        }

        Ok(())
    }

    fn load_materials(&mut self, data: &[u8]) -> ShdResult<()> {
        // Simplified material loading - in a real implementation this would
        // parse a more complex material format
        if !data.is_empty() {
            if let Ok(material_name) = std::str::from_utf8(data) {
                self.mesh.material.name = material_name.to_string();
            }
        }
        Ok(())
    }
}

/// Shader mesh loader for modern shader-based meshes
#[derive(Debug)]
pub struct ShdMeshLoader {
    _loader_type: String,
}

impl ShdMeshLoader {
    /// Create a new shader mesh loader
    pub fn new() -> Self {
        Self {
            _loader_type: "ShdMeshLoader".to_string(),
        }
    }

    /// Get the chunk type this loader handles
    pub fn chunk_type(&self) -> u32 {
        chunk_types::W3D_CHUNK_SHDMESH
    }

    /// Load a W3D mesh file asynchronously
    pub async fn load_w3d(&self, data: &[u8]) -> ShdResult<Arc<PrototypeClass>> {
        if data.len() < 8 {
            return Err(ShdError::FormatError("W3D file too small".to_string()));
        }

        let mut loader = BasicChunkLoader::new("ShdMesh".to_string());
        let mut offset = 0;

        // Parse chunks
        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]) as usize;

            offset += 8;

            if offset + chunk_size > data.len() {
                return Err(ShdError::FormatError(
                    "Chunk extends beyond file".to_string(),
                ));
            }

            let chunk_data = &data[offset..offset + chunk_size];
            loader.load_chunk(chunk_type, chunk_data).await?;

            offset += chunk_size;
        }

        let mut mesh = loader.get_mesh()?;

        // Post-process the mesh
        mesh.calculate_bounding_box();
        mesh.calculate_normals();
        mesh.calculate_tangent_space();

        let prototype = PrototypeClass::new(format!("{}_prototype", mesh.name), mesh);

        Ok(Arc::new(prototype))
    }
}

impl Default for ShdMeshLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Legacy mesh loader for converting old format meshes
#[derive(Debug)]
pub struct ShdMeshLegacyLoader {
    _loader_type: String,
}

impl ShdMeshLegacyLoader {
    /// Create a new legacy mesh loader
    pub fn new() -> Self {
        Self {
            _loader_type: "ShdMeshLegacyLoader".to_string(),
        }
    }

    /// Get the chunk type this loader handles
    pub fn chunk_type(&self) -> u32 {
        chunk_types::W3D_CHUNK_MESH
    }

    /// Load and convert a legacy W3D mesh file
    pub async fn load_w3d(&self, data: &[u8]) -> ShdResult<Arc<PrototypeClass>> {
        // First load as a regular mesh
        let legacy_loader = ShdMeshLoader::new();
        let prototype = legacy_loader.load_w3d(data).await?;

        // Convert to shader mesh format
        let mut mesh = prototype.mesh_data.as_ref().clone();

        // Legacy meshes might need additional processing
        // For example, generating default materials or shaders
        if mesh.material.name == "DefaultMaterial" {
            mesh.material.name = "LegacyConverted".to_string();
            mesh.material.surface_type = 1; // Mark as legacy converted
        }

        // Ensure we have all required vertex attributes
        if !mesh.vertices.iter().any(|v| v.tangent.is_some()) {
            mesh.calculate_tangent_space();
        }

        let converted_prototype = PrototypeClass::new(format!("{}_legacy", mesh.name), mesh);

        Ok(Arc::new(converted_prototype))
    }
}

impl Default for ShdMeshLegacyLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Global loader instances
static SHD_MESH_LOADER: once_cell::sync::Lazy<ShdMeshLoader> =
    once_cell::sync::Lazy::new(|| ShdMeshLoader::new());

static SHD_MESH_LEGACY_LOADER: once_cell::sync::Lazy<ShdMeshLegacyLoader> =
    once_cell::sync::Lazy::new(|| ShdMeshLegacyLoader::new());

/// Get the global shader mesh loader instance
pub fn get_shd_mesh_loader() -> &'static ShdMeshLoader {
    &*SHD_MESH_LOADER
}

/// Get the global legacy mesh loader instance  
pub fn get_shd_mesh_legacy_loader() -> &'static ShdMeshLegacyLoader {
    &*SHD_MESH_LEGACY_LOADER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex3d_creation() {
        let vertex = Vertex3D::default();
        assert_eq!(vertex.position, glam::Vec3::ZERO);
        assert_eq!(vertex.normal, glam::Vec3::Y);
        assert_eq!(vertex.uv, glam::Vec2::ZERO);
        assert_eq!(vertex.color, 0xFFFFFFFF);
        assert!(vertex.tangent.is_none());
        assert!(vertex.binormal.is_none());
    }

    #[test]
    fn test_tri_index_creation() {
        let triangle = TriIndex::new(0, 1, 2);
        assert_eq!(triangle.indices, [0, 1, 2]);
    }

    #[test]
    fn test_mesh_geometry_creation() {
        let mesh = MeshGeometry::new("TestMesh".to_string());
        assert_eq!(mesh.name, "TestMesh");
        assert!(mesh.vertices.is_empty());
        assert!(mesh.indices.is_empty());
        assert_eq!(mesh.transform, glam::Mat4::IDENTITY);
        assert!(mesh.bounding_box.is_none());
    }

    #[test]
    fn test_mesh_validation() {
        let mesh = MeshGeometry::new("Empty".to_string());
        assert!(mesh.validate().is_err());

        let mut mesh_with_vertices = MeshGeometry::new("WithVertices".to_string());
        mesh_with_vertices.vertices.push(Vertex3D::default());
        assert!(mesh_with_vertices.validate().is_err()); // No triangles

        mesh_with_vertices.indices.push(TriIndex::new(0, 0, 0));
        assert!(mesh_with_vertices.validate().is_ok());
    }

    #[test]
    fn test_bounding_box_calculation() {
        let mut mesh = MeshGeometry::new("BoundingTest".to_string());

        mesh.vertices.push(Vertex3D {
            position: glam::Vec3::new(-1.0, -2.0, -3.0),
            ..Default::default()
        });
        mesh.vertices.push(Vertex3D {
            position: glam::Vec3::new(1.0, 2.0, 3.0),
            ..Default::default()
        });
        mesh.vertices.push(Vertex3D {
            position: glam::Vec3::new(0.5, 0.5, 0.5),
            ..Default::default()
        });

        mesh.calculate_bounding_box();

        let (min, max) = mesh.bounding_box.unwrap();
        assert_eq!(min, glam::Vec3::new(-1.0, -2.0, -3.0));
        assert_eq!(max, glam::Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_normal_calculation() {
        let mut mesh = MeshGeometry::new("NormalTest".to_string());

        // Create a simple triangle
        mesh.vertices.push(Vertex3D {
            position: glam::Vec3::new(0.0, 0.0, 0.0),
            ..Default::default()
        });
        mesh.vertices.push(Vertex3D {
            position: glam::Vec3::new(1.0, 0.0, 0.0),
            ..Default::default()
        });
        mesh.vertices.push(Vertex3D {
            position: glam::Vec3::new(0.0, 1.0, 0.0),
            ..Default::default()
        });
        mesh.indices.push(TriIndex::new(0, 1, 2));

        mesh.calculate_normals();

        // All normals should point in the positive Z direction
        for vertex in &mesh.vertices {
            assert!(
                (vertex.normal.z - 1.0).abs() < 0.001,
                "Normal should be (0, 0, 1), got {:?}",
                vertex.normal
            );
        }
    }

    #[tokio::test]
    async fn test_basic_chunk_loader() {
        let mut loader = BasicChunkLoader::new("Test".to_string());

        // Create test vertex data
        let mut vertex_data = vec![0u8; 4 + 32]; // 1 vertex
        vertex_data[0] = 1; // vertex count = 1
                            // Position (0, 0, 0)
                            // Normal (0, 1, 0)
        vertex_data[16] = 0;
        vertex_data[17] = 0;
        vertex_data[18] = 128;
        vertex_data[19] = 63; // 1.0 as f32

        // Create test triangle data
        let mut triangle_data = vec![0u8; 4 + 6]; // 1 triangle
        triangle_data[0] = 1; // triangle count = 1
                              // Indices (0, 0, 0)

        loader
            .load_chunk(chunk_types::W3D_CHUNK_VERTICES, &vertex_data)
            .await
            .unwrap();
        loader
            .load_chunk(chunk_types::W3D_CHUNK_TRIANGLES, &triangle_data)
            .await
            .unwrap();

        let mesh = loader.get_mesh().unwrap();
        assert_eq!(mesh.vertices.len(), 1);
        assert_eq!(mesh.indices.len(), 1);
    }

    #[test]
    fn test_loader_creation() {
        let loader = ShdMeshLoader::new();
        assert_eq!(loader.chunk_type(), chunk_types::W3D_CHUNK_SHDMESH);

        let legacy_loader = ShdMeshLegacyLoader::new();
        assert_eq!(legacy_loader.chunk_type(), chunk_types::W3D_CHUNK_MESH);
    }

    #[test]
    fn test_global_loader_instances() {
        let loader1 = get_shd_mesh_loader();
        let loader2 = get_shd_mesh_loader();
        assert_eq!(loader1.chunk_type(), loader2.chunk_type());

        let legacy1 = get_shd_mesh_legacy_loader();
        let legacy2 = get_shd_mesh_legacy_loader();
        assert_eq!(legacy1.chunk_type(), legacy2.chunk_type());
    }
}
