#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]

/*
** Command & Conquer Generals Zero Hour(tm) - Actual Game Engine
** Copyright 2025 Electronic Arts Inc.
**
** Real C&C game engine replacing the cube demo with full RTS gameplay
*/

use crate::assets::{W3DModel, get_asset_manager};
use crate::command_line::CommandLineArgs;
use crate::fow_rendering;
use crate::game_logic::script_events::{self, ScriptEvent};
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
use crate::runtime::attachments::AttachmentDispatcher;
use crate::save_load::{
    GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo, init_game_state_system,
};
use crate::subsystem_manager::{
    AudioManagerSubsystem, NetworkSubsystem, SubsystemInterface, get_subsystem_manager,
    init_subsystem_manager, with_subsystem_mut,
};
use crate::ui::{
    DiagnosticsOverlayStats, GameHUD, GameUIState, MinimapActionKind, MinimapInteraction, Screen,
    UIEvent, UIManager, UISystemState,
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
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
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

#[cfg(feature = "game_client")]
thread_local! {
    static LOADING_PROGRESS: std::cell::Cell<f32> = const { std::cell::Cell::new(0.0) };
    static LOADING_PHASE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

mod audio;
mod boot;
mod camera_drain;
#[cfg(feature = "game_client")]
mod control_bar_bridge;
mod dispatch;
mod host;
mod host_authority;
mod hotkeys;
pub(crate) mod ini_crc_boot;
mod input;
#[cfg(feature = "game_client")]
mod minimap_bridge;
mod mouse;
pub(crate) mod object_ini_boot;
mod options_bridge;
mod presentation_terrain_cache;
#[cfg(feature = "game_client")]
mod quit_menu_bridge;
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
    Complete(std::result::Result<StartupLoadResult, String>),
}

enum StartupLoadState {
    Idle,
    InProgress {
        receiver: Receiver<StartupLoadMessage>,
        started_at: Instant,
        last_worker_progress: f32,
        last_worker_phase: Option<String>,
        last_worker_logged_bucket: u8,
    },
    Complete,
}

/// Generation token so an abandoned boot worker cannot clear NewGame or
/// keep mutating INI/weapon stores after the host owns the session.
static STARTUP_WORKER_GENERATION: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

fn startup_worker_generation() -> u64 {
    STARTUP_WORKER_GENERATION.load(std::sync::atomic::Ordering::SeqCst)
}

fn bump_startup_worker_generation() {
    STARTUP_WORKER_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

fn startup_worker_owns(generation: u64) -> bool {
    startup_worker_generation() == generation
}

#[cfg(test)]
mod architecture_honesty_tests;
#[cfg(test)]
mod source_scan_tests;
#[cfg(test)]
mod tests;

/// Concatenated engine source for residual `include_str!` scans.
pub const ENGINE_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("audio.rs"),
    include_str!("boot.rs"),
    include_str!("camera_drain.rs"),
    include_str!("dispatch.rs"),
    include_str!("host.rs"),
    include_str!("host_authority.rs"),
    include_str!("hotkeys.rs"),
    include_str!("ini_crc_boot.rs"),
    include_str!("input.rs"),
    include_str!("mouse.rs"),
    include_str!("mouse/selection_input.rs"),
    include_str!("mouse/camera.rs"),
    include_str!("mouse/ui_dispatch.rs"),
    include_str!("mouse/world_pick.rs"),
    include_str!("minimap_bridge.rs"),
    include_str!("object_ini_boot.rs"),
    include_str!("options_bridge.rs"),
    include_str!("presentation_terrain_cache.rs"),
    include_str!("quit_menu_bridge.rs"),
    include_str!("run_loop.rs"),
    include_str!("runtime.rs"),
    include_str!("selection.rs"),
    include_str!("shell.rs"),
    include_str!("start_game.rs"),
    include_str!("types.rs"),
    include_str!("ui_commands.rs"),
    include_str!("runtime_host/campaign_menus.rs"),
    include_str!("runtime_host/dialogs.rs"),
    include_str!("runtime_host/gameplay.rs"),
    include_str!("runtime_host/gameplay_orders.rs"),
    include_str!("runtime_host/gameplay_select.rs"),
    include_str!("runtime_host/hud_bar.rs"),
    include_str!("runtime_host/live_commands.rs"),
    include_str!("runtime_host/live_gates_a.rs"),
    include_str!("runtime_host/live_gates_b.rs"),
    include_str!("runtime_host/live_gates_c.rs"),
    include_str!("runtime_host/live_presentation.rs"),
    include_str!("runtime_host/live_probes.rs"),
    include_str!("runtime_host/mod.rs"),
    include_str!("runtime_host/overlay_clicks.rs"),
    include_str!("runtime_host/shell_core.rs"),
    include_str!("runtime_host/skirmish.rs"),
);
