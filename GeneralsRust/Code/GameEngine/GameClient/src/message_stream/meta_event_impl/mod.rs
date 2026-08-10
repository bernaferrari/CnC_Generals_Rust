//! Meta event translator for key and mouse remapping.

use std::collections::HashSet;
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{OnceLock, RwLock};
use std::time::Instant;

use game_engine::common::audio::game_audio::{
    get_global_audio_manager, initialize_global_audio_manager, AudioAffect,
};
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
use game_engine::common::ini::{
    get_global_data, register_block_parser, DynamicGameLODLevel, INIError, INILoadType, INIResult,
    TimeOfDay, INI,
};
use game_engine::common::rts::science::{get_science_store, SCIENCE_INVALID};
use log::debug;

use super::game_message::{
    build_region, Coord3D, GameMessage, GameMessageArgumentType, GameMessageType, ICoord2D,
    IRegion2D,
};
use super::message_stream::{emit_message, GameMessageDisposition, GameMessageTranslator};
use crate::core::script_action_handler::{
    get_script_display_debug_callback, script_set_3d_wireframe_mode,
    set_script_display_debug_callback, stop_script_display_movie, toggle_script_display_letter_box,
    toggle_script_display_movie_capture,
};
use crate::display::display::DebugDisplayCallback;
use crate::display::view::{with_tactical_view, FilterMode, FilterType};
use crate::drawable::drawable_manager::with_drawable_manager_ref;
use crate::gui::shell::get_shell;
use crate::gui::try_with_shell_mut;
use crate::gui::window_video_manager::with_window_video_manager;
use crate::helpers::{TheControlBar, TheInGameUI};
use crate::message_stream::player_state::{get_local_player_id, set_local_player_id};
use crate::message_stream::selection_xlat::DRAG_TOLERANCE;
use crate::presentation_translator_residual::{
    translator_entry_has_kind, translator_entry_is_local, with_translator_catalog,
};
use crate::system::DebugDisplay;
use gamelogic::commands::command::CommandType;
use gamelogic::commands::get_selection_manager;
use gamelogic::common::audio::TimeOfDay as LogicTimeOfDay;
use gamelogic::common::types::{GeometryExtentModType, GeometryInfo, KindOf};
use gamelogic::common::ModelConditionFlags;
use gamelogic::helpers::{
    TheAudio, TheGameClient, TheGameLogic, TheThingFactory, TheVictoryConditions,
};
use gamelogic::object::drawable::Drawable;
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::player::{PlayerType, ThePlayerList, PLAYER_INDEX_INVALID};
use gamelogic::scripting::engine::get_script_engine;

// Dump+path split: `message_stream/meta_event.rs` is the unused original dump.
// This directory is the live `meta_event` module. `include!` keeps one logical
// module so field privacy and the public API stay identical to the dump.

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
