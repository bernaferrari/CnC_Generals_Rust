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
//! Public types and impls remain identical. The sibling `w3d_model_draw.rs`
//! file is a scan dump only (not compiled).

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
    game_client_random_value, game_client_random_value_real, BoneOverrideState,
    MeshUvOverrideState, ModelDrawState, SubObjectVisibilityState, TheGameClient, TheGameLogic,
    TheParticleSystemManager,
};
use crate::upgrade::modules::model_condition::parse_model_condition_flag;
use game_engine::common::ini::{INIError, INI};
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
include!("impl_anim.rs");
include!("trait_impl.rs");
include!("snapshot.rs");
include!("parse.rs");
include!("constants.rs");
include!("tests.rs");
