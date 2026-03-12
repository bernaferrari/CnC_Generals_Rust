//! W3D Shadow System - Re-export from Shadow module
//!
//! Corresponds to C++ file: GameEngineDevice/Include/W3DDevice/GameClient/W3DShadow.h
//!
//! This module re-exports the shadow system implementation from the Shadow subdirectory.

// Re-export all types from the Shadow module
pub use super::shadow::{
    W3DShadowManager, W3DShadow, ShadowType, ShadowColor, ShadowTypeInfo,
    ShadowHandle, RenderObject, RenderInfo, Frustum, TimeOfDay,
    SUN_DISTANCE_FROM_GROUND, MAX_SHADOW_LIGHTS,
    get_light_pos_world, set_light_pos_world, the_w3d_shadow_manager, do_shadows,
};
