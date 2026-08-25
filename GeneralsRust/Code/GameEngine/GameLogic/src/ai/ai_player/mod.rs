//! AIPlayer - Computer player AI system
//!
//! This module implements the computerized opponent AI that manages all aspects
//! of computer player behavior including economy, military, construction, and
//! strategic decision making.
//!
//! Author: Converted from C++ original by Michael S. Booth
//!
//! Split from the former monolithic `ai/ai_player.rs` into focused submodules.
//! Public types and impls remain identical.

#![allow(
    unused_imports,
    dead_code,
    unused_variables,
    hidden_glob_reexports,
    ambiguous_glob_reexports
)]

use super::SkillSet;
use super::ai_update::AiPlayerTrait;
use crate::ai::modules::GameDifficulty as AiGameDifficulty;
use crate::ai::modules::{
    BuildOrderOptimizer, DifficultyHandler, StrategicDecision, StrategicDecisionMaker,
    ThreatAssessmentSystem,
};
use crate::ai::{AI, AiError, AiGroup, AttitudeType, ScienceType, THE_AI};
use crate::ai::{CommandSourceType, GuardMode};
use crate::common::Snapshot;
use crate::common::xfer::{Xfer, XferExt};
use crate::common::{
    AsciiString, ControlBarInterface, Coord2D, Coord3D, CoordOrigin, INVALID_ID, KindOf,
    LocomotorSetType, ObjectID, ObjectStatusMaskType, ObjectStatusTypes, PlayerId, Real,
    Relationship, TeamId, ThingTemplate, UnsignedInt,
};
use crate::control_bar::get_control_bar_bridge;
use crate::helpers::{
    TheGameLogic, ThePartitionManager, TheTerrainLogic, TheThingFactory, game_logic_random_value,
};
use crate::modules::AIUpdateInterfaceExt;
use crate::modules::ProductionUpdateInterface;
use crate::object::Object;
use crate::object::production::construction::FoundationValidator;
use crate::object::production::supply_warehouse_dock::SupplyWarehouseDockUpdate;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::player::{GameDifficulty, Player, PlayerType, player_list};
use crate::scripting::engine::get_script_engine;
use crate::scripting::evaluator::ScriptEvaluator;
use crate::supply_system::BASE_VALUE_PER_SUPPLY_BOX;
use crate::team::get_team_factory;
use crate::upgrade::center::with_upgrade_center;
use crate::upgrade::template::UpgradeType;
use game_engine::common::system::build_assistant::LocalLegalToBuildOptions;
use game_engine::common::thing::thing_factory::get_thing_factory;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};

/// Wave 255: host-only path has no dual-world factory objects.
#[inline]
pub(crate) fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

/// Convert the public GameDifficulty (from player module) to the AI-specific enum
pub(crate) fn to_ai_difficulty(diff: GameDifficulty) -> AiGameDifficulty {
    match diff {
        GameDifficulty::Easy => AiGameDifficulty::Easy,
        GameDifficulty::Normal => AiGameDifficulty::Normal,
        GameDifficulty::Hard => AiGameDifficulty::Hard,
        GameDifficulty::Brutal => AiGameDifficulty::Brutal,
    }
}

mod impl_build;
mod impl_dozer;
mod impl_economy;
mod impl_military;
mod impl_runtime;
mod impl_select;
mod impl_teams;
mod impl_update;
mod snapshot;
mod strategy;
mod team_in_queue;
mod trait_impl;
mod types;
mod work_order;

#[cfg(test)]
mod tests;

pub use strategy::*;
pub use team_in_queue::*;
pub use types::*;
pub use work_order::*;

/// Concatenated live sources for residual `include_str!` scans.
pub const AI_PLAYER_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("impl_build.rs"),
    include_str!("impl_dozer.rs"),
    include_str!("impl_economy.rs"),
    include_str!("impl_military.rs"),
    include_str!("impl_runtime.rs"),
    include_str!("impl_select.rs"),
    include_str!("impl_teams.rs"),
    include_str!("impl_update.rs"),
    include_str!("snapshot.rs"),
    include_str!("strategy.rs"),
    include_str!("team_in_queue.rs"),
    include_str!("trait_impl.rs"),
    include_str!("types.rs"),
    include_str!("work_order.rs"),
);
