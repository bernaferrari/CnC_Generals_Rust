#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]

/*
** Command & Conquer Generals Zero Hour(tm) - Actual Game Engine
** Copyright 2025 Electronic Arts Inc.
**
** Real C&C game engine replacing the cube demo with full RTS gameplay
*/

use crate::assets::{get_asset_manager, W3DModel};
use crate::command_line::CommandLineArgs;
use crate::fow_rendering;
use crate::game_logic::script_events::{self, ScriptEvent};
use crate::game_logic::victory_conditions::AllianceState;
use crate::game_logic::*;
#[cfg(feature = "integration-diagnostics")]
use crate::integration_bridge::IntegrationTelemetryBridge;
use crate::localization;
use crate::platform::{create_platform_message_handler, WindowMessageProcessor};
use crate::runtime::attachments::AttachmentDispatcher;
use crate::save_load::{
    init_game_state_system, GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo,
};
use crate::subsystem_manager::{
    get_subsystem_manager, init_subsystem_manager, with_subsystem_mut, AudioManagerSubsystem,
    NetworkSubsystem, SubsystemInterface,
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
    register_command_list_init, register_game_client_factory, GameClientInterface,
};
use game_engine::common::system::subsystem_interface::{
    SubsystemError, SubsystemResult, SubsystemState,
};
use glam::{Mat4, Vec2, Vec3};
#[cfg(feature = "integration-diagnostics")]
use integration::diagnostics::SystemDiagnostics;
#[cfg(feature = "integration-diagnostics")]
use integration::IntegrationConfig;
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
    event::{ElementState, Event, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowAttributes},
};
use ww3d_core::ww3d::WW3D;
use ww3d_engine::{self, EngineConfig, EngineError, FrameTiming};
use ww3d_renderer_3d::core::error::Error as RendererError;
use crate::graphics::{
    graphics_system::MAX_STAGE_TEXTURES, render_pipeline::gameplay_to_render_transform,
    GraphicsSystem, RenderPipeline,
};

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

mod types;
mod runtime;
mod host;
mod dispatch;
#[path = "runtime_host/mod.rs"]
mod runtime_host;
mod shell;
mod boot;
mod input;
mod ui_commands;
mod camera_drain;
mod host_authority;
mod start_game;
mod hotkeys;
mod selection;
mod mouse;
mod audio;
mod run_loop;

pub use types::{CnCGameEngine, VertexXYZNDUV2};
#[cfg(feature = "internal")]
pub use types::parity_test_support;
pub use run_loop::run_cnc_game;
use types::*;
use runtime::{RuntimeHostBridge, RuntimeHostSnapshot};
use run_loop::{SoundType, resolve_ui_structure_template_name};

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

#[cfg(test)]
mod tests;
#[cfg(test)]
mod source_scan_tests;

/// Concatenated engine source for residual `include_str!` scans.
pub const ENGINE_SRC: &str = concat!(
    include_str!("types.rs"),
    include_str!("dispatch.rs"),
    include_str!("runtime_host.rs"),
    include_str!("shell.rs"),
    include_str!("boot.rs"),
    include_str!("input.rs"),
    include_str!("ui_commands.rs"),
    include_str!("camera_drain.rs"),
    include_str!("host_authority.rs"),
    include_str!("start_game.rs"),
    include_str!("hotkeys.rs"),
    include_str!("selection.rs"),
    include_str!("mouse.rs"),
    include_str!("audio.rs"),
    include_str!("run_loop.rs"),
    include_str!("runtime.rs"),
    include_str!("host.rs"),
);
