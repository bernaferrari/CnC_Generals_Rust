//! Mesh File Format Loading
//!
//! This module provides functionality for loading mesh data from various
//! file formats including OBJ, STL, PLY, and custom WW3D formats.

use crate::mesh_geometry::{MeshMaterial, MeshTriangle, MeshVertex};
use crate::*;
use binrw::BinRead;
use glam::{Vec2, Vec3};
use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;
use ww3d_core::{
    W3DChunkType, W3dChunkHeader, W3dMeshHeader3Struct, W3dTexCoordStruct, W3dTriangleStruct,
    W3dVectorStruct,
};

/// Mesh loader result
#[derive(Debug)]
pub enum MeshLoadResult {
    Success(MeshGeometry),
    Error(String),
}

/// Supported mesh file formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MeshFormat {
    Obj,
    Stl,
    Ply,
    W3d,
}

/// Mesh loader trait
pub trait MeshLoader {
    fn load_from_file(&self, path: &Path) -> MeshLoadResult;
    fn load_from_bytes(&self, data: &[u8]) -> MeshLoadResult;
    fn supported_formats(&self) -> Vec<MeshFormat>;
}

/// OBJ file loader
pub struct ObjLoader;

impl ObjLoader {
    pub fn new() -> Self {
        Self
    }
}

impl MeshLoader for ObjLoader {
    fn load_from_file(&self, path: &Path) -> MeshLoadResult {
        match fs::read_to_string(path) {
            Ok(content) => self.load_from_string(&content),
            Err(e) => MeshLoadResult::Error(format!("Failed to read file: {}", e)),
        }
    }

    fn load_from_bytes(&self, data: &[u8]) -> MeshLoadResult {
        match std::str::from_utf8(data) {
            Ok(content) => self.load_from_string(content),
            Err(e) => MeshLoadResult::Error(format!("Invalid UTF-8: {}", e)),
        }
    }

    fn supported_formats(&self) -> Vec<MeshFormat> {
        vec![MeshFormat::Obj]
    }
}

impl ObjLoader {
    fn load_from_string(&self, content: &str) -> MeshLoadResult {
        let mut vertices = Vec::new();
        let mut normals = Vec::new();
        let mut tex_coords = Vec::new();
        let mut faces = Vec::new();
        let mut materials = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            match parts[0] {
                "v" => {
                    if parts.len() >= 4 {
                        let x = parts[1].parse::<f32>().unwrap_or(0.0);
                        let y = parts[2].parse::<f32>().unwrap_or(0.0);
                        let z = parts[3].parse::<f32>().unwrap_or(0.0);
                        vertices.push(Vec3::new(x, y, z));
                    }
                }
                "vn" => {
                    if parts.len() >= 4 {
                        let x = parts[1].parse::<f32>().unwrap_or(0.0);
                        let y = parts[2].parse::<f32>().unwrap_or(0.0);
                        let z = parts[3].parse::<f32>().unwrap_or(0.0);
                        normals.push(Vec3::new(x, y, z));
                    }
                }
                "vt" => {
                    if parts.len() >= 3 {
                        let u = parts[1].parse::<f32>().unwrap_or(0.0);
                        let v = parts[2].parse::<f32>().unwrap_or(0.0);
                        tex_coords.push(Vec2::new(u, v));
                    }
                }
                "f" => {
                    if parts.len() >= 4 {
                        let mut face_indices = Vec::new();
                        for part in &parts[1..] {
                            let indices: Vec<&str> = part.split('/').collect();
                            if !indices.is_empty() {
                                if let Ok(vertex_idx) = indices[0].parse::<i32>() {
                                    face_indices.push(vertex_idx);
                                }
                            }
                        }
                        if face_indices.len() >= 3 {
                            faces.push(face_indices);
                        }
                    }
                }
                "mtllib" => {
                    // Handle material library (simplified)
                    if parts.len() >= 2 {
                        materials.push(MeshMaterial::new(parts[1]));
                    }
                }
                _ => {} // Ignore other directives
            }
        }

        // Convert to mesh geometry
        let mut mesh = MeshGeometry::new();
        mesh.materials = materials;

        // Create vertices with default normals and tex coords if not provided
        for i in 0..vertices.len() {
            let normal = if i < normals.len() {
                normals[i]
            } else {
                Vec3::Y
            };
            let tex_coord = if i < tex_coords.len() {
                tex_coords[i]
            } else {
                Vec2::ZERO
            };
            mesh.add_vertex(MeshVertex::new(vertices[i], normal, tex_coord));
        }

        // Create triangles from faces
        for face in faces {
            if face.len() >= 3 {
                // Convert from 1-based to 0-based indexing
                let i0 = (face[0] - 1) as u32;
                let i1 = (face[1] - 1) as u32;
                let i2 = (face[2] - 1) as u32;

                if (i0 as usize) < mesh.vertices.len()
                    && (i1 as usize) < mesh.vertices.len()
                    && (i2 as usize) < mesh.vertices.len()
                {
                    mesh.add_triangle(MeshTriangle::new(i0, i1, i2, 0));
                }

                // Handle quad faces
                if face.len() >= 4 {
                    let i3 = (face[3] - 1) as u32;
                    if (i3 as usize) < mesh.vertices.len() {
                        mesh.add_triangle(MeshTriangle::new(i0, i2, i3, 0));
                    }
                }
            }
        }

        mesh.compute_normals();
        mesh.compute_plane_equations();
        mesh.update_bounds();

        MeshLoadResult::Success(mesh)
    }
}

/// STL file loader (binary and ASCII)
pub struct StlLoader;

impl StlLoader {
    pub fn new() -> Self {
        Self
    }
}

impl MeshLoader for StlLoader {
    fn load_from_file(&self, path: &Path) -> MeshLoadResult {
        match fs::read(path) {
            Ok(data) => self.load_from_bytes(&data),
            Err(e) => MeshLoadResult::Error(format!("Failed to read file: {}", e)),
        }
    }

    fn load_from_bytes(&self, data: &[u8]) -> MeshLoadResult {
        // Try binary format first
        if data.len() >= 84 && self.is_binary_stl(data) {
            self.load_binary_stl(data)
        } else {
            // Try ASCII format
            match std::str::from_utf8(data) {
                Ok(content) => self.load_ascii_stl(content),
                Err(_) => MeshLoadResult::Error("Invalid STL file format".to_string()),
            }
        }
    }

    fn supported_formats(&self) -> Vec<MeshFormat> {
        vec![MeshFormat::Stl]
    }
}

impl StlLoader {
    fn is_binary_stl(&self, data: &[u8]) -> bool {
        if data.len() < 84 {
            return false;
        }

        // Binary STL starts with 80 bytes header + 4 bytes triangle count
        let triangle_count = u32::from_le_bytes([data[80], data[81], data[82], data[83]]);
        let expected_size = 84 + triangle_count as usize * 50; // 50 bytes per triangle
        data.len() == expected_size
    }

    fn load_binary_stl(&self, data: &[u8]) -> MeshLoadResult {
        if data.len() < 84 {
            return MeshLoadResult::Error("Binary STL file too small".to_string());
        }

        let triangle_count = u32::from_le_bytes([data[80], data[81], data[82], data[83]]);
        let mut mesh = MeshGeometry::new();
        let mut offset = 84;

        for _ in 0..triangle_count {
            if offset + 50 > data.len() {
                return MeshLoadResult::Error("Binary STL file truncated".to_string());
            }

            // Skip normal (12 bytes)
            offset += 12;

            // Read three vertices (36 bytes)
            for _ in 0..3 {
                let x = f32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                let y = f32::from_le_bytes([
                    data[offset + 4],
                    data[offset + 5],
                    data[offset + 6],
                    data[offset + 7],
                ]);
                let z = f32::from_le_bytes([
                    data[offset + 8],
                    data[offset + 9],
                    data[offset + 10],
                    data[offset + 11],
                ]);

                let position = Vec3::new(x, y, z);
                let vertex = MeshVertex::new(position, Vec3::Y, Vec2::ZERO); // Default normal and tex coords
                mesh.add_vertex(vertex);

                offset += 12;
            }

            // Skip attribute byte count (2 bytes)
            offset += 2;
        }

        // Create triangles
        for i in 0..triangle_count {
            let base_index = (i * 3) as u32;
            mesh.add_triangle(MeshTriangle::new(
                base_index,
                base_index + 1,
                base_index + 2,
                0,
            ));
        }

        mesh.compute_normals();
        mesh.compute_plane_equations();
        mesh.update_bounds();

        MeshLoadResult::Success(mesh)
    }

    fn load_ascii_stl(&self, content: &str) -> MeshLoadResult {
        let mut mesh = MeshGeometry::new();
        let mut current_normal = Vec3::Y;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            match parts[0] {
                "facet" => {
                    if parts.len() >= 5 && parts[1] == "normal" {
                        if let (Ok(nx), Ok(ny), Ok(nz)) = (
                            parts[2].parse::<f32>(),
                            parts[3].parse::<f32>(),
                            parts[4].parse::<f32>(),
                        ) {
                            current_normal = Vec3::new(nx, ny, nz);
                        }
                    }
                }
                "vertex" => {
                    if parts.len() >= 4 {
                        if let (Ok(x), Ok(y), Ok(z)) = (
                            parts[1].parse::<f32>(),
                            parts[2].parse::<f32>(),
                            parts[3].parse::<f32>(),
                        ) {
                            let position = Vec3::new(x, y, z);
                            let vertex = MeshVertex::new(position, current_normal, Vec2::ZERO);
                            mesh.add_vertex(vertex);
                        }
                    }
                }
                "endfacet" => {
                    // Triangle complete, add it
                    let vertex_count = mesh.vertices.len();
                    if vertex_count >= 3 {
                        let base_index = (vertex_count - 3) as u32;
                        mesh.add_triangle(MeshTriangle::new(
                            base_index,
                            base_index + 1,
                            base_index + 2,
                            0,
                        ));
                    }
                }
                _ => {}
            }
        }

        mesh.update_bounds();
        MeshLoadResult::Success(mesh)
    }
}

/// PLY file loader
pub struct PlyLoader;

impl PlyLoader {
    pub fn new() -> Self {
        Self
    }
}

impl MeshLoader for PlyLoader {
    fn load_from_file(&self, path: &Path) -> MeshLoadResult {
        match fs::read_to_string(path) {
            Ok(content) => self.load_from_string(&content),
            Err(e) => MeshLoadResult::Error(format!("Failed to read file: {}", e)),
        }
    }

    fn load_from_bytes(&self, data: &[u8]) -> MeshLoadResult {
        match std::str::from_utf8(data) {
            Ok(content) => self.load_from_string(content),
            Err(e) => MeshLoadResult::Error(format!("Invalid UTF-8: {}", e)),
        }
    }

    fn supported_formats(&self) -> Vec<MeshFormat> {
        vec![MeshFormat::Ply]
    }
}

impl PlyLoader {
    fn load_from_string(&self, content: &str) -> MeshLoadResult {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        let mut in_header = true;
        let mut vertex_count = 0;
        let mut face_count = 0;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if in_header {
                if line == "end_header" {
                    in_header = false;
                } else if line.starts_with("element vertex") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        vertex_count = parts[2].parse().unwrap_or(0);
                    }
                } else if line.starts_with("element face") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        face_count = parts[2].parse().unwrap_or(0);
                    }
                }
            } else {
                // Parse data
                let parts: Vec<f32> = line
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();

                if vertices.len() < vertex_count && parts.len() >= 3 {
                    let position = Vec3::new(parts[0], parts[1], parts[2]);
                    vertices.push(position);
                } else if faces.len() < face_count && parts.len() >= 4 {
                    // Skip vertex count, take vertex indices
                    let indices: Vec<u32> = parts[1..].iter().map(|&x| x as u32).collect();
                    faces.push(indices);
                }
            }
        }

        // Create mesh
        let mut mesh = MeshGeometry::new();

        // Add vertices
        for position in vertices {
            let vertex = MeshVertex::new(position, Vec3::Y, Vec2::ZERO);
            mesh.add_vertex(vertex);
        }

        // Add triangles
        for face in faces {
            if face.len() >= 3 {
                mesh.add_triangle(MeshTriangle::new(face[0], face[1], face[2], 0));

                // Handle quad faces
                if face.len() >= 4 {
                    mesh.add_triangle(MeshTriangle::new(face[0], face[2], face[3], 0));
                }
            }
        }

        mesh.compute_normals();
        mesh.compute_plane_equations();
        mesh.update_bounds();

        MeshLoadResult::Success(mesh)
    }
}

/// WW3D format loader
pub struct W3dLoader;

impl W3dLoader {
    pub fn new() -> Self {
        Self
    }
}

impl MeshLoader for W3dLoader {
    fn load_from_file(&self, path: &Path) -> MeshLoadResult {
        match fs::read(path) {
            Ok(data) => self.load_from_bytes(&data),
            Err(e) => MeshLoadResult::Error(format!("Failed to read file: {}", e)),
        }
    }

    fn load_from_bytes(&self, data: &[u8]) -> MeshLoadResult {
        // Minimal W3D mesh loader: parses MeshHeader3, Vertices, VertexNormals, Triangles
        let mut cursor = Cursor::new(data);
        let file_len = data.len() as u64;

        // Iterate top-level chunks
        while cursor.stream_position().unwrap_or(0) + 8 <= file_len {
            let header: W3dChunkHeader = match BinRead::read(&mut cursor) {
                Ok(h) => h,
                Err(_) => break,
            };
            let start = cursor.stream_position().unwrap_or(0);
            let chunk_size = header.actual_size() as u64;
            let chunk_end = start + chunk_size;

            match header.chunk_type() {
                Some(W3DChunkType::Mesh) => {
                    // Parse mesh sub-chunks
                    let mut mesh_hdr: Option<W3dMeshHeader3Struct> = None;
                    let mut vertices: Vec<W3dVectorStruct> = Vec::new();
                    let mut normals: Vec<W3dVectorStruct> = Vec::new();
                    let mut triangles: Vec<W3dTriangleStruct> = Vec::new();
                    let mut stage_texcoords: Option<Vec<W3dTexCoordStruct>> = None;

                    while cursor.stream_position().unwrap_or(0) < chunk_end {
                        let sub_hdr: W3dChunkHeader = match BinRead::read(&mut cursor) {
                            Ok(h) => h,
                            Err(_) => break,
                        };
                        let sub_start = cursor.stream_position().unwrap_or(0);
                        let sub_end = sub_start + sub_hdr.actual_size() as u64;

                        match sub_hdr.chunk_type() {
                            Some(W3DChunkType::MeshHeader3) => {
                                // Read mesh header struct directly from bytes
                                if (sub_hdr.actual_size() as usize)
                                    >= std::mem::size_of::<W3dMeshHeader3Struct>()
                                {
                                    let mut buf =
                                        vec![0u8; std::mem::size_of::<W3dMeshHeader3Struct>()];
                                    if cursor.read_exact(&mut buf).is_ok() {
                                        // parse W3dMeshHeader3Struct field-by-field to avoid Pod requirements
                                        // Fallback: use BinRead on a dedicated cursor over the sub-buffer
                                        let mut sub_cur = Cursor::new(buf);
                                        if let Ok(h) = W3dMeshHeader3Struct::read(&mut sub_cur) {
                                            mesh_hdr = Some(h);
                                        } else {
                                            return MeshLoadResult::Error(
                                                "Failed to parse W3D MeshHeader3".to_string(),
                                            );
                                        }
                                    } else {
                                        return MeshLoadResult::Error(
                                            "Failed to read W3D MeshHeader3".to_string(),
                                        );
                                    }
                                } else {
                                    return MeshLoadResult::Error(
                                        "Invalid MeshHeader3 chunk size".to_string(),
                                    );
                                }
                            }
                            Some(W3DChunkType::Vertices) => {
                                if let Some(ref mh) = mesh_hdr {
                                    let count = mh.num_verts as usize;
                                    for _ in 0..count {
                                        match W3dVectorStruct::read(&mut cursor) {
                                            Ok(v) => vertices.push(v),
                                            Err(_) => {
                                                return MeshLoadResult::Error(
                                                    "Failed to read W3D vertices".to_string(),
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    let _ = cursor.seek(SeekFrom::Start(sub_end));
                                }
                            }
                            Some(W3DChunkType::VertexNormals) => {
                                if let Some(ref mh) = mesh_hdr {
                                    let count = mh.num_verts as usize;
                                    for _ in 0..count {
                                        match W3dVectorStruct::read(&mut cursor) {
                                            Ok(n) => normals.push(n),
                                            Err(_) => {
                                                return MeshLoadResult::Error(
                                                    "Failed to read W3D normals".to_string(),
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    let _ = cursor.seek(SeekFrom::Start(sub_end));
                                }
                            }
                            Some(W3DChunkType::Triangles) => {
                                if let Some(ref mh) = mesh_hdr {
                                    let count = mh.num_tris as usize;
                                    for _ in 0..count {
                                        match W3dTriangleStruct::read(&mut cursor) {
                                            Ok(t) => triangles.push(t),
                                            Err(_) => {
                                                return MeshLoadResult::Error(
                                                    "Failed to read W3D triangles".to_string(),
                                                )
                                            }
                                        }
                                    }
                                } else {
                                    let _ = cursor.seek(SeekFrom::Start(sub_end));
                                }
                            }
                            Some(W3DChunkType::StageTexcoords) => {
                                // Count by chunk length
                                let count = (sub_hdr.actual_size() as usize)
                                    / std::mem::size_of::<W3dTexCoordStruct>();
                                let mut coords = Vec::with_capacity(count);
                                for _ in 0..count {
                                    match W3dTexCoordStruct::read(&mut cursor) {
                                        Ok(tc) => coords.push(tc),
                                        Err(_) => {
                                            return MeshLoadResult::Error(
                                                "Failed to read W3D texcoords".to_string(),
                                            )
                                        }
                                    }
                                }
                                stage_texcoords = Some(coords);
                            }
                            _ => {
                                // Skip unknown sub-chunk
                                let _ = cursor.seek(SeekFrom::Start(sub_end));
                            }
                        }
                    }

                    // Build MeshGeometry
                    if let Some(mh) = mesh_hdr {
                        let mut mesh = MeshGeometry::new();
                        let uv = stage_texcoords;
                        for i in 0..(mh.num_verts as usize) {
                            let v = vertices.get(i).cloned().unwrap_or(W3dVectorStruct {
                                x: 0.0,
                                y: 0.0,
                                z: 0.0,
                            });
                            let n = normals.get(i).cloned().unwrap_or(W3dVectorStruct {
                                x: 0.0,
                                y: 0.0,
                                z: 1.0,
                            });
                            let t = uv
                                .as_ref()
                                .and_then(|uvs| uvs.get(i))
                                .cloned()
                                .unwrap_or(W3dTexCoordStruct { u: 0.0, v: 0.0 });

                            mesh.add_vertex(MeshVertex::new(
                                Vec3::new(v.x, v.y, v.z),
                                Vec3::new(n.x, n.y, n.z),
                                Vec2::new(t.u, t.v),
                            ));
                        }

                        for tri in triangles {
                            mesh.add_triangle(MeshTriangle::new(
                                tri.vindex[0],
                                tri.vindex[1],
                                tri.vindex[2],
                                0,
                            ));
                        }

                        if normals.is_empty() {
                            mesh.compute_normals();
                        }
                        mesh.compute_plane_equations();
                        mesh.update_bounds();

                        // Return first mesh found
                        return MeshLoadResult::Success(mesh);
                    } else {
                        return MeshLoadResult::Error("W3D Mesh missing MeshHeader3".to_string());
                    }
                }
                _ => {
                    // Skip other top-level chunks
                    let _ = cursor.seek(SeekFrom::Start(chunk_end));
                }
            }
        }

        MeshLoadResult::Error("No W3D Mesh chunk found".to_string())
    }

    fn supported_formats(&self) -> Vec<MeshFormat> {
        vec![MeshFormat::W3d]
    }
}

/// Universal mesh loader that automatically detects format
pub struct UniversalMeshLoader {
    loaders: HashMap<MeshFormat, Box<dyn MeshLoader>>,
}

impl UniversalMeshLoader {
    pub fn new() -> Self {
        let mut loaders = HashMap::new();

        loaders.insert(
            MeshFormat::Obj,
            Box::new(ObjLoader::new()) as Box<dyn MeshLoader>,
        );
        loaders.insert(
            MeshFormat::Stl,
            Box::new(StlLoader::new()) as Box<dyn MeshLoader>,
        );
        loaders.insert(
            MeshFormat::Ply,
            Box::new(PlyLoader::new()) as Box<dyn MeshLoader>,
        );
        loaders.insert(
            MeshFormat::W3d,
            Box::new(W3dLoader::new()) as Box<dyn MeshLoader>,
        );

        Self { loaders }
    }

    /// Load mesh from file with automatic format detection
    pub fn load(&self, path: &Path) -> MeshLoadResult {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        let format = match extension.as_str() {
            "obj" => MeshFormat::Obj,
            "stl" => MeshFormat::Stl,
            "ply" => MeshFormat::Ply,
            "w3d" => MeshFormat::W3d,
            _ => {
                return MeshLoadResult::Error(format!("Unsupported file extension: {}", extension))
            }
        };

        if let Some(loader) = self.loaders.get(&format) {
            loader.load_from_file(path)
        } else {
            MeshLoadResult::Error(format!("No loader available for format: {:?}", format))
        }
    }

    /// Load mesh from bytes with specified format
    pub fn load_from_bytes(&self, data: &[u8], format: MeshFormat) -> MeshLoadResult {
        if let Some(loader) = self.loaders.get(&format) {
            loader.load_from_bytes(data)
        } else {
            MeshLoadResult::Error(format!("No loader available for format: {:?}", format))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_obj_loader_basic() {
        let obj_content = r#"
# Simple cube
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 1.0 1.0 0.0
v 0.0 1.0 0.0
v 0.0 0.0 1.0
v 1.0 0.0 1.0
v 1.0 1.0 1.0
v 0.0 1.0 1.0

f 1 2 3 4
f 5 8 7 6
f 1 5 6 2
f 2 6 7 3
f 3 7 8 4
f 4 8 5 1
"#;

        let loader = ObjLoader::new();
        let result = loader.load_from_string(obj_content);

        match result {
            MeshLoadResult::Success(mesh) => {
                assert_eq!(mesh.vertex_count(), 8);
                assert_eq!(mesh.triangle_count(), 12); // Cube has 12 triangles
            }
            MeshLoadResult::Error(e) => panic!("Failed to load OBJ: {}", e),
        }
    }

    #[test]
    fn test_universal_loader() {
        let loader = UniversalMeshLoader::new();
        assert!(loader.loaders.contains_key(&MeshFormat::Obj));
        assert!(loader.loaders.contains_key(&MeshFormat::Stl));
        assert!(loader.loaders.contains_key(&MeshFormat::Ply));
        assert!(loader.loaders.contains_key(&MeshFormat::W3d));
    }
}
