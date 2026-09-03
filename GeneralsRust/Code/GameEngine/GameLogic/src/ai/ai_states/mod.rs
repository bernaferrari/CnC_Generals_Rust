//! AIStates - Complete AI state machine implementation
//!
//! This module implements the complete AI state machine system from the C++ original.
//! It provides all AI behavior states including movement, combat, guarding, pathfinding,
//! and complex tactical behaviors. The state machine drives all AI unit behavior.
//!
//! Author: Converted from C++ original by Michael S. Booth
//!
//! Split from the former monolithic `ai/ai_states.rs` into focused submodules.
//! Public types and impls remain identical.

#![allow(
    unused_imports,
    dead_code,
    unused_variables,
    unused_mut,
    unused_assignments,
    clippy::too_many_arguments
)]

use crate::ai::GuardMode;
use crate::ai::dock::AIDockMachine;
use crate::ai::formations::{
    FormationConfig, FormationType, calculate_group_spread, is_group_too_spread,
};
use crate::ai::guard::{AIGuardMachine, GuardStateType};
use crate::ai::guard_retaliate::{AIGuardRetaliateMachine, GuardRetaliateStateType};
use crate::ai::integration::with_ai_integration;
use crate::ai::object_registry::get_legacy_object;
use crate::ai::states::{AIAttackThenIdleStateMachine, AIStateType as LegacyAIStateType};
use crate::ai::tn_guard::{AITNGuardMachine, TNGuardStateType};
use crate::ai::{AiCommandInterface, AiCommandParams, AiCommandType, AiError};
use crate::ai::{the_ai, resolve_attack_priority_info_for_object, search_qualifiers};
use crate::common::{
    CommandSourceType, Coord2D, Coord3D, INVALID_ID, KindOf, LOGICFRAMES_PER_SECOND,
    LocomotorSetType, ModelConditionFlags, ObjectID, ObjectStatusMaskType, ObjectStatusTypes, Real,
    Relationship, TurretType,
};
use crate::damage::DamageInfo;
use crate::helpers::{
    TheGameLogic, TheTerrainLogic, game_logic_random_value, get_game_logic_random_value,
};
use crate::modules::{AIUpdateInterfaceExt, ContainWant};
use crate::object::Object as GameObject;
use crate::object::registry::OBJECT_REGISTRY;
use crate::path::PATHFIND_CLOSE_ENOUGH;
use crate::path::{PATHFIND_CELL_SIZE_F, SURFACE_GROUND};
use crate::player::PlayerType;
use crate::state_machine::{StateExitType, StateReturnType};
use crate::terrain::get_terrain_logic;
use crate::weapon::WeaponChoiceCriteria;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

include!("types.rs");
include!("move_states.rs");
include!("path_dock.rs");
include!("enter_hack.rs");
include!("face_special.rs");
include!("combat.rs");
include!("wander.rs");
include!("hunt.rs");
include!("state_machine.rs");

include!("tests.rs");

/// Concatenated live sources for residual `include_str!` scans.
pub const AI_STATES_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("combat.rs"),
    include_str!("enter_hack.rs"),
    include_str!("face_special.rs"),
    include_str!("hunt.rs"),
    include_str!("move_states.rs"),
    include_str!("path_dock.rs"),
    include_str!("state_machine.rs"),
    include_str!("types.rs"),
    include_str!("wander.rs"),
);
