// Mesh Model - Shared geometry and material data
// Ported from meshmdl.h

use crate::mesh_geometry::MeshGeometry;
use crate::material::MaterialInfo;
use crate::w3d_file::*;
use crate::{Result};
use std::io::Read;

// Mesh Model - contains shared geometry and material data
pub struct MeshModel {
    pub geometry: MeshGeometry,
    pub material_info: MaterialInfo,
    pub sort_level: i32,
}

impl MeshModel {
    pub fn new() -> Self {
        Self {
            geometry: MeshGeometry::new(),
            material_info: MaterialInfo::new(),
            sort_level: 0,
        }
    }

    pub fn get_name(&self) -> &str {
        self.geometry.get_name()
    }

    pub fn set_name(&mut self, name: String) {
        self.geometry.set_name(name);
    }

    pub fn get_pass_count(&self) -> usize {
        self.material_info.get_pass_count()
    }

    pub fn load_w3d<R: Read>(&mut self, reader: &mut R) -> Result<()> {
        loop {
            let header = match W3DChunkHeader::read(reader) {
                Ok(h) => h,
                Err(_) => break,
            };

            match W3DChunkType::from_u32(header.chunk_type) {
                Some(W3DChunkType::MeshHeader3) => self.read_mesh_header(reader)?,
                Some(W3DChunkType::Vertices) => self.geometry.read_vertices(reader, header.chunk_size)?,
                Some(W3DChunkType::VertexNormals) => self.geometry.read_vertex_normals(reader, header.chunk_size)?,
                Some(W3DChunkType::Triangles) => self.geometry.read_triangles(reader, header.chunk_size)?,
                Some(W3DChunkType::MeshUserText) => self.geometry.read_user_text(reader, header.chunk_size)?,
                Some(W3DChunkType::VertexInfluences) => self.geometry.read_vertex_influences(reader, header.chunk_size)?,
                Some(W3DChunkType::MaterialInfo) => self.read_material_info(reader)?,
                _ => {
                    // Skip unknown chunks
                    let mut buf = vec![0u8; header.chunk_size as usize];
                    reader.read_exact(&mut buf)?;
                }
            }
        }

        self.post_process();

        Ok(())
    }

    fn read_mesh_header<R: Read>(&mut self, reader: &mut R) -> Result<()> {
        let mut buf = [0u8; std::mem::size_of::<W3DMeshHeader3>()];
        reader.read_exact(&mut buf)?;

        let version = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let attributes = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);

        let mesh_name_bytes = &buf[8..24];
        let mesh_name_end = mesh_name_bytes.iter().position(|&b| b == 0).unwrap_or(16);
        let mesh_name = String::from_utf8_lossy(&mesh_name_bytes[..mesh_name_end]).to_string();

        let num_tris = u32::from_le_bytes([buf[40], buf[41], buf[42], buf[43]]) as usize;
        let num_vertices = u32::from_le_bytes([buf[44], buf[45], buf[46], buf[47]]) as usize;

        let sort_level = i32::from_le_bytes([buf[56], buf[57], buf[58], buf[59]]);

        self.geometry.set_name(mesh_name);
        self.geometry.reset_geometry(num_tris, num_vertices);
        self.geometry.w3d_attributes = attributes;
        self.sort_level = sort_level;

        Ok(())
    }

    fn read_material_info<R: Read>(&mut self, reader: &mut R) -> Result<()> {
        let mut buf = [0u8; std::mem::size_of::<W3DMaterialInfo>()];
        reader.read_exact(&mut buf)?;

        let pass_count = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;

        // Initialize material passes
        for _ in 0..pass_count {
            self.material_info.add_pass(crate::material::MaterialPass::new());
        }

        Ok(())
    }

    fn post_process(&mut self) {
        // Post-processing steps:
        // 1. Compute missing vertex normals if needed
        // 2. Compute plane equations
        // 3. Build culling tree if needed
        // 4. Setup default materials if none specified

        // Ensure vertex normals exist
        if self.geometry.vertex_normals.is_none() {
            self.geometry.compute_vertex_normals();
        }

        // If no material passes, create a default one
        if self.material_info.passes.is_empty() {
            self.material_info.add_pass(crate::material::MaterialPass::new());
        }
    }

    pub fn scale(&mut self, scale: &crate::math::Vec3) {
        self.geometry.scale(scale);
    }
}

impl Default for MeshModel {
    fn default() -> Self {
        Self::new()
    }
}
