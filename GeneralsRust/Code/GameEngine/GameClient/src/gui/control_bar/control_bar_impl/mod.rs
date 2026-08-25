//! Control Bar Implementation
//!
//! Rust conversion of ControlBar.cpp + ControlBarCommand.cpp - the main control bar system
//! that provides context-sensitive command interface for the game.
//!
//! Original C++ files:
//!   GameClient/GUI/ControlBar/ControlBar.cpp
//!   GameClient/GUI/ControlBar/ControlBarCommand.cpp
//! Original Author: Colin Day, March 2002

use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use super::{
    BuildQueueEntry, CommandAvailability, CommandButton, CommandOption, CommandSourceType,
    ControlBarContext, ControlBarState, MAX_BUILD_QUEUE_BUTTONS, ProductionItem, ProductionType,
    QueueProductionType,
};
use crate::game_text::GameText;
use crate::gui::{GameWindow, WindowManager, with_window_manager};

use crate::helpers::{
    TheInGameUI, drain_live_control_bar_events, set_live_control_bar_observer_look_at,
};
use crate::message_stream::game_message::GameMessageType;
use crate::message_stream::hot_key::with_hot_key_manager;
use crate::message_stream::message_stream::THE_MESSAGE_STREAM;
use crate::system::SubsystemInterface;
use game_engine::common::ini::ini_command_button::{
    CommandButton as IniCommandButton, get_control_bar as get_ini_control_bar,
};
use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
use game_engine::common::rts::{SCIENCE_INVALID, ScienceType, WeaponSlotType, get_science_store};
use gamelogic::command_button::map_gui_command_to_command_type;
use gamelogic::commands::command::CommandType;
use gamelogic::commands::{Command, CommandPriority, QueuedCommand, get_command_queue_manager};
use gamelogic::common::GameError;
use gamelogic::common::types::{KindOf, OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION};
use gamelogic::control_bar::get_control_bar_bridge;
use gamelogic::helpers::{TheGameLogic, TheThingFactory};
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::player::{PlayerIndex, player_list as logic_player_list};
use gamelogic::system::beacon_manager::snapshot_beacons;
use gamelogic::upgrade::center::with_upgrade_center;

// Live `control_bar` module via `#[path = "control_bar_impl/mod.rs"]`.
// `include!` keeps one logical module so field privacy and the public API
// stay identical to the former dump.

include!("types.rs");
include!("impl_presentation_availability.rs");
include!("impl_lifecycle.rs");
include!("impl_context.rs");
include!("impl_command_context.rs");
include!("impl_execute.rs");
include!("impl_buttons.rs");
include!("impl_contexts.rs");
include!("impl_portrait.rs");
include!("impl_science.rs");
include!("traits.rs");
include!("tests.rs");

/// Concatenated live sources for residual `include_str!` scans.
pub const CONTROL_BAR_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("types.rs"),
    include_str!("impl_presentation_availability.rs"),
    include_str!("impl_lifecycle.rs"),
    include_str!("impl_context.rs"),
    include_str!("impl_command_context.rs"),
    include_str!("impl_execute.rs"),
    include_str!("impl_buttons.rs"),
    include_str!("impl_contexts.rs"),
    include_str!("impl_portrait.rs"),
    include_str!("impl_science.rs"),
    include_str!("traits.rs"),
);
