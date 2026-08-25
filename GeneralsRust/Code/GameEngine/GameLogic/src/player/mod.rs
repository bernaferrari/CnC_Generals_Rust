//! Player system - Complete Rust conversion of C++ Player class
//!
//! A "Player" is an entity that contains the persistent info of the Player, as well as containing
//! transient mission data. Some attributes persist between missions, whereas others are "transient" and only
//! have meaning in a mission, wherein they change a lot (current tech tree state, current buildings
//! built, units trained, money, etc).
//!
//! A "Player" consists of an entity controlling a single set of units in a mission.
//! A Player may be human or computer controlled.

// Restricted re-exports so impl submodules can `use super::*;`
// without dumping the parent crate surface through `pub use`.
pub(in crate::player) use crate::ai::AIGroup;
pub(in crate::player) use crate::build_list_info::BuildListInfo;
pub(in crate::player) use crate::common::ThingTemplate;
pub(in crate::player) use crate::common::*;
pub(in crate::player) use crate::helpers::TheGameLogic;
pub(in crate::player) use crate::modules::AIUpdateInterfaceExt;
pub(in crate::player) use crate::object::Object;
pub(in crate::player) use crate::object::behavior::battle_plan_update::BattlePlanBonuses;
pub(in crate::player) use crate::object::special_power_template::SpecialPowerTemplate;
pub(in crate::player) use crate::object_manager::get_object_manager;
pub(in crate::player) use crate::special_power_module::integration::{FrameCount, PlayerInterface};
pub(in crate::player) use crate::special_power_module::types::SpecialPowerID;
pub(in crate::player) use crate::squad::Squad;
pub(in crate::player) use crate::supply_system::ResourceGatheringManager;
pub(in crate::player) use crate::team::{
    Team, TeamID, TeamPrototype, TeamRelationMap, get_team_factory,
};
pub(in crate::player) use crate::tunnel_tracker::TunnelTracker;
pub(in crate::player) use crate::upgrade::{PlayerUpgradeManager, Upgrade, UpgradeTemplate};
pub(in crate::player) use game_engine::common::global_data;
pub(in crate::player) use game_engine::common::ini::ensure_player_templates_loaded;
pub(in crate::player) use game_engine::common::name_key_generator::{
    NAMEKEY_INVALID, NameKeyGenerator,
};
pub(in crate::player) use game_engine::common::rts::player_template::{
    MAX_MP_STARTING_UNITS, get_player_template_store,
};
pub(in crate::player) use game_engine::common::rts::science::get_science_store;
pub(in crate::player) use game_engine::common::rts::score_keeper::{
    KindOf as ScoreKindOf, KindOfMaskType as ScoreKindOfMaskType,
};
pub(in crate::player) use game_engine::common::rts::{
    Money, SCIENCE_INVALID, ScienceAccess, ScienceType,
};
pub(in crate::player) use game_engine::common::system::snapshot::Snapshotable;
pub(in crate::player) use game_engine::common::system::xfer::{Xfer, XferMode, XferVersion};
pub(in crate::player) use lazy_static::lazy_static;
pub(in crate::player) use std::collections::HashMap;
pub(in crate::player) use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Player index type (matching C++ PlayerIndex)
pub type PlayerIndex = Int;
pub const PLAYER_INDEX_INVALID: PlayerIndex = -1;

/// Money interface (matching C++ MoneyInterface usage).
pub trait MoneyInterface: Send + Sync {
    fn count_money(&self) -> i32;
}

/// Maximum number of hotkey squads
pub const NUM_HOTKEY_SQUADS: usize = 10;

/// Invalid hotkey squad constant
pub const NO_HOTKEY_SQUAD: PlayerIndex = -1;

/// Player types (matching C++ PlayerType: HUMAN=0, COMPUTER=1)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PlayerType {
    Human = 0,
    Computer = 1,
    Observer = 2,
    Neutral = 3,
}

/// Game difficulty levels (matching C++ GameDifficulty: EASY=0, NORMAL=1, HARD=2)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GameDifficulty {
    Easy = 0,
    Normal = 1,
    Hard = 2,
    Brutal = 3,
}

/// Science availability types (matching C++ ScienceAvailabilityType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScienceAvailabilityType {
    Available,
    Disabled,
    Hidden,
}

/// Battle plan types (matching C++ battle plan system)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattlePlanType {
    Bombard,
    HoldTheLine,
    SearchAndDestroy,
}

/// Science vector type
pub type ScienceVec = Vec<ScienceType>;

/// Command source constant for AI commands (matching C++ CMD_FROM_AI)
pub const CMD_FROM_AI: CommandSourceType = CommandSourceType::FromAi;

/// Wave 268: skip only when BOTH the dual-world registry and GameLogic are empty.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    if !crate::object::registry::OBJECT_REGISTRY.is_empty() {
        return false;
    }
    match crate::system::game_logic::get_game_logic().try_lock() {
        Ok(logic) => logic.get_object_count() == 0,
        Err(_) => false,
    }
}

/// Type aliases and constants for compatibility
pub type NameKeyType = game_engine::common::thing::module::NameKeyType;

mod money;
pub use money::*;
mod energy_handicap;
pub use energy_handicap::*;
mod academy_score;
pub use academy_score::*;
mod template;
pub use template::*;
mod core;
pub use core::*;
mod objects;
pub use objects::*;
mod economy;
pub use economy::*;
mod production;
pub use production::*;
mod relations;
pub use relations::*;
mod snapshot;
pub use snapshot::*;
mod list;
pub use list::*;

pub mod manager;
pub mod science_management;
pub mod science_ui;

// Re-export UI types for convenience (was at the bottom of player.rs).
pub use science_ui::{
    LevelUpNotification, PurchasableScienceInfo, RankProgressInfo, ScienceTreeUIData,
};

#[cfg(test)]
mod tests;

/// Concatenated live sources for residual `include_str!` scans.
pub const PLAYER_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("academy_score.rs"),
    include_str!("core.rs"),
    include_str!("economy.rs"),
    include_str!("energy_handicap.rs"),
    include_str!("list.rs"),
    include_str!("manager.rs"),
    include_str!("money.rs"),
    include_str!("objects.rs"),
    include_str!("production.rs"),
    include_str!("relations.rs"),
    include_str!("science_management.rs"),
    include_str!("science_ui.rs"),
    include_str!("snapshot.rs"),
    include_str!("template.rs"),
);
