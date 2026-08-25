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
    W3D_CHUNK_ADAPTIVEDELTA_CHANNEL, W3D_CHUNK_AGGREGATE, W3D_CHUNK_AGGREGATE_CLASS_INFO,
    W3D_CHUNK_AGGREGATE_HEADER, W3D_CHUNK_AGGREGATE_INFO, W3D_CHUNK_ANIMATION,
    W3D_CHUNK_ANIMATION_CHANNEL, W3D_CHUNK_ANIMATION_HEADER, W3D_CHUNK_BIT_CHANNEL, W3D_CHUNK_BOX,
    W3D_CHUNK_COLLECTION, W3D_CHUNK_COLLECTION_HEADER, W3D_CHUNK_COLLECTION_OBJ_NAME,
    W3D_CHUNK_COMPRESSED_ANIMATION, W3D_CHUNK_COMPRESSED_ANIMATION_CHANNEL,
    W3D_CHUNK_COMPRESSED_ANIMATION_HEADER, W3D_CHUNK_COMPRESSED_BIT_CHANNEL, W3D_CHUNK_DAZZLE,
    W3D_CHUNK_DAZZLE_NAME, W3D_CHUNK_DAZZLE_TYPENAME, W3D_CHUNK_EMITTER, W3D_CHUNK_EMITTER_HEADER,
    W3D_CHUNK_EMITTER_INFO, W3D_CHUNK_EMITTER_INFOV2, W3D_CHUNK_EMITTER_LINE_PROPERTIES,
    W3D_CHUNK_EMITTER_PROPS, W3D_CHUNK_EMITTER_USER_DATA, W3D_CHUNK_HIERARCHY,
    W3D_CHUNK_HIERARCHY_HEADER, W3D_CHUNK_HLOD, W3D_CHUNK_HMODEL, W3D_CHUNK_HMODEL_HEADER,
    W3D_CHUNK_LOD, W3D_CHUNK_LODMODEL, W3D_CHUNK_LODMODEL_HEADER, W3D_CHUNK_MATERIAL_INFO,
    W3D_CHUNK_MATERIAL_PASS, W3D_CHUNK_MESH, W3D_CHUNK_MESH_HEADER3, W3D_CHUNK_MESH_USER_TEXT,
    W3D_CHUNK_NODE, W3D_CHUNK_NULL_OBJECT, W3D_CHUNK_PIVOT_FIXUPS, W3D_CHUNK_PIVOTS,
    W3D_CHUNK_PLACEHOLDER, W3D_CHUNK_POINTS, W3D_CHUNK_PRELIT_LIGHTMAP_MULTI_PASS,
    W3D_CHUNK_PRELIT_LIGHTMAP_MULTI_TEXTURE, W3D_CHUNK_PRELIT_UNLIT, W3D_CHUNK_PRELIT_VERTEX,
    W3D_CHUNK_RING, W3D_CHUNK_SHADER_IDS, W3D_CHUNK_SHADERS, W3D_CHUNK_SPHERE,
    W3D_CHUNK_TEXTURE_REPLACER_INFO, W3D_CHUNK_TEXTURES, W3D_CHUNK_TIMECODED_CHANNEL,
    W3D_CHUNK_TRANSFORM_NODE, W3D_CHUNK_TRIANGLES, W3D_CHUNK_VERTEX_INFLUENCES,
    W3D_CHUNK_VERTEX_NORMALS, W3D_CHUNK_VERTEX_SHADE_INDICES, W3D_CHUNK_VERTICES, W3DChunkType,
};
use once_cell::sync::Lazy;

pub use class_registry::{
    ClassRegistryError, class_id_for_type, class_name_from_id, is_class_registered,
    register_builtin_class_names, register_class, register_class_name, type_id_from_class,
};
pub use classid::{ClassID, RenderObjClassId};
pub use dllist::{DLListClass, DLListNode};
pub use errors::{W3DError, W3DResult};
pub use glam;
pub use w3d_format::*;
pub use w3d_obsolete::*;
pub use ww3d::{FrameStats, PrelitMode, WW3D, WW3DClass};
pub use wwstring::StringClass;

// Re-export commonly used types
pub use animation::{
    AnimationChannel, AnimationController, AnimationInstance, AnimationMode, Hierarchy,
    HierarchyAnimation, Pivot,
};
pub use asset_manager::{
    AssetHandle, AssetLoader, AssetManager, AssetStatus, global_asset_manager,
};
pub use lighting::{Attenuation, Light, LightEnvironment, LightType};
pub use material::{
    BlendMode, Color, CullMode, DepthCompare, MaterialInfo, MaterialLibrary, MaterialPass, Shader,
    ShaderType, TextureStage, VertexMaterial,
};
pub use mesh::{
    Mesh, MeshBuilder, MeshGeometry, Triangle, Vertex, create_cube_mesh, create_quad_mesh,
};
pub use render_object::{
    AABox, BoundingSphere, Ray, RayCollisionResult, RenderHook, RenderInfo, RenderObject,
    RenderObjectCollection, RenderObjectRef, SpecialRenderInfo, SpecialRenderMode,
};
pub use scene::{Camera, Frustum, Layer, ProjectionType, Scene, SceneBuilder};
pub use texture::{
    Texture, TextureAnimation, TextureAnimationType, TextureData, TextureDimensions, TextureFormat,
    TextureLoader, TextureManager, create_checkerboard_texture, create_solid_color_texture,
};
pub use w3d_io::{W3DChunk, W3DReader, W3DWriter, load_w3d_file, save_w3d_file};

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
