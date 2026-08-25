//! C++ parity wrapper for `LoadScreen.cpp`.

pub use super::loading_screen::*;

use crate::display::image::get_mapped_image_collection;
use crate::game_text::GameText;
use crate::input::with_mouse;
use crate::map_util::{find_draw_positions, get_map_cache_manager, get_map_preview_image};

use super::campaign_manager::{
    MAX_DISPLAYED_UNITS, MAX_OBJECTIVE_LINES, Mission, get_campaign_manager,
};
use super::challenge_generals::{
    ChallengeGenerals, GeneralPersona, get_challenge_generals, get_challenge_generals_mut,
    init_challenge_generals,
};
use super::game_window::{
    GPM_SET_PROGRESS, GameWindow, Image as WindowImage, WindowMessage, WindowMsgData,
};
use super::window_video_manager::{WindowVideoPlayType, with_window_video_manager};
use super::{WindowManager, WindowStatus, with_window_manager};
use game_engine::common::ini::ini_map_cache::MapMetaData;
use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
use game_engine::common::rts::player_template::{PlayerTemplate, get_player_template_store};
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::TheAudio;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// Live `load_screen` module. `include!` keeps one logical module so field
// privacy and the public API stay identical to the former dump.

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

/// Concatenated live sources for residual `include_str!` scans.
pub const LOAD_SCREEN_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("types.rs"),
    include_str!("api.rs"),
    include_str!("init_windows.rs"),
    include_str!("single_player.rs"),
    include_str!("challenge.rs"),
    include_str!("movies_audio.rs"),
    include_str!("multiplayer.rs"),
    include_str!("map_transfer.rs"),
    include_str!("helpers.rs"),
);
