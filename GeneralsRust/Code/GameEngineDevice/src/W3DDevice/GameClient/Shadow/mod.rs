//! W3D Shadow System Module
//!
//! This module provides shadow rendering functionality for the W3D rendering engine.
//! Corresponds to C++ files in GameEngineDevice/Source/W3DDevice/GameClient/Shadow/
//!
//! Components:
//! - wthree_d_shadow: Main shadow manager and coordination
//! - wthree_d_volumetric_shadow: Stencil-based shadow volumes
//! - wthree_d_projected_shadow: Texture-based projected shadows and decals
//! - wthree_d_buffer_manager: GPU buffer management for shadows

pub mod wthree_d_buffer_manager;
pub mod wthree_d_projected_shadow;
pub mod wthree_d_shadow;
pub mod wthree_d_volumetric_shadow;

// Re-export main types for convenience
pub use wthree_d_shadow::{
    W3DShadowManager, ShadowType, ShadowTypeInfo, ShadowHandle,
    RenderObject, RenderInfo, Frustum, TimeOfDay,
    the_w3d_shadow_manager, do_shadows, 
    SUN_DISTANCE_FROM_GROUND, MAX_SHADOW_LIGHTS,
};

pub use wthree_d_volumetric_shadow::{
    W3DVolumetricShadow, W3DVolumetricShadowManager,
    ShadowGeometry, ShadowGeometryMesh, Geometry,
    PolyNeighbor, NeighborEdge, VisibleState,
    AABBox, Sphere, TriIndex,
    the_w3d_volumetric_shadow_manager,
    MAX_SHADOW_CASTER_MESHES, MAX_SILHOUETTE_EDGES,
    SHADOW_VERTEX_SIZE, SHADOW_INDEX_SIZE,
};

pub use wthree_d_projected_shadow::{
    W3DProjectedShadow, W3DProjectedShadowManager,
    W3DShadowTexture, W3DShadowTextureManager,
    ShadowDecalVertex, ShadowVolumeVertex, TextureHandle,
    the_w3d_projected_shadow_manager, the_projected_shadow_manager,
    DEFAULT_RENDER_TARGET_WIDTH, DEFAULT_RENDER_TARGET_HEIGHT,
};
