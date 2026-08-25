#![allow(missing_docs)]

//! Message Stream Translators
//!
//! This module contains implementations of various message translators that
//! convert raw input events into tactical commands and game actions.
//!
//! Translators are the heart of the message stream system, processing messages
//! in priority order and deciding whether to keep, modify, or destroy them.

// Restricted re-exports so impl submodules can `use super::*;`
// without dumping the parent crate surface through `pub use`.
pub(in crate::message_stream::translators) use super::command_list::get_command_list;
pub(in crate::message_stream::translators) use super::game_message::*;
pub(in crate::message_stream::translators) use super::hot_key::HotKeyTranslator;
pub(in crate::message_stream::translators) use super::look_at_xlat::LookAtTranslator;
pub(in crate::message_stream::translators) use super::message_stream::{
    GameMessageDisposition, GameMessageTranslator,
};
pub(in crate::message_stream::translators) use super::meta_event::MetaEventTranslator;
pub(in crate::message_stream::translators) use super::place_event_translator::PlaceEventTranslator;
pub(in crate::message_stream::translators) use super::player_state::get_local_player_id;
pub(in crate::message_stream::translators) use super::selection_xlat::SelectionTranslator as SelectionTranslatorXlat;
pub(in crate::message_stream::translators) use super::window_xlat::WindowTranslator;
pub(in crate::message_stream::translators) use crate::core::game_client::CommandEvaluateType as ClientCommandEvaluateType;
pub(in crate::message_stream::translators) use crate::display::view::{
    IPoint2, Point3, with_tactical_view, with_tactical_view_ref,
};
pub(in crate::message_stream::translators) use crate::drawable::Drawable;
pub(in crate::message_stream::translators) use crate::gui::game_window::WindowStatus;
pub(in crate::message_stream::translators) use crate::gui::window_manager::with_window_manager_ref;
pub(in crate::message_stream::translators) use crate::gui::{
    toggle_control_bar, toggle_diplomacy, toggle_quit_menu,
};
pub(in crate::message_stream::translators) use crate::helpers::{PendingCommand, TheInGameUI};
pub(in crate::message_stream::translators) use crate::input::KeyModifiers;
pub(in crate::message_stream::translators) use crate::presentation_translator_residual::{
    translator_catalog_entry, translator_catalog_has_kind, translator_entry_apparent_team,
    translator_entry_has_kind, translator_entry_is_local, translator_local_team_name,
    with_translator_catalog,
};
pub(in crate::message_stream::translators) use crate::system::GameMessageResult;
pub(in crate::message_stream::translators) use crate::system::beacon_display;
pub(in crate::message_stream::translators) use game_engine::common::game_engine::get_game_engine;
pub(in crate::message_stream::translators) use game_engine::common::ini::ini_game_data::get_global_data;
pub(in crate::message_stream::translators) use game_engine::common::system::radar::get_radar_system;
pub(in crate::message_stream::translators) use gamelogic::action_manager::{
    ActionManager, CanEnterType,
};
pub(in crate::message_stream::translators) use gamelogic::attack::{
    AbleToAttackType, CanAttackResult,
};
pub(in crate::message_stream::translators) use gamelogic::commands::command::CommandType;
pub(in crate::message_stream::translators) use gamelogic::commands::get_selection_manager;
pub(in crate::message_stream::translators) use gamelogic::common::Coord3D as LogicCoord3D;
pub(in crate::message_stream::translators) use gamelogic::common::{
    CommandSourceType, KindOf, ObjectShroudStatus,
    ObjectStatusMaskType as LogicObjectStatusMaskType, Relationship,
};
pub(in crate::message_stream::translators) use gamelogic::damage::DamageType;
pub(in crate::message_stream::translators) use gamelogic::helpers::{
    TheGameLogic, TheTerrainLogic,
};
pub(in crate::message_stream::translators) use gamelogic::object::registry::OBJECT_REGISTRY;
pub(in crate::message_stream::translators) use gamelogic::object::special_power_template::{
    SpecialPowerTemplate, get_special_power_store,
};
pub(in crate::message_stream::translators) use gamelogic::path::SURFACE_CLIFF;
pub(in crate::message_stream::translators) use gamelogic::player::player_list;
pub(in crate::message_stream::translators) use gamelogic::system::shroud_manager::{
    ShroudState, get_shroud_manager,
};
pub(in crate::message_stream::translators) use gamelogic::weapon::WeaponSlotType;
pub(in crate::message_stream::translators) use log::{debug, info, warn};
pub(in crate::message_stream::translators) use std::collections::{HashMap, HashSet};
pub(in crate::message_stream::translators) use std::sync::{Arc, RwLock};

mod dual_world;
pub use dual_world::*;
mod pick;
pub use pick::*;
mod flags;
pub use flags::*;
mod pending;
pub use pending::*;
mod attack;
pub use attack::*;
mod context;
pub use context::*;
mod command_translator;
pub use command_translator::*;
mod voice;
pub(crate) use voice::play_voice_for_command;
pub(in crate::message_stream::translators) use voice::{
    VoicePlayInfo, pick_and_play_unit_voice_response,
};
mod command_translate;
pub use command_translate::*;
mod gui_command;
pub use gui_command::*;
mod hint_spy;
pub use hint_spy::*;
mod factory;
pub use factory::*;
mod select_meta;
pub(in crate::message_stream::translators) use select_meta::*;

#[cfg(test)]
mod tests;

/// Concatenated live sources for residual `include_str!` scans.
pub const TRANSLATORS_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("dual_world.rs"),
    include_str!("pick.rs"),
    include_str!("flags.rs"),
    include_str!("pending.rs"),
    include_str!("attack.rs"),
    include_str!("context.rs"),
    include_str!("command_translator.rs"),
    include_str!("command_translate.rs"),
    include_str!("gui_command.rs"),
    include_str!("hint_spy.rs"),
    include_str!("factory.rs"),
);
