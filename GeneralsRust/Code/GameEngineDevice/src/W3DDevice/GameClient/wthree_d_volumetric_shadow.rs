//! W3D Volumetric Shadow System - Re-export from Shadow module
//!
//! Corresponds to C++ file: GameEngineDevice/Include/W3DDevice/GameClient/W3DVolumetricShadow.h
//!
//! This module re-exports the volumetric shadow system implementation from the Shadow subdirectory.

// Re-export all types from the Shadow module
pub use super::shadow::{
    the_w3d_volumetric_shadow_manager, AABBox, Geometry, NeighborEdge, PolyNeighbor,
    ShadowGeometry, ShadowGeometryManager, ShadowGeometryMesh, ShadowRenderTask, Sphere,
    VisibleState, W3DVolumetricShadow, W3DVolumetricShadowManager, AIRBORNE_UNIT_GROUND_DELTA,
    COS_ANGLE_TO_CARE, MAX_EXTRUSION_LENGTH, MAX_POLYGON_NEIGHBORS, MAX_SHADOW_CASTER_MESHES,
    MAX_SHADOW_EXTRUSION_UNDER_OBJECT_CLAMP, MAX_SHADOW_LENGTH_EXTRA_AIRBORNE_SCALE_FACTOR,
    MAX_SHADOW_LENGTH_SCALE_FACTOR, MAX_SHADOW_VOLUME_VERTS, MAX_SILHOUETTE_EDGES, NO_NEIGHBOR,
    OVERHANGING_OBJECT_CLAMP_ANGLE, POLY_PROCESSED, POLY_VISIBLE, SHADOW_EXTRUSION_BUFFER,
    SHADOW_INDEX_SIZE, SHADOW_SAMPLING_INTERVAL, SHADOW_VERTEX_SIZE,
};
