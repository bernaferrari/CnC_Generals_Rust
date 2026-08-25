//! Script Evaluation System
//!
//! This module provides script evaluation logic that matches the C++ ScriptEngine evaluation,
//! including condition evaluation, action execution, and script state management.
//!
//! Split into focused submodules by condition/action family.

use super::core::*;
use super::engine::{ScriptActionHandler, get_area_tracker, get_named_object_tracker, *};
use super::executor::{
    ScriptActionDispatcher, ScriptActionResult, ScriptConditionEvaluator, ScriptConditionResult,
    ScriptContext,
};
use crate::commands::get_selection_manager;
use crate::common::{
    AsciiString, DisabledType, KindOf, LOGICFRAMES_PER_SECOND, ObjectID, ObjectShroudStatus,
    PlayerMaskType, UnsignedInt,
};
use crate::helpers::TheGameLogic;
use crate::modules::ContainModuleInterface;
use crate::object::object_types::ObjectTypes;
use crate::player::player_list;
use crate::polygon_trigger::PolygonTrigger;
use crate::team::get_team_factory;
use crate::terrain::get_terrain_logic;
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::{SCIENCE_INVALID, get_science_store};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Wave 343: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// Script Evaluator matching C++ ScriptEngine evaluation logic
pub struct ScriptEvaluator {
    engine: ScriptEngineHandle,
}

static TRANSPORT_STATUSES: Lazy<RwLock<HashMap<ObjectID, (UnsignedInt, usize)>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

include!("dispatch.rs");
include!("leftover.rs");
include!("eval_area.rs");
include!("eval_state.rs");
include!("eval_lifecycle.rs");
include!("eval_combat.rs");
include!("eval_player.rs");
include!("eval_unit.rs");
include!("actions.rs");

#[cfg(test)]
mod tests;

/// Concatenated live sources for residual `include_str!` scans.
pub const EVALUATOR_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("actions.rs"),
    include_str!("dispatch.rs"),
    include_str!("eval_area.rs"),
    include_str!("eval_combat.rs"),
    include_str!("eval_lifecycle.rs"),
    include_str!("eval_player.rs"),
    include_str!("eval_state.rs"),
    include_str!("eval_unit.rs"),
    include_str!("leftover.rs"),
);
