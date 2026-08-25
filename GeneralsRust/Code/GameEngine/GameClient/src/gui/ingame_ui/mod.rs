//! # In-Game UI System
//!
//! Comprehensive in-game user interface system ported from C++ InGameUI.cpp
//! Handles all in-game UI elements including selection, minimap, resource display,
//! and building placement preview.
//!
//! Original C++ file: GameClient/InGameUI.cpp
//! Original Author: Michael S. Booth, March 2001
//!
//! Live `ingame_ui` module. `include!` keeps one logical module so field
//! privacy and the public API stay identical to the former dump.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use game_engine::common::recorder::with_recorder;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use glam::{Vec2, Vec3};
use thiserror::Error;
use wgpu::TextureView;

use super::ui_renderer::{UIRect, UIRenderer, UIRendererError};
use super::window_video_manager::with_window_video_manager;
use crate::display::view::{IPoint2, Point3, with_tactical_view, with_tactical_view_ref};
use crate::game_text::GameText;
use crate::gui::callbacks::diplomacy::update_diplomacy_briefing_text;
use crate::gui::game_window::{GameWindow, WindowStatus};
use crate::gui::window_manager::with_window_manager_ref;
use crate::helpers::{PendingCommand, TheInGameUI};
use crate::input::keyboard::KeyboardState;
use crate::input::mouse::{ButtonState, MouseButton, MouseState, with_mouse};
use crate::message_stream::game_message::{
    Coord3D as MsgCoord3D, GameMessageType, ICoord2D as MsgICoord2D,
};
use crate::message_stream::message_stream::append_message_to_stream;
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::global_data;
use game_engine::common::ini::get_anim2d_collection;
use game_engine::common::ini::ini_language::{FontDesc, get_global_language_read};
use game_engine::common::thing::get_thing_factory;
use gamelogic::action_manager::ActionManager;
use gamelogic::commands::command::CommandType;
use gamelogic::commands::selection::{SelectionType, get_selection_manager};
use gamelogic::common::CommandSourceType;
use gamelogic::common::audio::AudioEventRts;
use gamelogic::common::{
    Coord3D, ICoord2D, IRegion2D, KindOf, MAX_PLAYER_COUNT, ObjectID, ObjectShroudStatus,
    Relationship,
};
use gamelogic::helpers::{TheAudio, TheGameLogic, TheScriptEngine, TheThingFactory};
use gamelogic::object::Object;
use gamelogic::object::production::construction::FoundationValidator;
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::object::special_power_template::get_special_power_store;
use gamelogic::object::update::special_power_update::SpecialPowerCommandOption;
use gamelogic::player::{Player, player_list};
use gamelogic::system::disguise_manager::get_disguise_manager;
use gamelogic::system::shroud_manager::{ShroudState, get_shroud_manager};

/// Re-export of the INI settings type from the Common crate's INI parser.
/// C++: InGameUI fieldParseTable settings (InGameUI.cpp:752-856, ini_in_game_ui.rs)
pub use game_engine::common::ini::ini_in_game_ui::{
    Coord2D as IniCoord2D, ICoord2D as IniICoord2D, InGameUISettings as InGameUIIniSettings,
    RGBAColorInt,
};

include!("types.rs");
include!("messages.rs");
include!("floating_text.rs");
include!("selection.rs");
include!("radius_cursor.rs");
include!("minimap.rs");
include!("world_anim.rs");
include!("impl_update.rs");
include!("impl_input.rs");
include!("snapshot.rs");
include!("leftover.rs");
include!("place_icons.rs");
include!("radar_map.rs");
include!("radius_cursor_ini.rs");
include!("select_all.rs");
mod superweapon_ready_flash;
pub use superweapon_ready_flash::*;
mod live_hud;
pub use live_hud::*;
include!("tests.rs");

/// Concatenated live sources for residual `include_str!` scans.
pub const INGAME_UI_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("types.rs"),
    include_str!("messages.rs"),
    include_str!("floating_text.rs"),
    include_str!("selection.rs"),
    include_str!("radius_cursor.rs"),
    include_str!("minimap.rs"),
    include_str!("world_anim.rs"),
    include_str!("impl_update.rs"),
    include_str!("impl_input.rs"),
    include_str!("snapshot.rs"),
    include_str!("leftover.rs"),
    include_str!("place_icons.rs"),
    include_str!("radar_map.rs"),
    include_str!("radius_cursor_ini.rs"),
    include_str!("select_all.rs"),
);
