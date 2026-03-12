//! Asset Exporters
//!
//! This module provides exporters for various asset formats:
//! - W3D (Westwood 3D) - Native format for C&C Generals
//! - glTF/GLB - Industry standard 3D format
//! - FBX - Autodesk Filmbox export
//! - Custom binary formats

use crate::{Asset, AssetData, AssetError, AssetExporter, AssetType, MeshData, Result};
use async_trait::async_trait;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// W3D (Westwood 3D) exporter
#[derive(Debug, Clone)]
pub struct W3dExporter {
    version: u32,
    compress: bool,
    include_hierarchy: bool,
}

impl W3dExporter {
    pub fn new() -> Self {
        Self {
            version: 0x00040000, // Version 4.0
            compress: true,
            include_hierarchy: true,
        }
    }

    pub fn with_version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    pub fn with_compression(mut self, enabled: bool) -> Self {
        self.compress = enabled;
        self
    }

    fn write_w3d_header<W: Write>(&self, writer: &mut W) -> Result<()> {
        // W3D magic number
        writer.write_all(b"W3D\0")?;

        // Version
        writer.write_all(&self.version.to_le_bytes())?;

        Ok(())
    }

    fn write_mesh_data<W: Write>(&self, writer: &mut W, mesh: &MeshData) -> Result<()> {
        // Write vertex count
        writer.write_all(&(mesh.vertices.len() as u32).to_le_bytes())?;

        // Write vertices
        for vertex in &mesh.vertices {
            writer.write_all(&vertex.position[0].to_le_bytes())?;
            writer.write_all(&vertex.position[1].to_le_bytes())?;
            writer.write_all(&vertex.position[2].to_le_bytes())?;

            writer.write_all(&vertex.normal[0].to_le_bytes())?;
            writer.write_all(&vertex.normal[1].to_le_bytes())?;
            writer.write_all(&vertex.normal[2].to_le_bytes())?;

            writer.write_all(&vertex.uv[0].to_le_bytes())?;
            writer.write_all(&vertex.uv[1].to_le_bytes())?;
        }

        // Write index count
        writer.write_all(&(mesh.indices.len() as u32).to_le_bytes())?;

        // Write indices
        for index in &mesh.indices {
            writer.write_all(&index.to_le_bytes())?;
        }

        Ok(())
    }
}

impl Default for W3dExporter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AssetExporter for W3dExporter {
    async fn export(&self, asset: &Asset, path: &Path) -> Result<()> {
        log::info!("Exporting to W3D: {:?}", path);

        match &asset.data {
            AssetData::Mesh(mesh) => {
                let file = File::create(path).map_err(|e| {
                    AssetError::ExportFailed(format!("Failed to create W3D file: {}", e))
                })?;
                let mut writer = BufWriter::new(file);

                self.write_w3d_header(&mut writer)?;
                self.write_mesh_data(&mut writer, mesh)?;

                writer.flush()?;
                log::info!("W3D export complete: {} vertices", mesh.vertices.len());
                Ok(())
            }
            _ => Err(AssetError::ExportFailed(
                "W3D export only supports mesh data".to_string(),
            )),
        }
    }

    fn supported_extensions(&self) -> &[&str] {
        &["w3d", "W3D"]
    }

    fn can_export(&self, asset: &Asset) -> bool {
        matches!(asset.asset_type, AssetType::Mesh) && matches!(asset.data, AssetData::Mesh(_))
    }
}

/// glTF/GLB exporter
#[derive(Debug, Clone)]
pub struct GltfExporter {
    binary: bool,
    embed_buffers: bool,
    embed_images: bool,
    pretty_print: bool,
}

impl GltfExporter {
    pub fn new() -> Self {
        Self {
            binary: false,
            embed_buffers: true,
            embed_images: true,
            pretty_print: true,
        }
    }

    pub fn binary(mut self, enabled: bool) -> Self {
        self.binary = enabled;
        self
    }

    pub fn embed_buffers(mut self, enabled: bool) -> Self {
        self.embed_buffers = enabled;
        self
    }

    pub fn embed_images(mut self, enabled: bool) -> Self {
        self.embed_images = enabled;
        self
    }

    pub fn pretty_print(mut self, enabled: bool) -> Self {
        self.pretty_print = enabled;
        self
    }
}

impl Default for GltfExporter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AssetExporter for GltfExporter {
    async fn export(&self, asset: &Asset, path: &Path) -> Result<()> {
        log::info!("Exporting to glTF: {:?}", path);

        #[cfg(feature = "exporters")]
        {
            // TODO: Implement full glTF export using gltf-json crate
            // For now, create a basic JSON structure

            match &asset.data {
                AssetData::Mesh(_mesh) => {
                    let gltf_json = serde_json::json!({
                        "asset": {
                            "version": "2.0",
                            "generator": "AssetPipeline 1.0"
                        },
                        "scene": 0,
                        "scenes": [
                            {
                                "nodes": [0]
                            }
                        ],
                        "nodes": [
                            {
                                "mesh": 0,
                                "name": asset.name
                            }
                        ],
                        "meshes": [
                            {
                                "primitives": [
                                    {
                                        "attributes": {
                                            "POSITION": 0,
                                            "NORMAL": 1,
                                            "TEXCOORD_0": 2
                                        },
                                        "indices": 3
                                    }
                                ],
                                "name": asset.name
                            }
                        ],
                        "accessors": [],
                        "bufferViews": [],
                        "buffers": []
                    });

                    let file = File::create(path).map_err(|e| {
                        AssetError::ExportFailed(format!("Failed to create glTF file: {}", e))
                    })?;

                    if self.pretty_print {
                        serde_json::to_writer_pretty(file, &gltf_json)?;
                    } else {
                        serde_json::to_writer(file, &gltf_json)?;
                    }

                    log::info!("glTF export complete");
                    Ok(())
                }
                _ => Err(AssetError::ExportFailed(
                    "glTF export currently only supports mesh data".to_string(),
                )),
            }
        }

        #[cfg(not(feature = "exporters"))]
        {
            Err(AssetError::ExportFailed(
                "glTF export requires 'exporters' feature".to_string(),
            ))
        }
    }

    fn supported_extensions(&self) -> &[&str] {
        if self.binary {
            &["glb", "GLB"]
        } else {
            &["gltf", "GLTF"]
        }
    }

    fn can_export(&self, asset: &Asset) -> bool {
        matches!(
            asset.asset_type,
            AssetType::Mesh | AssetType::Scene | AssetType::Material
        )
    }
}

/// Raw binary exporter for custom formats
#[derive(Debug, Clone)]
pub struct BinaryExporter {
    compress: bool,
    include_metadata: bool,
}

impl BinaryExporter {
    pub fn new() -> Self {
        Self {
            compress: true,
            include_metadata: true,
        }
    }

    pub fn compress(mut self, enabled: bool) -> Self {
        self.compress = enabled;
        self
    }

    pub fn metadata(mut self, enabled: bool) -> Self {
        self.include_metadata = enabled;
        self
    }
}

impl Default for BinaryExporter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AssetExporter for BinaryExporter {
    async fn export(&self, asset: &Asset, path: &Path) -> Result<()> {
        log::info!("Exporting to binary format: {:?}", path);

        let file = File::create(path).map_err(|e| {
            AssetError::ExportFailed(format!("Failed to create binary file: {}", e))
        })?;
        let mut writer = BufWriter::new(file);

        // Magic number
        writer.write_all(b"ASST")?;

        // Version
        writer.write_all(&1u32.to_le_bytes())?;

        // Asset type (convert to discriminant)
        let type_id: u32 = match asset.asset_type {
            crate::AssetType::Mesh => 0,
            crate::AssetType::Texture => 1,
            crate::AssetType::Material => 2,
            crate::AssetType::Animation => 3,
            crate::AssetType::Audio => 4,
            crate::AssetType::Video => 5,
            crate::AssetType::Scene => 6,
            crate::AssetType::Prefab => 7,
            crate::AssetType::Script => 8,
            crate::AssetType::Shader => 9,
            crate::AssetType::Font => 10,
            crate::AssetType::Custom(id) => 1000 + id,
        };
        writer.write_all(&type_id.to_le_bytes())?;

        // Metadata
        if self.include_metadata {
            let metadata_json = serde_json::to_string(&asset.metadata)?;
            let metadata_bytes = metadata_json.as_bytes();
            writer.write_all(&(metadata_bytes.len() as u32).to_le_bytes())?;
            writer.write_all(metadata_bytes)?;
        } else {
            writer.write_all(&0u32.to_le_bytes())?;
        }

        // Asset data
        match &asset.data {
            AssetData::Raw(data) => {
                writer.write_all(&(data.len() as u32).to_le_bytes())?;
                writer.write_all(data)?;
            }
            _ => {
                // Serialize other data types
                let data = vec![]; // Placeholder
                writer.write_all(&(data.len() as u32).to_le_bytes())?;
                writer.write_all(&data)?;
            }
        }

        writer.flush()?;
        log::info!("Binary export complete");
        Ok(())
    }

    fn supported_extensions(&self) -> &[&str] {
        &["bin", "dat", "asset"]
    }

    fn can_export(&self, _asset: &Asset) -> bool {
        true // Can export any asset type
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoneWeight, BoundingBox, Vertex};

    #[test]
    fn test_w3d_exporter_creation() {
        let exporter = W3dExporter::new();
        assert_eq!(exporter.version, 0x00040000);
        assert!(exporter.compress);
        assert!(exporter.include_hierarchy);
    }

    #[test]
    fn test_gltf_exporter_creation() {
        let exporter = GltfExporter::new();
        assert!(!exporter.binary);
        assert!(exporter.embed_buffers);
        assert!(exporter.embed_images);
        assert!(exporter.pretty_print);
    }

    #[test]
    fn test_binary_exporter_creation() {
        let exporter = BinaryExporter::new();
        assert!(exporter.compress);
        assert!(exporter.include_metadata);
    }

    #[test]
    fn test_supported_extensions() {
        let w3d = W3dExporter::new();
        assert!(w3d.supported_extensions().contains(&"w3d"));

        let gltf = GltfExporter::new();
        assert!(gltf.supported_extensions().contains(&"gltf"));

        let bin = BinaryExporter::new();
        assert!(bin.supported_extensions().contains(&"bin"));
    }

    #[test]
    fn test_can_export() {
        let mesh_asset = Asset::new("test", AssetType::Mesh);

        let w3d = W3dExporter::new();
        // Empty mesh can't be exported
        assert!(!w3d.can_export(&mesh_asset));

        let gltf = GltfExporter::new();
        assert!(gltf.can_export(&mesh_asset));

        let bin = BinaryExporter::new();
        assert!(bin.can_export(&mesh_asset));
    }

    #[test]
    fn test_gltf_binary_mode() {
        let exporter = GltfExporter::new().binary(true);
        assert!(exporter.supported_extensions().contains(&"glb"));
    }
}
