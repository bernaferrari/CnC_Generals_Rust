#![allow(missing_docs)]
#![allow(dead_code)]
#![cfg_attr(test, cfg(feature = "internal"))]
// Game Engine - Rust port from C++
// Command & Conquer Generals Zero Hour Game Engine

pub mod System;
pub mod common;
pub mod game_network;
pub mod memory;

pub mod custom_match_preferences;
pub mod errors;
pub mod skirmish_preferences;
pub mod terrain;

pub mod audio_affect;
pub mod audio_event_info;
pub mod audio_handle_special_values;
pub mod audio_random_value;
pub mod audio_settings;
pub mod battle_honors;
pub mod bit_flags_io;
pub mod border_colors;
pub mod client_update_module;
pub mod game_spy_misc_preferences;
pub mod gamespy_misc_preferences;
pub mod ignore_preferences;
pub mod ini_exception;
pub mod ladder_preferences;
pub mod latch_restore;
pub mod map_object;
pub mod map_reader_writer_info;
pub mod misc_audio;
pub mod model_state;
pub mod overridable;
pub mod r#override;
pub mod perf_metrics;
pub mod quickmatch_preferences;
pub mod scoped_mutex;
pub mod special_power_mask_type;
pub mod special_power_type;
pub mod stl_typedefs;
pub mod thing_sort;
pub mod unit_timings;
pub mod xfer_deep_crc;
pub use common::*;

// Re-export System components for convenience
pub use System::{
    get_game_state,
    init_game_state,
    AvailableGameInfo,
    // Save/Load system
    GameState,
    GameStateMap,
    SaveCode,
    SaveFileType,
    SaveGameInfo,
    SaveLoadLayoutType,
    Snapshot,
    SnapshotType,
    // Xfer system
    Xfer,
    XferLoad,
    XferMode,
    XferSave,
    XferStatus,
};
