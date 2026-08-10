//! C++ parity wrapper for `LoadScreen.cpp`.

pub use super::loading_screen::*;

use crate::display::image::get_mapped_image_collection;
use crate::game_text::GameText;
use crate::input::with_mouse;
use crate::map_util::{find_draw_positions, get_map_cache_manager, get_map_preview_image};

use super::campaign_manager::{
    get_campaign_manager, Mission, MAX_DISPLAYED_UNITS, MAX_OBJECTIVE_LINES,
};
use super::challenge_generals::{
    get_challenge_generals, get_challenge_generals_mut, init_challenge_generals, ChallengeGenerals,
    GeneralPersona,
};
use super::game_window::{
    GameWindow, Image as WindowImage, WindowMessage, WindowMsgData, GPM_SET_PROGRESS,
};
use super::window_video_manager::{with_window_video_manager, WindowVideoPlayType};
use super::{with_window_manager, WindowManager, WindowStatus};
use game_engine::common::ini::ini_map_cache::MapMetaData;
use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
use game_engine::common::rts::player_template::{get_player_template_store, PlayerTemplate};
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::TheAudio;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// Dump+path split: `gui/load_screen.rs` is the unused original dump.
// This directory is the live `load_screen` module. `include!` keeps one logical
// module so field privacy and the public API stay identical to the dump.

include!("types.rs");
include!("api.rs");
include!("init_windows.rs");
include!("single_player.rs");
include!("challenge.rs");
include!("movies_audio.rs");
include!("multiplayer.rs");
include!("map_transfer.rs");
include!("helpers.rs");
include!("tests.rs");
