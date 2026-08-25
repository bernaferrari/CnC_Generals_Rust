//! W3DModelDraw - Main 3D model drawing module
//!
//! Port of C++ W3DModelDraw.h/cpp
//! Reference: /GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DModelDraw.h
//!
//! This is the primary draw module for rendering 3D models with:
//! - Model condition-based state switching
//! - Skeletal animation
//! - Weapon fire effects
//! - Particle systems
//! - Turret positioning
//! - Weapon recoil
//! - Shadows and decals
//!
//! Split from the former monolithic `object/draw/w3d_model_draw.rs`.
//! Public types and impls remain identical. Live module is `w3d_model_draw_impl/`
//! via `#[path = "w3d_model_draw_impl/mod.rs"]`.

#![allow(
    unused_imports,
    dead_code,
    unused_variables,
    unused_mut,
    unused_assignments,
    clippy::too_many_arguments
)]

use super::draw_module::*;
use crate::common::*;
use crate::helpers::{
    BoneOverrideState, MeshUvOverrideState, ModelDrawState, ModelDrawWeaponBoneBindings,
    SubObjectVisibilityState, TheGameClient, TheGameLogic, TheGlobalData, TheParticleSystemManager,
    game_client_random_value, game_client_random_value_real,
};
use crate::object::draw::client_visual::{
    TerrainDecalDesc, leftover_default_shadow_texture, object_should_animate, preload_draw_asset,
    terrain_decal_client, terrain_decal_texture_name, terrain_track_client,
};
use crate::upgrade::modules::model_condition::parse_model_condition_flag;
use game_engine::common::ini::{INI, INIError};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{
    Module, ModuleData, ModuleInterfaceType, ModuleType, TimeOfDay,
};
use log::warn;
use std::any::Any;
use std::collections::HashMap;

include!("types.rs");
include!("module_data.rs");
include!("recoil.rs");
include!("draw.rs");
include!("hlod_live_child.rs");
include!("impl_anim.rs");
include!("carrying.rs");
include!("hide_show.rs");
include!("anim_playback.rs");
include!("shadow_bind.rs");

include!("trait_impl.rs");
include!("snapshot.rs");
include!("parse.rs");
include!("constants.rs");
include!("tests.rs");

/// Concatenated live sources for residual `include_str!` scans.
pub const W3D_MODEL_DRAW_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("constants.rs"),
    include_str!("draw.rs"),
    include_str!("hlod_live_child.rs"),
    include_str!("impl_anim.rs"),
    include_str!("module_data.rs"),
    include_str!("parse.rs"),
    include_str!("recoil.rs"),
    include_str!("snapshot.rs"),
    include_str!("trait_impl.rs"),
    include_str!("types.rs"),
    include_str!("carrying.rs"),
    include_str!("hide_show.rs"),
    include_str!("anim_playback.rs"),
    include_str!("shadow_bind.rs"),
);
