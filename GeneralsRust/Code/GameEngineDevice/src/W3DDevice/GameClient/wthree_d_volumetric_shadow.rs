//! W3D Volumetric Shadow System - Re-export from Shadow module
//!
//! Corresponds to C++ file: GameEngineDevice/Include/W3DDevice/GameClient/W3DVolumetricShadow.h
//!
//! This module re-exports the volumetric shadow system implementation from the Shadow subdirectory.

// Re-export all types from the Shadow module
pub use super::shadow::{
    W3DVolumetricShadowManager, W3DVolumetricShadow, ShadowGeometry, ShadowGeometryMesh,
    Geometry, PolyNeighbor, NeighborEdge, VisibleState, AABBox, Sphere,
    MAX_SHADOW_CASTER_MESHES, MAX_SILHOUETTE_EDGES, SHADOW_EXTRUSION_BUFFER,
    AIRBORNE_UNIT_GROUND_DELTA, MAX_SHADOW_LENGTH_SCALE_FACTOR,
    MAX_SHADOW_LENGTH_EXTRA_AIRBORNE_SCALE_FACTOR, MAX_EXTRUSION_LENGTH,
    MAX_SHADOW_EXTRUSION_UNDER_OBJECT_CLAMP, SHADOW_SAMPLING_INTERVAL,
    OVERHANGING_OBJECT_CLAMP_ANGLE, COS_ANGLE_TO_CARE, MAX_SHADOW_VOLUME_VERTS,
    SHADOW_VERTEX_SIZE, SHADOW_INDEX_SIZE, POLY_VISIBLE, POLY_PROCESSED,
    NO_NEIGHBOR, MAX_POLYGON_NEIGHBORS,
    the_w3d_volumetric_shadow_manager, ShadowRenderTask, ShadowGeometryManager,
};
