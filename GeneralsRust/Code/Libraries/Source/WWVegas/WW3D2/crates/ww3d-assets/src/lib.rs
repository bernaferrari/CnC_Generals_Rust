use std::path::PathBuf;
use ww3d_core::errors::W3DResult;
use ww3d_core::W3DError;

pub mod agg_def;
pub mod animation;
pub mod asset_manager;
pub mod assets;
pub mod chunk_reader;
pub mod dazzle;
pub mod loader;
pub mod loaders;
pub use loaders::{animation_loader, hierarchy_loader, mesh_loader};
pub mod material;
pub mod prototype;
pub mod prototype_loader;
pub mod prototypes;
pub mod rendering;
pub mod shatter;
pub mod sound;
pub mod texture;
pub mod w3d_loader;

/// Asset loading status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetStatus {
    /// Asset not loaded
    Unloaded,
    /// Asset is currently being loaded
    Loading,
    /// Asset is loaded and ready to use
    Loaded,
    /// Asset failed to load
    Failed(String),
}

/// Asset dependency information
#[derive(Debug, Clone)]
pub struct AssetDependency {
    /// Path to the dependent asset
    pub asset_path: PathBuf,
    /// Whether this dependency is optional
    pub optional: bool,
    /// Description of the dependency
    pub description: String,
}

impl AssetDependency {
    /// Create a new required dependency
    pub fn required(path: PathBuf, description: String) -> Self {
        Self {
            asset_path: path,
            optional: false,
            description,
        }
    }

    /// Create a new optional dependency
    pub fn optional(path: PathBuf, description: String) -> Self {
        Self {
            asset_path: path,
            optional: true,
            description,
        }
    }
}

pub use agg_def::{AggregateAttachment, AggregateInstance, AggregatePrototype, AggregateSubobject};
pub use animation::{
    Animation, AnimationBlendEntry, AnimationBlender, AnimationChannel, AnimationManager,
    AnimationPlayer, AnimationState, BoneTransform, ChannelType, Keyframe,
};
pub use asset_manager::{AssetManagerExt, AssetManagerStats};
pub use assets::AssetManager;
pub use dazzle::{
    clear_dazzle_types, get_dazzle_type, get_dazzle_type_names, get_lensflare_type,
    init_dazzle_system, is_dazzle_rendering_enabled, register_dazzle_type, register_lensflare_type,
    set_dazzle_rendering_enabled, BlendFunc, CullMode as DazzleCullMode, DazzleEntry,
    DazzleInitClass, DazzleLayerClass, DazzleLibrary, DazzleRenderObjClass, DazzleTypeClass,
    DazzleVertex, DepthCompare as DazzleDepthCompare, LensflareInitClass, LensflareTypeClass,
    Matrix3D, Matrix4, ShaderState, Vector3, Vector4,
};
pub use loader::w3d_streaming_loader::*;
pub use material::{
    AlphaTest, ColorMask, CullMode, DepthCompare, DepthMask, DetailAlphaFunc, DetailColorFunc,
    DstBlendFunc, FogFunc, Material, MaterialManager, MaterialPass, PrimaryGradient,
    SecondaryGradient, Shader, SrcBlendFunc, Texturing, VertexMaterial,
};
pub use prototype::{CloneRenderObj, PrimitivePrototype, Prototype, PrototypeBuilder};
pub use prototype_loader::{
    DefaultLoaders, HAnimLoader, HModelLoader, HTreeLoader, MeshLoader, PrototypeLoader,
};
pub use rendering::{
    BufferHandle, GpuMesh, GpuSkinnedMesh, MeshData, NullRenderBackend, PipelineHandle,
    RenderBackend, RenderCommand, Renderer, SkinnedMeshData, SkinnedVertex, TextureHandle, Vertex,
    WgpuRenderBackend,
};
pub use shatter::{
    MeshFragment, MeshMtlParams, MeshVertex, Plane as ShatterPlane, Polygon, ShatterSystem,
    Vertex as ShatterVertex, BSP,
};
pub use texture::{
    CubeTexture, MipCount, PoolType, Texture, TextureAssetType, TextureBase, TextureFormat,
    TextureLoader, TextureManager, TextureMipLevel, VolumeTexture,
};
pub use w3d_loader::{LoadError, ParseError, W3DChunk, W3DLoader, W3DModel};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_manager_creation() {
        let mgr = AssetManager::new();
        // Test that AssetManager can be created successfully
        assert_eq!(mgr.prototypes.len(), 0);
        // Future: Add tests for loading with mock data
    }
}
