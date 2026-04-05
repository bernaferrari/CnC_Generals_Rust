//! W3D Shadow System - Re-export from Shadow module
//!
//! Corresponds to C++ file: GameEngineDevice/Include/W3DDevice/GameClient/W3DShadow.h
//!
//! This module re-exports the shadow system implementation from the Shadow subdirectory.

// Re-export all types from the Shadow module
pub use super::shadow::{
    do_shadows, get_light_pos_world, set_light_pos_world, the_w3d_shadow_manager, Frustum,
    RenderInfo, RenderObject, ShadowColor, ShadowHandle, ShadowType, ShadowTypeInfo, TimeOfDay,
    W3DShadow, W3DShadowManager, MAX_SHADOW_LIGHTS, SUN_DISTANCE_FROM_GROUND,
};
