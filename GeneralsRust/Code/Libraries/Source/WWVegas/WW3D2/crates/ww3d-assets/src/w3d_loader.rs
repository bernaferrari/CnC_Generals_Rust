//! High-Level W3D Model/Asset Loading System
//!
//! This module provides a complete, user-friendly interface for loading W3D game models.
//! It integrates all the low-level loaders (mesh, hierarchy, animation) into a single
//! cohesive system.
//!
//! # Architecture
//!
//! The W3D loading system consists of:
//! - **ChunkReader**: Low-level binary chunk parser (chunk_reader.rs)
//! - **MeshLoader**: Loads mesh geometry, materials, textures (loaders/mesh_loader.rs)
//! - **HierarchyLoader**: Loads skeletal hierarchies (loaders/hierarchy_loader.rs)
//! - **AnimationLoader**: Loads animations (loaders/animation_loader.rs)
//! - **AssetManager**: High-level asset management and caching (assets.rs)
//! - **W3DLoader**: This module - simplified facade for common operations
//!
//! # C++ References
//! - `/GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/w3d.h` - Chunk IDs
//! - `/GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/meshload.cpp` - Mesh loading
//! - `/GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/hanimload.cpp` - Animation loading
//!
//! # Usage Example
//!
//! ```no_run
//! use ww3d_assets::W3DLoader;
//!
//! // Load a model file
//! let model = W3DLoader::load("models/tank.w3d")?;
//!
//! // Access loaded data
//! println!("Model has {} meshes", model.meshes.len());
//! println!("Model has {} animations", model.animations.len());
//! # Ok::<(), ww3d_core::W3DError>(())
//! ```

use crate::chunk_reader::{ChunkError, ChunkReader};
use crate::loaders::animation_loader::W3DAnimation;
use crate::loaders::hierarchy_loader::W3DHierarchy;
use crate::loaders::mesh_loader::W3DMesh;
use crate::loaders::{AnimationLoader, HierarchyLoader, HlodLoader, MeshLoader};
use crate::prototypes::HlodPrototype;
use std::io::Cursor;
use std::path::Path;
use ww3d_core::{W3DChunkType, W3DError, W3DResult};

/// Convert ChunkError to W3DError
fn chunk_error_to_w3d(err: ChunkError) -> W3DError {
    match err {
        ChunkError::Io(e) => W3DError::from(e),
        ChunkError::InvalidHeader => W3DError::CorruptedFile,
        ChunkError::UnexpectedEof => W3DError::CorruptedFile,
        ChunkError::BoundsExceeded => W3DError::CorruptedFile,
        ChunkError::InvalidString(e) => {
            W3DError::InvalidParameter(format!("Invalid string: {}", e))
        }
        ChunkError::StackOverflow => W3DError::CorruptedFile,
        ChunkError::StackUnderflow => W3DError::CorruptedFile,
        ChunkError::MicroChunkAlreadyOpen => W3DError::CorruptedFile,
        ChunkError::NoMicroChunkOpen => W3DError::CorruptedFile,
    }
}

/// Complete W3D model with all assets
///
/// This structure represents a fully loaded W3D file containing meshes,
/// hierarchies (skeletons), and animations. It matches the C++ representation
/// where a single .w3d file can contain multiple asset types.
#[derive(Debug, Clone)]
pub struct W3DModel {
    /// All meshes in this model
    pub meshes: Vec<W3DMesh>,

    /// All hierarchies (skeletons) in this model
    pub hierarchies: Vec<W3DHierarchy>,

    /// All animations in this model
    pub animations: Vec<W3DAnimation>,

    /// All HLOD definitions in this model
    pub hlods: Vec<HlodPrototype>,

    /// Texture references from all meshes
    pub textures: Vec<String>,
}

impl W3DModel {
    /// Create an empty W3D model
    pub fn new() -> Self {
        Self {
            meshes: Vec::new(),
            hierarchies: Vec::new(),
            animations: Vec::new(),
            hlods: Vec::new(),
            textures: Vec::new(),
        }
    }

    /// Get total vertex count across all meshes
    pub fn total_vertices(&self) -> usize {
        self.meshes.iter().map(|m| m.vertices.len()).sum()
    }

    /// Get total triangle count across all meshes
    pub fn total_triangles(&self) -> usize {
        self.meshes.iter().map(|m| m.triangles.len()).sum()
    }

    /// Find a mesh by name
    pub fn find_mesh(&self, name: &str) -> Option<&W3DMesh> {
        self.meshes.iter().find(|m| m.header.mesh_name == name)
    }

    /// Find a hierarchy by name
    pub fn find_hierarchy(&self, name: &str) -> Option<&W3DHierarchy> {
        self.hierarchies.iter().find(|h| h.header.name == name)
    }

    /// Find an animation by name
    pub fn find_animation(&self, name: &str) -> Option<&W3DAnimation> {
        self.animations.iter().find(|a| a.header.name == name)
    }

    /// Find an HLOD definition by name
    pub fn find_hlod(&self, name: &str) -> Option<&HlodPrototype> {
        self.hlods.iter().find(|h| h.name == name)
    }

    /// Check if this model has skinning data
    pub fn has_skinning(&self) -> bool {
        self.meshes.iter().any(|m| !m.vertex_influences.is_empty())
    }

    /// Check if this model has animations
    pub fn has_animations(&self) -> bool {
        !self.animations.is_empty()
    }
}

impl Default for W3DModel {
    fn default() -> Self {
        Self::new()
    }
}

/// High-level W3D loader with simplified API
///
/// This loader provides a simple interface for loading W3D models while
/// handling all the low-level chunk parsing and data structure assembly.
///
/// # Implementation Notes
///
/// The loader is stateless and can be reused for loading multiple files.
/// For caching and asset management, use `AssetManager` instead.
pub struct W3DLoader;

impl W3DLoader {
    /// Create a new W3D loader
    pub fn new() -> Self {
        Self
    }

    /// Load a W3D model from a file path
    ///
    /// This is the main entry point for loading W3D files. It reads the file,
    /// parses all chunks, and returns a complete model structure.
    ///
    /// # Arguments
    /// * `path` - Path to the .w3d file
    ///
    /// # Returns
    /// * `Ok(W3DModel)` - Successfully loaded model
    /// * `Err(W3DError)` - Loading failed
    ///
    /// # C++ Reference
    /// This matches the behavior of `WW3DAssetManager::Load_3D_Assets()`
    /// from assetmgr.cpp lines 631-695
    pub fn load<P: AsRef<Path>>(path: P) -> W3DResult<W3DModel> {
        // Read file into memory
        let data = std::fs::read(path.as_ref())?;

        // Load from bytes
        Self::load_from_bytes(&data)
    }

    /// Load a W3D model from a byte buffer
    ///
    /// This allows loading from in-memory data, useful for:
    /// - Loading from embedded resources
    /// - Loading from compressed archives
    /// - Network streaming
    ///
    /// # Arguments
    /// * `data` - Raw W3D file data
    ///
    /// # Returns
    /// * `Ok(W3DModel)` - Successfully loaded model
    /// * `Err(W3DError)` - Parsing failed
    pub fn load_from_bytes(data: &[u8]) -> W3DResult<W3DModel> {
        let mut model = W3DModel::new();
        let cursor = Cursor::new(data);
        let mut reader = ChunkReader::new(cursor);

        // Read all top-level chunks
        // C++ Reference: assetmgr.cpp:665-695 - chunk iteration loop
        while reader.open_chunk().map_err(chunk_error_to_w3d)? {
            let chunk_type = reader.current_chunk_id().map_err(chunk_error_to_w3d)?;

            match W3DChunkType::from_u32(chunk_type) {
                // W3D_CHUNK_MESH (0x00000000)
                // C++ Reference: meshload.cpp:239-428
                Some(W3DChunkType::Mesh) => {
                    let mesh = MeshLoader::load_mesh(&mut reader).map_err(chunk_error_to_w3d)?;

                    // Extract texture names
                    for texture in &mesh.textures {
                        if !model.textures.contains(&texture.name) {
                            model.textures.push(texture.name.clone());
                        }
                    }

                    model.meshes.push(mesh);
                }

                // W3D_CHUNK_HIERARCHY (0x00000100)
                // C++ Reference: htree.cpp:176-244
                Some(W3DChunkType::Hierarchy) => {
                    let hierarchy =
                        HierarchyLoader::load_hierarchy(&mut reader).map_err(chunk_error_to_w3d)?;
                    model.hierarchies.push(hierarchy);
                }

                // W3D_CHUNK_ANIMATION (0x00000200) or COMPRESSED_ANIMATION (0x00000280)
                // C++ Reference: hcanim.cpp:235-374
                Some(W3DChunkType::Animation) | Some(W3DChunkType::CompressedAnimation) => {
                    let animation =
                        AnimationLoader::load_animation(&mut reader).map_err(chunk_error_to_w3d)?;
                    model.animations.push(animation);
                }

                // W3D_CHUNK_HLOD (0x00000700) - Hierarchical LOD
                Some(W3DChunkType::Hlod) => {
                    let hlod = HlodLoader::load_hlod(&mut reader).map_err(chunk_error_to_w3d)?;
                    model.hlods.push(hlod);
                }

                // Unknown or unsupported chunk types are silently skipped
                // This matches C++ behavior in assetmgr.cpp
                _ => {
                    // Skip unknown chunks
                }
            }

            reader.close_chunk().map_err(chunk_error_to_w3d)?;
        }

        Ok(model)
    }

    /// Parse W3D chunk format data
    ///
    /// Low-level function for parsing a specific chunk type. Most users should
    /// use `load()` or `load_from_bytes()` instead.
    ///
    /// # Arguments
    /// * `data` - Raw chunk data
    /// * `chunk_type` - Expected chunk type ID
    ///
    /// # Returns
    /// Parsed chunk data as a generic enum
    ///
    /// # C++ Reference
    /// This is equivalent to the ChunkLoadClass::Open_Chunk() pattern used
    /// throughout the C++ codebase (chunkio.cpp:412-433)
    pub fn parse_chunk(data: &[u8], chunk_type: u32) -> W3DResult<W3DChunk> {
        let cursor = Cursor::new(data);
        let mut reader = ChunkReader::new(cursor);

        if !reader.open_chunk().map_err(chunk_error_to_w3d)? {
            return Err(W3DError::InvalidParameter("No chunk found".to_string()));
        }

        let actual_type = reader.current_chunk_id().map_err(chunk_error_to_w3d)?;
        if actual_type != chunk_type {
            return Err(W3DError::InvalidParameter(format!(
                "Expected chunk type 0x{:08X}, got 0x{:08X}",
                chunk_type, actual_type
            )));
        }

        match W3DChunkType::from_u32(chunk_type) {
            Some(W3DChunkType::Mesh) => Ok(W3DChunk::Mesh(
                MeshLoader::load_mesh(&mut reader).map_err(chunk_error_to_w3d)?,
            )),
            Some(W3DChunkType::Hierarchy) => Ok(W3DChunk::Hierarchy(
                HierarchyLoader::load_hierarchy(&mut reader).map_err(chunk_error_to_w3d)?,
            )),
            Some(W3DChunkType::Animation) | Some(W3DChunkType::CompressedAnimation) => {
                Ok(W3DChunk::Animation(
                    AnimationLoader::load_animation(&mut reader).map_err(chunk_error_to_w3d)?,
                ))
            }
            _ => Err(W3DError::InvalidParameter(format!(
                "Unsupported chunk type: 0x{:08X}",
                chunk_type
            ))),
        }
    }
}

impl Default for W3DLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Parsed W3D chunk data
///
/// This enum represents the possible chunk types that can be parsed from
/// a W3D file. Each variant contains the fully parsed data structure.
#[derive(Debug, Clone)]
pub enum W3DChunk {
    /// Mesh chunk (W3D_CHUNK_MESH = 0x00000000)
    Mesh(W3DMesh),

    /// Hierarchy chunk (W3D_CHUNK_HIERARCHY = 0x00000100)
    Hierarchy(W3DHierarchy),

    /// Animation chunk (W3D_CHUNK_ANIMATION = 0x00000200)
    Animation(W3DAnimation),
}

/// Load error types
///
/// Re-export from ww3d_core for convenience
pub type LoadError = W3DError;

/// Parse error types
///
/// Re-export from ww3d_core for convenience
pub type ParseError = W3DError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_w3d_model_creation() {
        let model = W3DModel::new();
        assert_eq!(model.meshes.len(), 0);
        assert_eq!(model.hierarchies.len(), 0);
        assert_eq!(model.animations.len(), 0);
        assert_eq!(model.textures.len(), 0);
    }

    #[test]
    fn test_w3d_loader_creation() {
        let _loader = W3DLoader::new();
        // Should not panic
    }

    // Additional integration tests would require actual W3D test files
}
