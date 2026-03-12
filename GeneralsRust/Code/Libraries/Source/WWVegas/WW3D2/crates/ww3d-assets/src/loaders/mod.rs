//! W3D File Format Loaders
//!
//! This module provides complete W3D file format loaders for meshes,
//! hierarchies, and animations. These loaders are faithful ports of the
//! C++ implementation from meshmdlio.cpp, htree.cpp, and hcompressedanim.cpp.
//!
//! # Organization
//! - `mesh_loader` - W3D mesh loading (vertices, materials, skinning)
//! - `hierarchy_loader` - W3D skeleton hierarchy loading
//! - `animation_loader` - W3D compressed animation loading
//! - `hlod_loader` - W3D HLOD loading (hierarchical LOD definitions)
//!
//! # C++ References
//! - meshmdlio.cpp - Mesh loading implementation
//! - htree.cpp - Hierarchy loading (lines 800-1200)
//! - hcompressedanim.cpp - Animation loading (lines 650-1200)

pub mod animation_loader;
pub mod hierarchy_loader;
pub mod hlod_loader;
pub mod mesh_loader;

pub use animation_loader::AnimationLoader;
pub use hierarchy_loader::HierarchyLoader;
pub use hlod_loader::HlodLoader;
pub use mesh_loader::MeshLoader;

// Compatibility re-exports from old loaders module
// These are stub types to maintain compatibility with existing code

/// Animation data (compatibility stub)
#[derive(Debug, Clone)]
pub struct AnimationData {
    pub name: String,
    pub frames: u32,
}

/// Parse W3D file (compatibility stub)
pub fn parse_w3d_file<R: std::io::Read + std::io::Seek>(
    _reader: &mut R,
    _asset_manager: &mut crate::assets::AssetManager,
) -> std::io::Result<()> {
    // Stub implementation - real parsing is done by specific loaders
    Ok(())
}
