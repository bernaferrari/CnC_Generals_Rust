#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]

/*
** Command & Conquer Generals Zero Hour(tm) - Actual Game Engine
** Copyright 2025 Electronic Arts Inc.
**
** Real C&C game engine replacing the cube demo with full RTS gameplay
*/

use crate::assets::{W3DModel, get_asset_manager};
use crate::command_line::CommandLineArgs;
use crate::game_logic::victory_conditions::AllianceState;
use crate::game_logic::*;
use crate::graphics::{
    GraphicsSystem, RenderPipeline, graphics_system::MAX_STAGE_TEXTURES,
    render_pipeline::gameplay_to_render_transform,
};
#[cfg(feature = "integration-diagnostics")]
use crate::integration_bridge::IntegrationTelemetryBridge;
use crate::localization;
use crate::platform::{WindowMessageProcessor, create_platform_message_handler};
use crate::runtime::attachments::dispatch_attachments;
use crate::save_load::{
    GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo, init_game_state_system,
};
use crate::subsystem_manager::{AudioManagerSubsystem, NetworkSubsystem, SubsystemInterface};
use crate::ui::{
    DiagnosticsOverlayStats, GameHUD, GameUIState, MinimapActionKind, MinimapInteraction, Screen,
    UIEvent, UIManager,
};
use crate::util::profiler::InitTimer;
use ::game_engine::common::frame_clock::{FrameClock, FrameTiming as ClockFrameTiming};
use anyhow::Result;
pub use game_engine::common::game_engine::GameState;
use game_engine::common::game_engine::{
    GameClientInterface, register_command_list_init, register_game_client_factory,
};
use game_engine::common::system::subsystem_interface::{
    SubsystemError, SubsystemResult, SubsystemState,
};
use glam::{Mat4, Vec2, Vec3};
#[cfg(feature = "integration-diagnostics")]
use integration::IntegrationConfig;
#[cfg(feature = "integration-diagnostics")]
use integration::diagnostics::SystemDiagnostics;
use log::{debug, error, info, warn};
use rodio_compat::{OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::{HashMap, HashSet, VecDeque};
use std::f32::consts::{PI, TAU};
use std::fs;
use std::future::Future;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::AtomicU32;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use wgpu::util::DeviceExt;
use winit::{
    self,
    event::{DeviceEvent, ElementState, Event, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowAttributes},
};
use ww3d_core::ww3d::WW3D;
use ww3d_engine::{self, EngineConfig, EngineError, FrameTiming};
use ww3d_renderer_3d::core::error::Error as RendererError;

#[cfg(feature = "network")]
use game_network::time::NetworkClock;

#[cfg(not(feature = "network"))]
struct NetworkClock;

#[cfg(not(feature = "network"))]
impl NetworkClock {
    fn override_with_duration(_duration: Duration) {}
    fn clear_override() {}
}

mod audio;
mod audio_view;
mod boot;
mod camera_drain;
mod camera_filter;
mod camera_height;
mod camera_motion;
mod camera_view_effects;
mod clear_game_data;
mod command_phase;
#[cfg(feature = "game_client")]
mod control_bar_bridge;
mod control_bar_viewport;
mod dispatch;
mod display_clock;
mod eva_delivery;
mod host;
mod host_authority;
mod hotkeys;
pub(crate) mod ini_crc_boot;
mod input;
#[cfg(feature = "game_client")]
mod minimap_bridge;
mod mouse;
mod startup_definition_source;
mod startup_ini_content;
mod startup_worker_owner;
#[cfg(test)]
pub(crate) use mouse::live_view_height_clamp;
mod native_display;
mod native_view_effects;
#[cfg(feature = "game_client")]
mod native_visual_time;
pub(crate) mod object_ini_boot;
mod options_bridge;
#[cfg(feature = "game_client")]
mod presentation_materials;
mod presentation_terrain_cache;
#[cfg(feature = "game_client")]
mod quit_menu_bridge;
mod retirement_guard;
use retirement_guard::NativeBoundaryFailure;
mod run_loop;
mod runtime;
mod runtime_host;
mod selection;
mod selection_hud;
mod shell;
mod start_game;
mod types;
mod ui_commands;

pub use run_loop::run_cnc_game;
#[cfg(all(feature = "internal", not(target_arch = "wasm32")))]
pub fn run_snow_anim2d_owner_probe() -> anyhow::Result<()> {
    camera_drain::run_snow_anim2d_owner_probe()
}
#[cfg(feature = "internal")]
pub fn run_replay_fast_forward_engine_probe() -> anyhow::Result<()> {
    camera_drain::run_replay_fast_forward_engine_probe()
}
use presentation_terrain_cache::PresentationTerrainCache;
use run_loop::{SoundType, resolve_ui_structure_template_name};
use runtime::{RuntimeHostBridge, RuntimeHostSnapshot};
#[cfg(feature = "internal")]
pub use types::parity_test_support;
use types::*;
pub use types::{CnCGameEngine, VertexXYZNDUV2};

enum StartupLoadMessage {
    Progress { progress: f32, phase: String },
    Prepared(std::result::Result<StartupPreparedLoadResult, String>),
}

enum StartupLoadState {
    Idle,
    InProgress {
        _worker_owner: startup_worker_owner::StartupWorkerOwner,
        receiver: Receiver<StartupLoadMessage>,
        started_at: Instant,
        last_worker_progress: f32,
        last_worker_phase: Option<String>,
        last_worker_logged_bucket: u8,
    },
    Complete,
}

#[cfg(test)]
mod architecture_honesty_tests;
#[cfg(test)]
mod camera_startup_tests;
#[cfg(test)]
mod runtime_host_option_tests;
#[cfg(test)]
mod tests;

/// Concatenated engine source for residual `include_str!` scans.
mod camera_continuation;
pub(crate) use camera_continuation::NativeCameraContinuation;

mod replay_lifecycle;

#[cfg(feature = "game_client")]
mod replay_window;

#[cfg(all(feature = "internal", not(target_arch = "wasm32")))]
mod replay_probe_paths;
#[cfg(all(feature = "internal", not(target_arch = "wasm32")))]
pub use replay_probe_paths::prepare_replay_probe_private_paths;

mod map_native_admission;
mod native_audio_effects;

#[cfg(all(
    feature = "internal",
    feature = "game_client",
    not(target_arch = "wasm32")
))]
pub fn run_terminal_load_owner_probe() -> anyhow::Result<()> {
    host_authority::terminal_load_owner_probe::run()
}
