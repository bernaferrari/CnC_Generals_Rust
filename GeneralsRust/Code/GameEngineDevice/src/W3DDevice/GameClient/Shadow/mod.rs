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
    do_shadows, the_w3d_shadow_manager, Frustum, RenderInfo, RenderObject, ShadowHandle,
    ShadowType, ShadowTypeInfo, TimeOfDay, W3DShadowManager, MAX_SHADOW_LIGHTS,
    SUN_DISTANCE_FROM_GROUND,
};

pub use wthree_d_volumetric_shadow::{
    the_w3d_volumetric_shadow_manager, AABBox, Geometry, NeighborEdge, PolyNeighbor,
    ShadowGeometry, ShadowGeometryMesh, Sphere, TriIndex, VisibleState, W3DVolumetricShadow,
    W3DVolumetricShadowManager, MAX_SHADOW_CASTER_MESHES, MAX_SILHOUETTE_EDGES, SHADOW_INDEX_SIZE,
    SHADOW_VERTEX_SIZE,
};

pub use wthree_d_projected_shadow::{
    the_projected_shadow_manager, the_w3d_projected_shadow_manager, ShadowDecalVertex,
    ShadowVolumeVertex, TextureHandle, W3DProjectedShadow, W3DProjectedShadowManager,
    W3DShadowTexture, W3DShadowTextureManager, DEFAULT_RENDER_TARGET_HEIGHT,
    DEFAULT_RENDER_TARGET_WIDTH,
};
