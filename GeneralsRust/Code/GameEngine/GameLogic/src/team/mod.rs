//! Team system - Complete Rust conversion of C++ Team class
//!
//! Teams manage groups of objects that work together, handle ownership,
//! relationships, and provide team-level functionality. This includes both
//! individual Team instances and TeamPrototypes for creating new teams.
//!
//! Split into focused submodules by team concern.

use crate::ai::AIGroup;
use crate::common::CoordOrigin;
use crate::common::*;
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::{ThePartitionManager, TheThingFactory};
use crate::locomotor::core::{LocomotorSurfaceTypeMask, SURFACE_GROUND};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object_manager::get_object_manager;
use crate::player::{PLAYER_INDEX_INVALID, player_list};
use crate::polygon_trigger::PolygonTrigger;
use crate::scripting::core::Script;
use crate::scripting::engine::{get_area_tracker, get_script_engine};
use crate::scripting::evaluator::ScriptEvaluator;
use crate::waypoint::WaypointId;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::snapshot::Snapshotable;
use game_engine::common::system::xfer::{Xfer, XferMode, XferVersion};
use game_engine::common::well_known_keys::{
    key_team_aggressiveness, key_team_all_clear_script, key_team_attack_common_target,
    key_team_auto_reinforce, key_team_avoid_threats, key_team_destroyed_threshold,
    key_team_enemy_sighted_script, key_team_executes_actions_on_create,
    key_team_generic_script_hook, key_team_home, key_team_initial_idle_frames,
    key_team_is_ai_recruitable, key_team_is_base_defense, key_team_is_perimeter_defense,
    key_team_max_instances, key_team_name, key_team_on_create_script, key_team_on_destroyed_script,
    key_team_on_idle_script, key_team_on_unit_destroyed_script, key_team_owner,
    key_team_production_condition, key_team_production_priority,
    key_team_production_priority_failure_decrease, key_team_production_priority_success_increase,
    key_team_reinforcement_origin, key_team_starts_full, key_team_transport,
    key_team_transports_exit, key_team_transports_return, key_team_unit_max_count1,
    key_team_unit_max_count2, key_team_unit_max_count3, key_team_unit_max_count4,
    key_team_unit_max_count5, key_team_unit_max_count6, key_team_unit_max_count7,
    key_team_unit_min_count1, key_team_unit_min_count2, key_team_unit_min_count3,
    key_team_unit_min_count4, key_team_unit_min_count5, key_team_unit_min_count6,
    key_team_unit_min_count7, key_team_unit_type1, key_team_unit_type2, key_team_unit_type3,
    key_team_unit_type4, key_team_unit_type5, key_team_unit_type6, key_team_unit_type7,
};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{
    Arc, LockResult, Mutex, MutexGuard, OnceLock, PoisonError, RwLock, TryLockError, TryLockResult,
    Weak,
};

include!("ids.rs");
include!("team_struct.rs");
include!("team_identity.rs");
include!("team_state.rs");
include!("team_members.rs");
include!("team_areas.rs");
include!("team_actions.rs");
include!("snapshot.rs");
include!("prototype.rs");
include!("factory.rs");
include!("factory_access.rs");
include!("tests.rs");

/// Concatenated live sources for residual `include_str!` scans.
pub const TEAM_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("factory.rs"),
    include_str!("factory_access.rs"),
    include_str!("ids.rs"),
    include_str!("prototype.rs"),
    include_str!("snapshot.rs"),
    include_str!("team_actions.rs"),
    include_str!("team_areas.rs"),
    include_str!("team_identity.rs"),
    include_str!("team_members.rs"),
    include_str!("team_state.rs"),
    include_str!("team_struct.rs"),
);
