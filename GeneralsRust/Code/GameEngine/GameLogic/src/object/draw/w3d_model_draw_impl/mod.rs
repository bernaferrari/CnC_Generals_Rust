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
include!("particles.rs");
include!("hlod_live_child.rs");
include!("impl_anim.rs");
include!("carrying.rs");
include!("hide_show.rs");
include!("anim_playback.rs");
include!("shadow_bind.rs");

include!("projectile.rs");
include!("trait_impl.rs");
include!("snapshot.rs");
include!("parse.rs");
include!("constants.rs");
include!("tests.rs");

#[cfg(test)]
mod receiving_state_particle_tests;

#[cfg(test)]
mod receiving_particle_tests;
