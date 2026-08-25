//! Meta event translator for key and mouse remapping.

use std::collections::HashSet;
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{OnceLock, RwLock};
use std::time::Instant;

use game_engine::common::audio::game_audio::{
    AudioAffect, get_global_audio_manager, initialize_global_audio_manager,
};
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
use game_engine::common::ini::{
    DynamicGameLODLevel, INI, INIError, INILoadType, INIResult, TimeOfDay, get_global_data,
    register_block_parser,
};
use game_engine::common::rts::science::{SCIENCE_INVALID, get_science_store};
use log::debug;

use super::game_message::{
    Coord3D, GameMessage, GameMessageArgumentType, GameMessageType, ICoord2D, IRegion2D,
    build_region,
};
use super::message_stream::{GameMessageDisposition, GameMessageTranslator, emit_message};
use crate::core::script_action_handler::{
    get_script_display_debug_callback, script_set_3d_wireframe_mode,
    set_script_display_debug_callback, stop_script_display_movie, toggle_script_display_letter_box,
    toggle_script_display_movie_capture,
};
use crate::display::display::DebugDisplayCallback;
use crate::display::view::{FilterMode, FilterType, with_tactical_view};
use crate::drawable::drawable_manager::with_drawable_manager_ref;
use crate::gui::shell::get_shell;
use crate::gui::window_video_manager::with_window_video_manager;
use crate::gui::with_shell_ref;
use crate::helpers::{TheControlBar, TheInGameUI};
use crate::message_stream::player_state::{get_local_player_id, set_local_player_id};
use crate::message_stream::selection_xlat::DRAG_TOLERANCE;
use crate::presentation_translator_residual::{
    translator_entry_has_kind, translator_entry_is_local, with_translator_catalog,
};
use crate::system::DebugDisplay;
use gamelogic::commands::command::CommandType;
use gamelogic::commands::get_selection_manager;
use gamelogic::common::ModelConditionFlags;
use gamelogic::common::audio::TimeOfDay as LogicTimeOfDay;
use gamelogic::common::types::{GeometryExtentModType, GeometryInfo, KindOf};
use gamelogic::helpers::{
    TheAudio, TheGameClient, TheGameLogic, TheThingFactory, TheVictoryConditions,
};
use gamelogic::object::drawable::Drawable;
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::player::{PLAYER_INDEX_INVALID, PlayerType, ThePlayerList};
use gamelogic::scripting::engine::get_script_engine;

// Live `meta_event` module via `#[path = "meta_event_impl/mod.rs"]`.
// `include!` keeps one logical module so field privacy and the public API
// stay identical to the former dump.

include!("residual.rs");
include!("types.rs");
include!("state.rs");
include!("command_map.rs");
include!("helpers.rs");
include!("player.rs");
include!("parse.rs");
include!("dispatch.rs");
include!("translator.rs");
include!("tests.rs");

/// Concatenated live sources for residual `include_str!` scans.
pub const META_EVENT_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("residual.rs"),
    include_str!("types.rs"),
    include_str!("state.rs"),
    include_str!("command_map.rs"),
    include_str!("helpers.rs"),
    include_str!("player.rs"),
    include_str!("parse.rs"),
    include_str!("dispatch.rs"),
    include_str!("translator.rs"),
);
