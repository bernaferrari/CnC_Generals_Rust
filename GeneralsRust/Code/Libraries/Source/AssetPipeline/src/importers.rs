//! Asset Importers
//!
//! This module provides importers for various 3D asset formats including:
//! - FBX (Autodesk Filmbox)
//! - glTF/GLB (GL Transmission Format)
//! - OBJ (Wavefront Object)
//! - W3D (Westwood 3D - for legacy asset support)

use crate::{
    Asset, AssetData, AssetError, AssetImporter, AssetMetadata, AssetType, BoundingBox, MeshData,
    Result, Vertex,
};
use async_trait::async_trait;
use std::path::Path;

/// FBX file importer
#[derive(Debug, Clone)]
pub struct FbxImporter {
    // Configuration options
    import_animations: bool,
    import_materials: bool,
    scale_factor: f32,
}

impl FbxImporter {
    pub fn new() -> Self {
        Self {
            import_animations: true,
            import_materials: true,
            scale_factor: 1.0,
        }
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale_factor = scale;
        self
    }

    pub fn animations(mut self, enabled: bool) -> Self {
        self.import_animations = enabled;
        self
    }

    pub fn materials(mut self, enabled: bool) -> Self {
        self.import_materials = enabled;
        self
    }
}

impl Default for FbxImporter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AssetImporter for FbxImporter {
    async fn import(&self, path: &Path) -> Result<Asset> {
        log::info!("Importing FBX file: {:?}", path);

        if !path.exists() {
            return Err(AssetError::FileNotFound {
                path: path.to_path_buf(),
            });
        }

        // TODO: Implement full FBX parsing using FBX SDK or custom parser
        // For now, create a placeholder asset
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
            .to_string();

        let mut asset = Asset::new(name, AssetType::Mesh);
        asset.metadata.source_file = Some(path.to_path_buf());
        asset
            .metadata
            .import_settings
            .insert("scale_factor".to_string(), self.scale_factor.to_string());

        // Create minimal mesh data as placeholder
        let mesh_data = MeshData {
            vertices: vec![],
            indices: vec![],
            materials: vec![],
            bone_weights: vec![],
            bounds: BoundingBox {
                min: [0.0, 0.0, 0.0],
                max: [0.0, 0.0, 0.0],
            },
        };

        asset.data = AssetData::Mesh(mesh_data);

        log::info!("FBX import complete: {} vertices", 0);
        Ok(asset)
    }

    fn supported_extensions(&self) -> &[&str] {
        &["fbx", "FBX"]
    }
}

/// glTF/GLB file importer
#[derive(Debug, Clone)]
pub struct GltfImporter {
    import_animations: bool,
    import_cameras: bool,
    import_lights: bool,
}

impl GltfImporter {
    pub fn new() -> Self {
        Self {
            import_animations: true,
            import_cameras: true,
            import_lights: true,
        }
    }
}

impl Default for GltfImporter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AssetImporter for GltfImporter {
    async fn import(&self, path: &Path) -> Result<Asset> {
        log::info!("Importing glTF file: {:?}", path);

        if !path.exists() {
            return Err(AssetError::FileNotFound {
                path: path.to_path_buf(),
            });
        }

        #[cfg(feature = "importers")]
        {
            // Use gltf crate for actual parsing
            let gltf_data = gltf::Gltf::open(path)
                .map_err(|e| AssetError::ImportFailed(format!("glTF parse error: {}", e)))?;

            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unnamed")
                .to_string();

            let mut asset = Asset::new(name, AssetType::Scene);
            asset.metadata.source_file = Some(path.to_path_buf());

            // Extract mesh data from first mesh
            if let Some(mesh) = gltf_data.meshes().next() {
                let mut vertices = Vec::new();
                let mut indices = Vec::new();

                for primitive in mesh.primitives() {
                    let reader = primitive.reader(|_buffer| None);

                    // Read positions (required)
                    if let Some(positions_iter) = reader.read_positions() {
                        for position in positions_iter {
                            vertices.push(Vertex {
                                position,
                                normal: [0.0, 1.0, 0.0], // Default up
                                uv: [0.0, 0.0],
                                tangent: [1.0, 0.0, 0.0],    // Default
                                color: [1.0, 1.0, 1.0, 1.0], // White
                            });
                        }
                    }

                    // Read indices if available
                    if let Some(indices_reader) = reader.read_indices() {
                        indices.extend(indices_reader.into_u32());
                    }
                }

                let mesh_data = MeshData {
                    vertices,
                    indices,
                    materials: vec![],
                    bone_weights: vec![],
                    bounds: calculate_bounds(&[]),
                };

                asset.data = AssetData::Mesh(mesh_data);
            }

            log::info!("glTF import complete");
            Ok(asset)
        }

        #[cfg(not(feature = "importers"))]
        {
            Err(AssetError::ImportFailed(
                "glTF support requires 'importers' feature".to_string(),
            ))
        }
    }

    fn supported_extensions(&self) -> &[&str] {
        &["gltf", "glb"]
    }
}

/// OBJ file importer
#[derive(Debug, Clone)]
pub struct ObjImporter {
    triangulate: bool,
    flip_uvs: bool,
}

impl ObjImporter {
    pub fn new() -> Self {
        Self {
            triangulate: true,
            flip_uvs: false,
        }
    }
}

impl Default for ObjImporter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AssetImporter for ObjImporter {
    async fn import(&self, path: &Path) -> Result<Asset> {
        log::info!("Importing OBJ file: {:?}", path);

        if !path.exists() {
            return Err(AssetError::FileNotFound {
                path: path.to_path_buf(),
            });
        }

        #[cfg(feature = "importers")]
        {
            use std::fs::File;
            use std::io::BufReader;

            let file = File::open(path)
                .map_err(|e| AssetError::ImportFailed(format!("Failed to open OBJ: {}", e)))?;
            let reader = BufReader::new(file);

            let obj_data = obj::ObjData::load_buf(reader)
                .map_err(|e| AssetError::ImportFailed(format!("OBJ parse error: {}", e)))?;

            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unnamed")
                .to_string();

            let mut asset = Asset::new(name, AssetType::Mesh);
            asset.metadata.source_file = Some(path.to_path_buf());

            // Convert OBJ data to our mesh format
            let mut vertices = Vec::new();
            let mut indices = Vec::new();

            for object in obj_data.objects {
                for group in object.groups {
                    for poly in group.polys {
                        // Add vertices from polygon
                        for vertex_index in poly.0 {
                            let position = obj_data.position[vertex_index.0];
                            let normal = vertex_index
                                .2
                                .map(|idx| obj_data.normal[idx])
                                .unwrap_or([0.0, 1.0, 0.0]);
                            let uv = vertex_index
                                .1
                                .map(|idx| obj_data.texture[idx])
                                .unwrap_or([0.0, 0.0]);

                            vertices.push(Vertex {
                                position,
                                normal,
                                uv,
                                tangent: [1.0, 0.0, 0.0],
                                color: [1.0, 1.0, 1.0, 1.0],
                            });

                            indices.push((vertices.len() - 1) as u32);
                        }
                    }
                }
            }

            let mesh_data = MeshData {
                vertices,
                indices,
                materials: vec![],
                bone_weights: vec![],
                bounds: calculate_bounds(&[]),
            };

            asset.data = AssetData::Mesh(mesh_data);

            log::info!("OBJ import complete");
            Ok(asset)
        }

        #[cfg(not(feature = "importers"))]
        {
            Err(AssetError::ImportFailed(
                "OBJ support requires 'importers' feature".to_string(),
            ))
        }
    }

    fn supported_extensions(&self) -> &[&str] {
        &["obj", "OBJ"]
    }
}

/// Calculate bounding box from vertices
fn calculate_bounds(vertices: &[Vertex]) -> BoundingBox {
    if vertices.is_empty() {
        return BoundingBox {
            min: [0.0, 0.0, 0.0],
            max: [0.0, 0.0, 0.0],
        };
    }

    let mut min = vertices[0].position;
    let mut max = vertices[0].position;

    for vertex in vertices.iter().skip(1) {
        for i in 0..3 {
            min[i] = min[i].min(vertex.position[i]);
            max[i] = max[i].max(vertex.position[i]);
        }
    }

    BoundingBox { min, max }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fbx_importer_creation() {
        let importer = FbxImporter::new();
        assert_eq!(importer.scale_factor, 1.0);
        assert!(importer.import_animations);
        assert!(importer.import_materials);
    }

    #[test]
    fn test_gltf_importer_creation() {
        let importer = GltfImporter::new();
        assert!(importer.import_animations);
        assert!(importer.import_cameras);
        assert!(importer.import_lights);
    }

    #[test]
    fn test_obj_importer_creation() {
        let importer = ObjImporter::new();
        assert!(importer.triangulate);
        assert!(!importer.flip_uvs);
    }

    #[test]
    fn test_supported_extensions() {
        let fbx = FbxImporter::new();
        assert!(fbx.supported_extensions().contains(&"fbx"));

        let gltf = GltfImporter::new();
        assert!(gltf.supported_extensions().contains(&"gltf"));
        assert!(gltf.supported_extensions().contains(&"glb"));

        let obj = ObjImporter::new();
        assert!(obj.supported_extensions().contains(&"obj"));
    }

    #[test]
    fn test_bounds_calculation() {
        let vertices = vec![
            Vertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                tangent: [1.0, 0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            Vertex {
                position: [1.0, 2.0, 3.0],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 1.0],
                tangent: [1.0, 0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
        ];

        let bounds = calculate_bounds(&vertices);
        assert_eq!(bounds.min, [0.0, 0.0, 0.0]);
        assert_eq!(bounds.max, [1.0, 2.0, 3.0]);
    }
}
