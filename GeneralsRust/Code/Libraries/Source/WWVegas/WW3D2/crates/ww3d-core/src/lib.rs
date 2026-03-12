// Core modules
pub mod chunks;
pub mod class_registry;
pub mod classid;
pub mod dllist;
pub mod errors;
pub mod memory;
pub mod w3d_format;
pub mod w3d_obsolete;
pub mod ww3d;
pub mod wwstring;

// New modules - Complete WW3D implementation
pub mod animation;
pub mod asset_manager;
pub mod lighting;
pub mod material;
pub mod mesh;
pub mod render_object;
pub mod scene;
pub mod texture;
pub mod w3d_io;

pub use chunks::{
    W3DChunkType, W3D_CHUNK_ADAPTIVEDELTA_CHANNEL, W3D_CHUNK_ANIMATION,
    W3D_CHUNK_ANIMATION_CHANNEL, W3D_CHUNK_ANIMATION_HEADER, W3D_CHUNK_BIT_CHANNEL,
    W3D_CHUNK_COMPRESSED_ANIMATION, W3D_CHUNK_COMPRESSED_ANIMATION_CHANNEL,
    W3D_CHUNK_COMPRESSED_ANIMATION_HEADER, W3D_CHUNK_COMPRESSED_BIT_CHANNEL, W3D_CHUNK_HIERARCHY,
    W3D_CHUNK_HIERARCHY_HEADER, W3D_CHUNK_HMODEL, W3D_CHUNK_HMODEL_HEADER, W3D_CHUNK_MATERIAL_INFO,
    W3D_CHUNK_MATERIAL_PASS, W3D_CHUNK_MESH, W3D_CHUNK_MESH_HEADER3, W3D_CHUNK_MESH_USER_TEXT,
    W3D_CHUNK_NODE, W3D_CHUNK_PIVOTS, W3D_CHUNK_PIVOT_FIXUPS, W3D_CHUNK_SHADERS,
    W3D_CHUNK_SHADER_IDS, W3D_CHUNK_TEXTURES, W3D_CHUNK_TIMECODED_CHANNEL, W3D_CHUNK_TRIANGLES,
    W3D_CHUNK_VERTEX_INFLUENCES, W3D_CHUNK_VERTEX_NORMALS, W3D_CHUNK_VERTEX_SHADE_INDICES,
    W3D_CHUNK_VERTICES,
};
use once_cell::sync::Lazy;

pub use class_registry::{
    class_id_for_type, class_name_from_id, is_class_registered, register_builtin_class_names,
    register_class, register_class_name, type_id_from_class, ClassRegistryError,
};
pub use classid::{ClassID, RenderObjClassId};
pub use dllist::{DLListClass, DLListNode};
pub use errors::{W3DError, W3DResult};
pub use glam;
pub use w3d_format::*;
pub use w3d_obsolete::*;
pub use ww3d::{FrameStats, WW3DClass, WW3D};
pub use wwstring::StringClass;

// Re-export commonly used types
pub use animation::{
    AnimationChannel, AnimationController, AnimationInstance, AnimationMode, Hierarchy,
    HierarchyAnimation, Pivot,
};
pub use asset_manager::{
    global_asset_manager, AssetHandle, AssetLoader, AssetManager, AssetStatus,
};
pub use lighting::{Attenuation, Light, LightEnvironment, LightType};
pub use material::{
    BlendMode, Color, CullMode, DepthCompare, MaterialInfo, MaterialLibrary, MaterialPass, Shader,
    ShaderType, TextureStage, VertexMaterial,
};
pub use mesh::{
    create_cube_mesh, create_quad_mesh, Mesh, MeshBuilder, MeshGeometry, Triangle, Vertex,
};
pub use render_object::{
    AABox, BoundingSphere, Ray, RayCollisionResult, RenderHook, RenderInfo, RenderObject,
    RenderObjectCollection, RenderObjectRef, SpecialRenderInfo, SpecialRenderMode,
};
pub use scene::{Camera, Frustum, Layer, ProjectionType, Scene, SceneBuilder};
pub use texture::{
    create_checkerboard_texture, create_solid_color_texture, Texture, TextureAnimation,
    TextureAnimationType, TextureData, TextureDimensions, TextureFormat, TextureLoader,
    TextureManager,
};
pub use w3d_io::{load_w3d_file, save_w3d_file, W3DChunk, W3DReader, W3DWriter};

static CLASS_REGISTRY_INIT: Lazy<()> = Lazy::new(|| {
    register_builtin_class_names();
});

/// Ensures the WW3D class registry has been populated with the builtin tables.
pub fn ensure_class_registry_initialized() {
    Lazy::force(&CLASS_REGISTRY_INIT);
}

/// Prototype base class for shared asset data
#[derive(Debug)]
pub struct Prototype {
    pub id: u32,
    pub data: Vec<u8>, // Placeholder for shared data - will be replaced by specific types
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_type() {
        assert_eq!(W3DChunkType::Mesh as u32, 0x00000000);
    }

    #[test]
    fn test_prototype_creation() {
        let proto = Prototype {
            id: 1,
            data: vec![1, 2, 3],
        };
        assert_eq!(proto.id, 1);
        assert_eq!(proto.data, vec![1, 2, 3]);
    }
}
