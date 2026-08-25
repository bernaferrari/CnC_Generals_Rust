//! # GameClient Implementation
//!
//! This module contains the main GameClient struct and implementation,
//! converted from the original C++ GameClient class. The GameClient serves
//! as the primary interface for all client-side game operations.
//!
//! ## Key Features
//!
//! - Drawable registration and management
//! - Subsystem lifecycle management
//! - Message dispatch and filtering
//! - Game state synchronization
//! - Resource preloading and cleanup
//!
//! ## Usage
//!
//! ```rust,no_run
//! use game_client_rust::core::GameClient;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Initialize the global GameClient instance
//! let mut client = GameClient::new()?;
//! client.init()?;
//!
//! // Main game loop (simplified example)
//! for _frame in 0..10 {
//!     client.update()?;
//!     // Game logic would check for exit conditions here
//! }
//!
//! // Cleanup is automatic via Drop trait
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::io::Cursor;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use glam;

use crate::assets::{AssetConfig, AssetHandle, AssetManager, AssetPriority};
use crate::audio::GameAudio;
use crate::audio::{AudioEngine, AudioEventQueue, MusicSystem, SpeechSystem};
use crate::core::Region3D;
use crate::core::script_action_handler::{
    GameClientScriptActionHandler, apply_pending_script_display_state, get_script_fps_limit,
    get_script_visual_speed_multiplier, register_script_display_bridge,
    reset_script_action_runtime_state,
};
use crate::core::subsystems::{
    AudioSubsystem, DisplayStringManagerSubsystem, FontLibrarySubsystem,
    HeaderTemplateManagerSubsystem, HotKeyManagerSubsystem, InGameUISubsystem, InGameUiHandle,
    KeyboardHandle, MouseHandle, TerrainVisualStub, VideoPlayerSubsystem, WindowManagerSubsystem,
    create_keyboard, create_mouse, register_campaign_snapshot_block,
    register_game_client_snapshot_block, register_particle_system_snapshot_block,
    register_radar_snapshot_block, register_terrain_visual_snapshot_block,
};
use crate::display::DisplayInterface;
use crate::display::display::Display as GraphicsDisplay;
use crate::display::image::{get_mapped_image_collection, sync_mapped_images_from_common};
use crate::display::view::with_tactical_view_ref;
use crate::drawable::*;
use crate::effects::weather_complete::{get_weather_system_mut, initialize_weather_system};
use crate::effects::{DecalManager, EffectsConfig};
use crate::fx_list::{init_fx_list_store, register_decal_manager, register_fx_audio};
use crate::game_text::GameText;
use crate::gui::campaign_manager::get_campaign_manager;
use crate::gui::gadgets::register_button_audio_hook;
use crate::gui::ime_manager::get_ime_manager;
use crate::gui::load_screen::{
    clear_load_screen_presentation_pump, register_load_screen_presentation_pump,
};
use crate::gui::{
    UIRenderer, WindowStatus, get_shell, get_skirmish_setup, set_ui_renderer, with_window_manager,
};
use crate::helpers::{register_in_game_ui_backend, register_mouse_backend};
use crate::input::*;
use crate::message_stream::command_list::get_command_list;
use crate::message_stream::command_router::route_commands_to_gamelogic;
use crate::message_stream::game_message::GameMessageType;
use crate::message_stream::message_stream::THE_MESSAGE_STREAM;
use crate::message_stream::player_state::set_local_player_id;
use crate::message_stream::translators::{
    CommandTranslator as CommandTranslatorImpl, TranslatorFactory,
};
use crate::message_stream::{GameMessage, GameMessageDisposition, GameMessageTranslator};
use crate::network::{NetworkBridgeHandle, is_network_command_message};
use crate::platform::PlatformContext;
use crate::system::beacon_display;
use crate::system::{
    BeaconNotification, Coord3D, GameMessageResult, SubsystemInterface, TimeOfDay,
};
use crate::video_player::{
    VideoPlayerInterface as GlobalVideoPlayerInterface, get_video_player, init_video_player,
    shutdown_video_player,
};
use game_engine::System::{
    register_campaign_manager_runtime_hooks, register_drawable_id_counter_hooks,
    register_save_load_campaign_hooks, register_save_load_mission_hooks,
    register_save_load_skirmish_hooks,
};
use game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL;
use game_engine::common::game_lod::prefers_low_res_movies;
use game_engine::common::global_data as runtime_global_data;
use game_engine::common::ini::ini_game_data::TimeOfDay as IniTimeOfDay;
use game_engine::common::ini::{INI, INILoadType, get_global_data, get_global_language_read};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::recorder::{init_recorder, with_recorder_mut};
use game_engine::common::system::{
    Snapshotable, Xfer, XferMode as CommonXferMode, XferStatus as CommonXferStatus, XferVersion,
    geometry::Matrix3D,
};
use game_engine::common::thing::{ThingTemplate, get_thing_factory};
use game_engine::common::user_preferences::UserPreferences;

use game_engine::{
    Xfer as RuntimeXfer, XferMode as RuntimeXferMode, XferStatus as RuntimeXferStatus,
};
use nalgebra::Point3;

// GameLogic integration for object iteration
// Note: gamelogic is the crate name (from Cargo.toml)
use game_engine::common::frame_clock::FrameTiming;
use gamelogic::common::types::{INVALID_ID, ObjectID, Real};
use gamelogic::helpers::{
    TerrainTreeEvent, TerrainUnitMovedInfo, TheGameClient, TheGameLogic, TheScriptEngine,
    register_animation_metadata_hook, register_scorch_hook, register_terrain_tree_hook,
    register_terrain_unit_moved_hook,
};
use gamelogic::object::Object as GameLogicObject;
use gamelogic::object::draw::{
    TruckDrawLivePhysics, W3DDebrisDraw, W3DDebrisDrawModuleData, W3DLaserDraw,
    W3DLaserDrawModuleData, W3DModelDraw, W3DModelDrawModuleData, W3DOverlordAircraftDraw,
    W3DOverlordAircraftDrawModuleData, W3DOverlordTankDraw, W3DOverlordTankDrawModuleData,
    W3DOverlordTruckDraw, W3DOverlordTruckDrawModuleData, W3DPoliceCarDraw,
    W3DPoliceCarDrawModuleData, W3DScienceModelDraw, W3DScienceModelDrawModuleData, W3DTankDraw,
    W3DTankDrawModuleData, W3DTankTruckDraw, W3DTankTruckDrawModuleData, W3DTreeDraw,
    W3DTreeDrawModuleData, W3DTruckDraw, W3DTruckDrawModuleData, leftover_science_model_data,
    prune_live_host_police_car_light, prune_live_host_tread_debris, prune_live_host_truck_dust,
    tick_live_host_police_car_light, tick_live_host_science_model_hide,
    tick_live_host_tread_debris, tick_live_host_truck_dust,
};
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::object::update::{
    AnimatedParticleSysBoneClientUpdateModule, BeaconClientUpdateModule, SwayClientUpdateModule,
    leftover_template_uses_animated_particle_sys_bones,
    prune_live_host_animated_particle_sys_bones, tick_live_host_animated_particle_sys_bones,
};
use ww3d_core::w3d_io::{W3DChunk, W3DReader};

// Live `crate::core::game_client` module (C++ GameClient.cpp).
// `include!` keeps one logical module so field privacy and the public API
// stay identical to the former `core/game_client.rs` god-file.

include!("ids.rs");
include!("presentation_specialized_draw.rs");

include!("live_slot.rs");
include!("xfer_adapters.rs");
include!("errors.rs");
include!("animation.rs");
include!("shadows.rs");
include!("client_types.rs");
include!("subsystem.rs");
include!("message.rs");
include!("dispatcher.rs");
include!("impl_init.rs");
include!("impl_update.rs");
include!("impl_draw.rs");
include!("leftover.rs");
include!("tests.rs");

/// Concatenated live sources for residual `include_str!` scans.
pub const GAME_CLIENT_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("ids.rs"),
    include_str!("presentation_specialized_draw.rs"),
    include_str!("live_slot.rs"),
    include_str!("xfer_adapters.rs"),
    include_str!("errors.rs"),
    include_str!("animation.rs"),
    include_str!("shadows.rs"),
    include_str!("client_types.rs"),
    include_str!("subsystem.rs"),
    include_str!("message.rs"),
    include_str!("dispatcher.rs"),
    include_str!("impl_init.rs"),
    include_str!("impl_update.rs"),
    include_str!("impl_draw.rs"),
    include_str!("leftover.rs"),
);
