#![allow(unused_imports, unused_variables, dead_code)]

/*
** Command & Conquer Generals Zero Hour(tm) - Actual Game Engine
** Copyright 2025 Electronic Arts Inc.
**
** Real C&C game engine replacing the cube demo with full RTS gameplay
*/

use crate::assets::{get_asset_manager, W3DModel};
use crate::command_line::CommandLineArgs;
use crate::config::GlobalData;
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
    get_subsystem_manager, with_subsystem_mut, GlobalDataSubsystem, NetworkSubsystem,
};
use crate::ui::{
    DiagnosticsOverlayStats, GameHUD, GameUIState, MinimapActionKind, MinimapInteraction, Screen,
    UIEvent, UIManager, UISystemEvent, UISystemState, WgpuUISystem,
};
use crate::util::profiler::InitTimer;
use ::game_engine::common::frame_clock::{FrameClock, FrameTiming as ClockFrameTiming};
use anyhow::Result;
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
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{Child, Command, Stdio};
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

#[cfg(feature = "network")]
use game_network::time::NetworkClock;

#[cfg(not(feature = "network"))]
struct NetworkClock;

#[cfg(not(feature = "network"))]
impl NetworkClock {
    fn override_with_duration(_duration: Duration) {}
    fn clear_override() {}
}

#[cfg(test)]
mod tests {
    use super::{CnCGameEngine, GameState};

    #[test]
    fn startup_deferred_budget_menu_is_zero_without_startup_frame() {
        let budget = CnCGameEngine::startup_deferred_model_load_budget(GameState::Menu, None, 0);
        assert_eq!(budget, 0);
    }

    #[test]
    fn startup_deferred_budget_menu_stays_zero_across_startup_age() {
        let early =
            CnCGameEngine::startup_deferred_model_load_budget(GameState::Menu, Some(100), 120);
        let mid =
            CnCGameEngine::startup_deferred_model_load_budget(GameState::Menu, Some(100), 170);
        let late =
            CnCGameEngine::startup_deferred_model_load_budget(GameState::Menu, Some(100), 400);
        assert_eq!(early, 0);
        assert_eq!(mid, 0);
        assert_eq!(late, 0);
    }

    #[test]
    fn startup_deferred_budget_disables_startup_prewarm_for_ui_responsiveness() {
        let loading_early =
            CnCGameEngine::startup_deferred_model_load_budget(GameState::Loading, Some(100), 110);
        let menu_early =
            CnCGameEngine::startup_deferred_model_load_budget(GameState::Menu, Some(100), 110);
        assert_eq!(loading_early, 0);
        assert_eq!(menu_early, 0);
    }

    #[test]
    fn startup_deferred_budget_disabled_during_playing() {
        let budget =
            CnCGameEngine::startup_deferred_model_load_budget(GameState::Playing, Some(0), 1000);
        assert_eq!(budget, 0);
    }

    #[test]
    fn menu_caustic_warmup_waits_for_stable_menu_frames() {
        assert!(!CnCGameEngine::should_trigger_menu_caustic_warmup(None, 200));
        assert!(!CnCGameEngine::should_trigger_menu_caustic_warmup(Some(100), 219));
        assert!(CnCGameEngine::should_trigger_menu_caustic_warmup(Some(100), 220));
    }

    #[test]
    fn effective_fps_limit_prefers_script_override() {
        let limit =
            CnCGameEngine::effective_fps_limit_for_frame(Some(45), false, 30, 2.0, true, true);
        assert_eq!(limit, Some(45));
    }

    #[test]
    fn effective_fps_limit_honors_cpp_tivo_replay_rule_for_global_limit() {
        let limit = CnCGameEngine::effective_fps_limit_for_frame(None, true, 30, 1.0, true, true);
        assert_eq!(limit, None);
    }

    #[test]
    fn effective_fps_limit_disables_global_limit_for_fast_visual_multiplier() {
        let limit = CnCGameEngine::effective_fps_limit_for_frame(None, true, 30, 1.5, false, false);
        assert_eq!(limit, None);
    }

    #[test]
    fn game_logic_gate_without_network_matches_cpp_pause_behavior() {
        assert!(CnCGameEngine::should_update_game_logic_frame(false, None));
        assert!(!CnCGameEngine::should_update_game_logic_frame(true, None));
    }

    #[test]
    fn game_logic_gate_with_network_uses_frame_ready_only() {
        assert!(CnCGameEngine::should_update_game_logic_frame(
            false,
            Some(true)
        ));
        assert!(CnCGameEngine::should_update_game_logic_frame(
            true,
            Some(true)
        ));
        assert!(!CnCGameEngine::should_update_game_logic_frame(
            false,
            Some(false)
        ));
        assert!(!CnCGameEngine::should_update_game_logic_frame(
            true,
            Some(false)
        ));
    }
}

const DEFAULT_SKIRMISH_MAP: &str = "Defcon6";
const DEFAULT_VIEW_FOV_RADIANS: f32 = 50.0_f32.to_radians();
const DEFAULT_VIEW_NEAR_CLIP: f32 = 1.0;
const SHELL_LOADING_LAYOUT: &str = "Menus/ShellGameLoadScreen.wnd";
const SHELL_LOADING_PARENT_NAME: &str = "ShellGameLoadScreen.wnd:ParentShellGameLoadScreen";
const SHELL_LOADING_PROGRESS_NAME: &str = "ShellGameLoadScreen.wnd:ProgressLoad";

fn pack_ui_mouse_data(x: i32, y: i32) -> u32 {
    ((y as u32) << 16) | ((x as u32) & 0xFFFF)
}
const DEFAULT_VIEW_FAR_CLIP: f32 = 20_000.0;

// C++ SAGE Engine equivalent modules
use crate::graphics::{
    graphics_system::MAX_STAGE_TEXTURES, render_pipeline::gameplay_to_render_transform,
    GraphicsSystem, RenderPipeline,
};

/// Game state - matches C++ GameEngine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    /// Initial state - showing main menu
    Menu,
    /// Loading game assets
    Loading,
    /// Active gameplay
    Playing,
    /// Game paused
    Paused,
    /// Shutting down
    Exiting,
}

#[derive(Debug, Clone)]
struct ScriptCameraShaker {
    epicenter: Vec3,
    radius: f32,
    duration_seconds: f32,
    elapsed_seconds: f32,
    amplitude_degrees: f32,
    phase: f32,
    frequency_hz: f32,
}

impl ScriptCameraShaker {
    fn new(epicenter: Vec3, radius: f32, duration_seconds: f32, amplitude_degrees: f32) -> Self {
        // Deterministic phase/frequency seed from shaker parameters.
        let seed = (epicenter.x * 0.013
            + epicenter.y * 0.021
            + epicenter.z * 0.034
            + amplitude_degrees * 0.055)
            .sin();
        let normalized = ((seed * 43_758.547).fract()).abs();
        Self {
            epicenter,
            radius: radius.max(0.01),
            duration_seconds: duration_seconds.max(0.01),
            elapsed_seconds: 0.0,
            amplitude_degrees,
            phase: normalized * TAU,
            frequency_hz: 2.0 + normalized * 4.0,
        }
    }
}

struct StartupLoadResult {
    game_logic: GameLogic,
    loaded_map_name: Option<String>,
    start_in_menu: bool,
    map_requested_from_cli: bool,
}

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

/// Main C&C game engine with full RTS functionality - restructured to match C++ SAGE architecture
pub struct CnCGameEngine {
    window: Arc<Window>,
    #[allow(dead_code)]
    command_line: Arc<CommandLineArgs>,

    // C++ SAGE equivalent rendering subsystems
    graphics_system: GraphicsSystem,
    render_pipeline: RenderPipeline,

    // Platform message handling
    message_processor: WindowMessageProcessor,

    // Audio system
    #[allow(dead_code)]
    audio_output: Option<OutputStream>,
    audio_handle: Option<OutputStreamHandle>,
    background_music: Option<Sink>,
    sound_effects: Vec<Sink>,
    ui_sound_cache: HashMap<String, Arc<[u8]>>,

    // Game state machine - matches C++ GameEngine m_quitting and state management
    current_state: GameState,
    pending_state: Option<GameState>,
    startup_load_state: StartupLoadState,
    startup_target_state: Option<GameState>,
    startup_start_in_menu: bool,
    last_loading_title_update: Option<Instant>,
    startup_last_reported_progress: f32,
    startup_last_progress_change_at: Instant,
    startup_last_stall_warning_at: Option<Instant>,
    last_caustic_warmup_attempt: Option<Instant>,

    // Game state
    game_logic: GameLogic,
    combat_system: CombatSystem,
    pathfinding_system: PathfindingSystem,
    resource_manager: ResourceManager,
    save_file_manager: SaveFileManager,

    // Camera system
    camera_position: Vec3,
    camera_target: Vec3,
    camera_zoom: f32,
    camera_zoom_target: Option<f32>,
    camera_zoom_start: f32,
    camera_zoom_duration: f32,
    camera_zoom_elapsed: f32,
    camera_zoom_ease_in: f32,
    camera_zoom_ease_out: f32,
    camera_orbit_distance: f32,
    camera_pitch_radians: f32,
    camera_pitch_target: Option<f32>,
    camera_pitch_start: f32,
    camera_pitch_duration: f32,
    camera_pitch_elapsed: f32,
    camera_pitch_ease_in: f32,
    camera_pitch_ease_out: f32,
    camera_yaw_radians: f32,
    camera_yaw_target: Option<f32>,
    camera_yaw_start: f32,
    camera_yaw_duration: f32,
    camera_yaw_elapsed: f32,
    camera_yaw_ease_in: f32,
    camera_yaw_ease_out: f32,
    camera_shake_offset: Vec3,
    screen_shake_intensity: f32,
    screen_shake_angle_cos: f32,
    screen_shake_angle_sin: f32,
    script_camera_shakers: Vec<ScriptCameraShaker>,
    script_fps_limit: Option<u32>,
    script_fps_limit_last_tick: Option<Instant>,
    camera_slave_mode: Option<CameraSlaveModeRequest>,
    view_matrix: Mat4,
    projection_matrix: Mat4,

    // Input state
    keys_pressed: HashSet<Key>,
    mouse_position: (f32, f32),
    mouse_world_position: Vec3,
    is_dragging: bool,
    selection_start: Option<Vec3>,

    // Game state
    selected_objects: Vec<ObjectId>,
    control_groups: HashMap<u8, Vec<ObjectId>>,
    current_player_id: u32,
    game_paused: bool,

    // UI state
    show_debug_info: bool,
    show_health_bars: bool,
    frame_counter: u32,
    fps: f32,
    last_frame_timing: Option<FrameTiming>,
    frame_clock: FrameClock,
    menu_loading_tick_accumulator: Duration,
    menu_loading_last_tick: Instant,
    diagnostics_overlay: Option<DiagnosticsOverlayStats>,

    // UI system
    ui_manager: UIManager,
    wgpu_ui_system: WgpuUISystem,
    game_hud: GameHUD,
    active_menu_shell_hook: Option<&'static str>,
    gpui_menu_bridge: Option<GpuiMenuBridge>,

    // Model loading state
    models_loaded: bool,
    pending_shell_model_prewarm: VecDeque<String>,
    menu_enter_frame: Option<u64>,
    shell_ui_enqueued_frame: Option<u64>,
    last_menu_stall_warning: Option<Instant>,
    match_over: bool,
    victory_summary: Option<VictorySummary>,
}

/// C++ SAGE engine VertexFormatXYZNDUV2 equivalent - matches original vertex declarations
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexXYZNDUV2 {
    pub position: [f32; 3],    // XYZ - Position coordinates
    pub normal: [f32; 3],      // N - Normal vector
    pub diffuse: u32,          // D - Diffuse color (RGBA packed as u32, like D3D8)
    pub tex_coords0: [f32; 2], // UV - Primary texture coordinates
    pub tex_coords1: [f32; 2], // UV2 - Secondary texture coordinates for multi-stage texturing
}

impl VertexXYZNDUV2 {
    /// C++ SAGE VertexFormatXYZNDUV2 buffer layout - matches D3DVERTEXELEMENT9 declarations
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<VertexXYZNDUV2>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position (XYZ)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Normal (N)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Diffuse color (D) - packed RGBA like D3D8
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Unorm8x4,
                },
                // Primary texture coordinates (UV)
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2 + std::mem::size_of::<u32>())
                        as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // Secondary texture coordinates (UV2) for multi-texturing
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2
                        + std::mem::size_of::<u32>()
                        + std::mem::size_of::<[f32; 2]>())
                        as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// C++ SAGE engine equivalent uniforms - matches GlobalUniforms structure
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SAGEUniforms {
    view_projection: [[f32; 4]; 4],
    view_matrix: [[f32; 4]; 4],
    projection_matrix: [[f32; 4]; 4],
    camera_position: [f32; 4],
    time: f32,
    ambient_light: [f32; 3],
    sun_direction: [f32; 3],
    sun_color: [f32; 3],
    _padding: f32,
}

/// C++ SAGE VertexMaterialClass equivalent - matches original material properties
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialProperties {
    diffuse_color: [f32; 4],   // Base color reflected by lighting
    specular_color: [f32; 4],  // Sharp reflective highlights
    emissive_color: [f32; 4],  // Self-illumination color
    opacity: f32,              // Transparency (1.0 = opaque, 0.0 = transparent)
    shininess: f32,            // Specular power
    stage0_uv_scale: [f32; 2], // UV scaling for stage 0
    stage1_uv_scale: [f32; 2], // UV scaling for stage 1
}

#[derive(Debug, Clone, Copy)]
struct StartupCameraDefaults {
    pitch_degrees: f32,
    yaw_degrees: f32,
    camera_height: f32,
    max_camera_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GpuiMenuAction {
    StartSkirmish,
    ExitGame,
    OpenOptions,
}

impl GpuiMenuAction {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "start_skirmish" => Some(Self::StartSkirmish),
            "exit_game" => Some(Self::ExitGame),
            "open_options" => Some(Self::OpenOptions),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct GpuiMenuWindowPlacement {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug)]
struct GpuiMenuBridge {
    child: Child,
    ipc_path: PathBuf,
    launch_label: &'static str,
}

impl GpuiMenuBridge {
    fn spawn(placement: Option<GpuiMenuWindowPlacement>) -> Result<Self> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let ipc_path = std::env::temp_dir().join(format!(
            "generals_gpui_menu_action_{}_{}.txt",
            std::process::id(),
            now
        ));
        let _ = fs::remove_file(&ipc_path);

        if let Ok(bridge) = Self::spawn_from_binary(&ipc_path, placement) {
            return Ok(bridge);
        }

        Self::spawn_from_cargo(&ipc_path, placement)
    }

    fn spawn_from_binary(
        ipc_path: &Path,
        placement: Option<GpuiMenuWindowPlacement>,
    ) -> Result<Self> {
        let helper_name = if cfg!(target_os = "windows") {
            "gpui-gui.exe"
        } else {
            "gpui-gui"
        };
        let current_exe = std::env::current_exe()
            .map_err(|err| anyhow::anyhow!("failed to resolve current executable path: {err}"))?;
        let helper = current_exe.with_file_name(helper_name);
        if !helper.is_file() {
            return Err(anyhow::anyhow!("gpui helper binary not found at {:?}", helper));
        }

        let mut command = Command::new(&helper);
        command
            .arg("--runtime-menu-ipc")
            .arg(ipc_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(placement) = placement {
            command
                .env("GENERALS_GPUI_MENU_X", placement.x.to_string())
                .env("GENERALS_GPUI_MENU_Y", placement.y.to_string())
                .env("GENERALS_GPUI_MENU_WIDTH", placement.width.to_string())
                .env("GENERALS_GPUI_MENU_HEIGHT", placement.height.to_string());
        }
        let child = command
            .spawn()
            .map_err(|err| anyhow::anyhow!("failed to spawn gpui helper binary: {err}"))?;

        Ok(Self {
            child,
            ipc_path: ipc_path.to_path_buf(),
            launch_label: "binary",
        })
    }

    fn spawn_from_cargo(
        ipc_path: &Path,
        placement: Option<GpuiMenuWindowPlacement>,
    ) -> Result<Self> {
        let workspace_manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
        let mut command = Command::new("cargo");
        command
            .arg("run")
            .arg("--manifest-path")
            .arg(workspace_manifest)
            .arg("-p")
            .arg("gpui-gui")
            .arg("--")
            .arg("--runtime-menu-ipc")
            .arg(ipc_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(placement) = placement {
            command
                .env("GENERALS_GPUI_MENU_X", placement.x.to_string())
                .env("GENERALS_GPUI_MENU_Y", placement.y.to_string())
                .env("GENERALS_GPUI_MENU_WIDTH", placement.width.to_string())
                .env("GENERALS_GPUI_MENU_HEIGHT", placement.height.to_string());
        }
        let child = command
            .spawn()
            .map_err(|err| anyhow::anyhow!("failed to spawn gpui helper via cargo run: {err}"))?;

        Ok(Self {
            child,
            ipc_path: ipc_path.to_path_buf(),
            launch_label: "cargo",
        })
    }

    fn poll_action(&mut self) -> Option<GpuiMenuAction> {
        let raw = fs::read_to_string(&self.ipc_path).ok()?;
        let _ = fs::remove_file(&self.ipc_path);
        GpuiMenuAction::parse(&raw)
    }

    fn has_exited(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_some()
    }

    fn terminate(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_file(&self.ipc_path);
    }
}

impl Drop for GpuiMenuBridge {
    fn drop(&mut self) {
        self.terminate();
    }
}

impl CnCGameEngine {
    const MENU_CAUSTIC_WARMUP_DELAY_FRAMES: u64 = 120;
    const CAUSTIC_WARMUP_RETRY_INTERVAL: Duration = Duration::from_secs(10);

    fn menu_uses_gpui_bridge(&self) -> bool {
        false
    }

    fn current_menu_window_placement(&self) -> Option<GpuiMenuWindowPlacement> {
        let size = self.window.inner_size();
        let position = self.window.outer_position().ok()?;
        Some(GpuiMenuWindowPlacement {
            x: position.x,
            y: position.y,
            width: size.width.max(1),
            height: size.height.max(1),
        })
    }

    fn ensure_gpui_menu_bridge(&mut self) {
        // Runtime menu is rendered in the main game window.
    }

    fn shutdown_gpui_menu_bridge(&mut self) {
        if let Some(mut bridge) = self.gpui_menu_bridge.take() {
            bridge.terminate();
        }
    }

    fn poll_gpui_menu_actions(&mut self) -> bool {
        false
    }

    fn shell_ui_active(&self) -> bool {
        false
    }

    fn shell_ui_owns_menu(&self) -> bool {
        self.current_state == GameState::Menu && self.shell_ui_active()
    }

    fn loading_visual_phase(elapsed_seconds: f32) -> (&'static str, f32) {
        if elapsed_seconds < 1.0 {
            ("Initializing engine", (elapsed_seconds / 1.0) * 0.15)
        } else if elapsed_seconds < 4.0 {
            (
                "Loading map data",
                0.15 + ((elapsed_seconds - 1.0) / 3.0) * 0.30,
            )
        } else if elapsed_seconds < 10.0 {
            (
                "Spawning world objects",
                0.45 + ((elapsed_seconds - 4.0) / 6.0) * 0.35,
            )
        } else {
            (
                "Finalizing startup",
                0.80 + ((elapsed_seconds - 10.0) / 6.0).clamp(0.0, 1.0) * 0.15,
            )
        }
    }

    fn ui_window_manager_has_windows(&self) -> bool {
        false
    }

    fn gameplay_ui_active(&self) -> bool {
        false
    }

    fn ensure_shell_loading_overlay(&mut self) {
        let _ = (SHELL_LOADING_LAYOUT, SHELL_LOADING_PARENT_NAME, SHELL_LOADING_PROGRESS_NAME);
    }

    fn hide_shell_loading_overlay(&mut self) {
    }

    fn update_shell_loading_progress(&mut self, progress: f32, phase: Option<&str>) {
        self.wgpu_ui_system.set_loading_progress(progress, phase);
    }

    fn observe_startup_progress(&mut self, progress: f32, phase: &str) {
        let progress = progress.clamp(0.0, 1.0);
        if progress > self.startup_last_reported_progress + 0.001 {
            self.startup_last_reported_progress = progress;
            self.startup_last_progress_change_at = Instant::now();
            self.startup_last_stall_warning_at = None;
            return;
        }

        let stalled_for = self.startup_last_progress_change_at.elapsed();
        if stalled_for < Duration::from_secs(2) {
            return;
        }

        let should_warn = self
            .startup_last_stall_warning_at
            .map(|last| last.elapsed() >= Duration::from_secs(2))
            .unwrap_or(true);
        if !should_warn {
            return;
        }

        warn!(
            "Startup progress stalled at {:.0}% in phase '{}' for {:.2}s (game_state={:?})",
            progress * 100.0,
            phase,
            stalled_for.as_secs_f32(),
            self.current_state
        );
        self.startup_last_stall_warning_at = Some(Instant::now());
    }

    fn hide_gameplay_layouts(&mut self) {}

    fn ensure_gameplay_layouts(&mut self) {}

    fn to_engine_timing(clock: ClockFrameTiming, frame_start: Instant) -> FrameTiming {
        let sync_time = clock.total_time.as_millis() as u32;
        let previous_sync_time = sync_time.saturating_sub(clock.delta_time.as_millis() as u32);
        FrameTiming {
            frame_number: clock.frame_number,
            delta_time: clock.delta_time,
            total_time: clock.total_time,
            fps: if clock.delta_time.as_secs_f32() > 0.0 {
                1.0 / clock.delta_time.as_secs_f32()
            } else {
                0.0
            },
            frame_start,
            sync_time,
            previous_sync_time,
        }
    }

    fn configured_startup_shell_map() -> Option<String> {
        if let Some(shell_map) =
            crate::subsystem_manager::with_subsystem::<GlobalDataSubsystem, _>(|subsystem| {
                subsystem.get_global_data().and_then(|global| {
                    if global.shell_map_on && !global.shell_map_name.trim().is_empty() {
                        Some(global.shell_map_name.clone())
                    } else {
                        None
                    }
                })
            })
            .flatten()
        {
            return Some(shell_map);
        }

        let mut global = GlobalData::new();
        let _ = global.load_ini("Data/INI/Default/GameData.ini");
        let _ = global.load_ini("Data/INI/GameData.ini");
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            let _ = global.load_ini("Data/INI/GameDataDebug.ini");
        }

        if global.shell_map_on && !global.shell_map_name.trim().is_empty() {
            Some(global.shell_map_name)
        } else {
            None
        }
    }

    fn current_startup_logic_frame(&self) -> u64 {
        // Use engine frame cadence for startup budgeting. Game-logic frame counters can jump
        // during long blocking startup operations, which over-ages menu startup budgets.
        self.frame_counter as u64
    }

    fn shell_start_frame(&self) -> Option<u64> {
        // Anchor startup age to the frame where menu state became active when available.
        // Shell enqueue can happen earlier during loading and should not age out menu
        // startup budgets before first visible menu frames.
        self.menu_enter_frame.or(self.shell_ui_enqueued_frame)
    }

    fn startup_deferred_model_load_budget(
        current_state: GameState,
        startup_frame: Option<u64>,
        current_logic_frame: u64,
    ) -> usize {
        if !matches!(current_state, GameState::Menu | GameState::Loading) {
            return 0;
        }

        // Keep loading/menu frames strictly responsive and avoid startup hitches that can
        // trigger OS "not responding" behavior or hide/show shell UI frames.
        let _ = startup_frame;
        let _ = current_logic_frame;
        0
    }

    fn should_trigger_menu_caustic_warmup(
        startup_frame: Option<u64>,
        current_logic_frame: u64,
    ) -> bool {
        startup_frame
            .map(|start| {
                current_logic_frame.saturating_sub(start) >= Self::MENU_CAUSTIC_WARMUP_DELAY_FRAMES
            })
            .unwrap_or(false)
    }

    fn maybe_trigger_deferred_caustic_warmup(&mut self) {
        let should_start = match self.current_state {
            GameState::Playing => true,
            GameState::Menu => Self::should_trigger_menu_caustic_warmup(
                self.shell_start_frame(),
                self.current_startup_logic_frame(),
            ),
            _ => false,
        };
        if !should_start {
            return;
        }

        if self
            .last_caustic_warmup_attempt
            .is_some_and(|last| last.elapsed() < Self::CAUSTIC_WARMUP_RETRY_INTERVAL)
        {
            return;
        }

        self.last_caustic_warmup_attempt = Some(Instant::now());
        let queued = crate::assets::manager::warmup_caustic_textures_async(
            self.graphics_system.device_arc(),
            self.graphics_system.queue_arc(),
        );
        if queued {
            info!(
                "Queued deferred caustic texture warmup (state={:?})",
                self.current_state
            );
        }
    }

    #[cfg(feature = "game_client")]
    fn should_skip_world_scene_for_shell_menu(&self) -> bool {
        if self.current_state == GameState::Loading {
            // C++ load screens are full-screen UI overlays; they do not render live terrain/world.
            return true;
        }
        if self.current_state != GameState::Menu || !self.shell_ui_owns_menu() {
            return false;
        }
        // Keep shell menu UI isolated from world rendering to avoid startup race/stall behavior.
        true
    }

    #[cfg(not(feature = "game_client"))]
    fn should_skip_world_scene_for_shell_menu(&self) -> bool {
        false
    }

    fn configured_startup_camera_defaults() -> StartupCameraDefaults {
        if let Some(defaults) =
            crate::subsystem_manager::with_subsystem::<GlobalDataSubsystem, _>(|subsystem| {
                subsystem
                    .get_global_data()
                    .map(|global| StartupCameraDefaults {
                        pitch_degrees: global.camera_pitch,
                        yaw_degrees: global.camera_yaw,
                        camera_height: global.camera_height,
                        max_camera_height: global.max_camera_height,
                    })
            })
            .flatten()
        {
            return defaults;
        }

        let mut global = GlobalData::new();
        let _ = global.load_ini("Data/INI/Default/GameData.ini");
        let _ = global.load_ini("Data/INI/GameData.ini");
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            let _ = global.load_ini("Data/INI/GameDataDebug.ini");
        }

        StartupCameraDefaults {
            pitch_degrees: global.camera_pitch,
            yaw_degrees: global.camera_yaw,
            camera_height: global.camera_height,
            max_camera_height: global.max_camera_height,
        }
    }

    fn bootstrap_camera_for_loaded_map(
        game_logic: &GameLogic,
        current_player_id: u32,
        defaults: StartupCameraDefaults,
    ) -> (Vec3, Vec3, f32) {
        const DEFAULT_VIEW_WIDTH: f32 = 640.0;
        const DEFAULT_VIEW_HEIGHT: f32 = 480.0;
        let (world_min, world_max) = game_logic.world_bounds();
        let world_center = Vec3::new(
            (world_min.x + world_max.x) * 0.5,
            (world_min.y + world_max.y) * 0.5,
            (world_min.z + world_max.z) * 0.5,
        );

        let metadata_initial_camera = game_logic
            .last_parsed_map_settings()
            .and_then(|meta| meta.initial_camera_position);
        let metadata_target = metadata_initial_camera.map(|pos| Vec2::new(pos.x, pos.y));

        let team_target = game_logic
            .get_player(current_player_id)
            .map(|player| player.team)
            .and_then(|team| game_logic.team_base_position(team));

        let focus_2d = metadata_target
            .or(team_target.map(|pos| Vec2::new(pos.x, pos.z)))
            .unwrap_or(Vec2::new(world_center.x, world_center.z));

        // Match C++ W3DView::lookAt(): unlike the old 2D View::lookAt(), the W3D path writes the
        // requested world coordinate directly into m_pos and builds the camera transform from that.
        let terrain_target = Vec3::new(focus_2d.x, 0.0, focus_2d.y);
        let (camera_anchor_ground_height, terrain_height_max) =
            Self::sample_startup_camera_heights(game_logic, terrain_target, world_center.y);
        let focus_target = Vec3::new(focus_2d.x, 0.0, focus_2d.y);
        let (focus_ground_height, _) =
            Self::sample_startup_camera_heights(game_logic, focus_target, world_center.y);

        // Keep the C++ zoom/offset sampling from the top-left anchor, but aim the modern
        // Rust camera at the requested scene focus. This remains the closest visible match for the
        // current renderer bridge.
        let camera_target = Vec3::new(focus_2d.x, focus_ground_height, focus_2d.y);
        let camera_offset_z = camera_anchor_ground_height + defaults.camera_height.max(0.0);
        let pitch_radians = defaults.pitch_degrees.to_radians();
        let yaw_radians = defaults.yaw_degrees.to_radians();
        let camera_offset_y = if pitch_radians.tan().abs() > f32::EPSILON {
            -(camera_offset_z / pitch_radians.tan())
        } else {
            0.0
        };
        let camera_offset_x = -(camera_offset_y * yaw_radians.tan());

        // Match W3DView::setZoomToDefault exactly: desired zoom is the visible terrain max
        // around the look-at point plus max camera height, divided by the base offset height.
        let zoom = Self::compute_default_camera_zoom_from_heights(
            camera_anchor_ground_height,
            terrain_height_max,
            defaults,
            1.0,
        );

        // Match W3DView::buildCameraTransform when angle/pitch defaults are zero:
        // source = cameraOffset * zoom; source *= (1 - ground / source.z); then translate.
        let source_z = camera_offset_z * zoom;
        let factor = if source_z.abs() > f32::EPSILON {
            1.0 - (camera_anchor_ground_height / source_z)
        } else {
            1.0
        };
        let source = Vec3::new(
            camera_offset_x * zoom * factor,
            camera_offset_z * zoom * factor,
            camera_offset_y * zoom * factor,
        );
        let camera_position = camera_target + source;

        info!(
            "Startup camera bootstrap: raw_initial={:?} requested_focus_2d={:?} target={:?} position={:?} ground_height={:.2} terrain_height_max={:.2} camera_offset=({:.2}, {:.2}, {:.2}) pitch_deg={:.2} yaw_deg={:.2} zoom={:.2} factor={:.3}",
            metadata_initial_camera,
            focus_2d,
            camera_target,
            camera_position,
            camera_anchor_ground_height,
            terrain_height_max,
            camera_offset_x,
            camera_offset_y,
            camera_offset_z,
            defaults.pitch_degrees,
            defaults.yaw_degrees,
            zoom,
            factor,
        );

        (camera_target, camera_position, zoom)
    }

    fn sample_startup_camera_heights(
        game_logic: &GameLogic,
        terrain_target: Vec3,
        fallback_ground_height: f32,
    ) -> (f32, f32) {
        const MAX_GROUND_LEVEL: f32 = 120.0;
        const TERRAIN_SAMPLE_SIZE: f32 = 40.0;
        let (world_min, world_max) = game_logic.world_bounds();

        let mut ground_height = game_logic
            .terrain_height_at(terrain_target)
            .unwrap_or(fallback_ground_height);
        if ground_height > MAX_GROUND_LEVEL {
            ground_height = MAX_GROUND_LEVEL;
        }

        let sample_positions = [
            terrain_target,
            terrain_target + Vec3::new(TERRAIN_SAMPLE_SIZE, 0.0, -TERRAIN_SAMPLE_SIZE),
            terrain_target + Vec3::new(-TERRAIN_SAMPLE_SIZE, 0.0, -TERRAIN_SAMPLE_SIZE),
            terrain_target + Vec3::new(TERRAIN_SAMPLE_SIZE, 0.0, TERRAIN_SAMPLE_SIZE),
            terrain_target + Vec3::new(-TERRAIN_SAMPLE_SIZE, 0.0, TERRAIN_SAMPLE_SIZE),
        ];
        let terrain_height_max = sample_positions
            .into_iter()
            .filter_map(|sample| {
                let clamped = Vec3::new(
                    sample.x.clamp(world_min.x, world_max.x),
                    sample.y,
                    sample.z.clamp(world_min.z, world_max.z),
                );
                game_logic.terrain_height_at(clamped)
            })
            .fold(ground_height, f32::max);

        (ground_height, terrain_height_max)
    }

    fn compute_default_camera_zoom_from_heights(
        ground_height: f32,
        terrain_height_max: f32,
        defaults: StartupCameraDefaults,
        max_height_scale: f32,
    ) -> f32 {
        let camera_offset_z = ground_height + defaults.camera_height.max(0.0);
        // Match C++ W3DView::setDefaultView()/setZoomToDefault():
        // maxHeight is a scale on GlobalData.maxCameraHeight, and angle does not participate.
        let desired_height =
            terrain_height_max + (defaults.max_camera_height * max_height_scale.max(0.0)).max(0.0);
        if camera_offset_z.abs() > f32::EPSILON {
            desired_height / camera_offset_z
        } else {
            1.0
        }
    }

    fn compute_default_camera_zoom_for_target(&self, target: Vec3, max_height_scale: f32) -> f32 {
        let defaults = Self::configured_startup_camera_defaults();
        let (ground_height, terrain_height_max) =
            Self::sample_startup_camera_heights(&self.game_logic, target, target.y);
        Self::compute_default_camera_zoom_from_heights(
            ground_height,
            terrain_height_max,
            defaults,
            max_height_scale,
        )
    }

    fn write_startup_debug_state(&self) {
        let _ = self;
    }

    fn emit_startup_load_progress(
        sender: &mpsc::Sender<StartupLoadMessage>,
        progress: f32,
        phase: &str,
    ) {
        let _ = sender.send(StartupLoadMessage::Progress {
            progress: progress.clamp(0.0, 0.96),
            phase: phase.to_string(),
        });
    }

    fn spawn_startup_map_load(
        start_in_menu: bool,
        map_to_load: String,
        map_requested_from_cli: bool,
        player_name: Option<String>,
        graphics_device: Arc<wgpu::Device>,
        graphics_queue: Arc<wgpu::Queue>,
    ) -> StartupLoadState {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            Self::emit_startup_load_progress(&sender, 0.03, "Initializing asset manager");
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || -> std::result::Result<StartupLoadResult, String> {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|err| {
                            format!("failed to create startup tokio runtime for asset init: {err}")
                        })?;
                    runtime
                        .block_on(crate::assets::manager::init_asset_manager(
                            graphics_device.as_ref(),
                            graphics_queue.as_ref(),
                        ))
                        .map_err(|err| format!("asset manager init failed: {err}"))?;

                    Self::emit_startup_load_progress(&sender, 0.14, "Asset manager ready");
                    Self::emit_startup_load_progress(&sender, 0.18, "Creating game session");
                    let mut game_logic = GameLogic::initialize();
                    game_logic.start_new_game(if start_in_menu {
                        GameMode::Shell
                    } else {
                        GameMode::Skirmish
                    });
                    Self::emit_startup_load_progress(&sender, 0.22, "Priming object templates");
                    let thing_factory_needs_init = game_engine::common::thing::get_thing_factory()
                        .ok()
                        .map(|guard| guard.is_none())
                        .unwrap_or(false);
                    if thing_factory_needs_init {
                        if let Err(err) = game_engine::common::thing::init_thing_factory() {
                            warn!(
                                "ThingFactory prewarm failed before map spawn; continuing with lazy init: {}",
                                err
                            );
                        }
                    }

                    Self::emit_startup_load_progress(&sender, 0.24, "Loading map data");
                    let mut loaded_map_name = None;
                    let map_loaded =
                        game_logic.load_map_with_progress(&map_to_load, |progress, phase| {
                            Self::emit_startup_load_progress(&sender, progress, phase);
                        });
                    if !map_loaded {
                        warn!(
                            "Failed to load map '{}', falling back to default map '{}'",
                            map_to_load, DEFAULT_SKIRMISH_MAP
                        );
                        Self::emit_startup_load_progress(
                            &sender,
                            0.45,
                            "Retrying with default map",
                        );
                        if map_to_load != DEFAULT_SKIRMISH_MAP
                            && game_logic.load_map_with_progress(
                                DEFAULT_SKIRMISH_MAP,
                                |progress, phase| {
                                    Self::emit_startup_load_progress(&sender, progress, phase);
                                },
                            )
                        {
                            loaded_map_name = Some(DEFAULT_SKIRMISH_MAP.to_string());
                        }
                    } else {
                        loaded_map_name = Some(map_to_load.clone());
                    }

                    if let Some(player_name) = player_name.as_deref() {
                        if game_logic.set_player_name(0, player_name) {
                            info!("Set local player name to '{}'", player_name);
                        } else {
                            warn!("Failed to apply player name '{}'", player_name);
                        }
                    }
                    Self::emit_startup_load_progress(&sender, 0.92, "Finalizing startup data");

                    Ok(StartupLoadResult {
                        game_logic,
                        loaded_map_name,
                        start_in_menu,
                        map_requested_from_cli,
                    })
                },
            ))
            .map_err(|panic_payload| {
                if let Some(message) = panic_payload.downcast_ref::<&str>() {
                    format!("startup map load panicked: {message}")
                } else if let Some(message) = panic_payload.downcast_ref::<String>() {
                    format!("startup map load panicked: {message}")
                } else {
                    "startup map load panicked with non-string payload".to_string()
                }
            })
            .and_then(|inner| inner);

            let _ = sender.send(StartupLoadMessage::Complete(result));
        });

        StartupLoadState::InProgress {
            receiver,
            started_at: Instant::now(),
            last_worker_progress: 0.0,
            last_worker_phase: None,
            last_worker_logged_bucket: 0,
        }
    }

    fn finalize_startup_map_load(&mut self, result: StartupLoadResult) -> Result<()> {
        self.update_shell_loading_progress(0.97, Some("Finalizing startup"));
        self.game_logic = result.game_logic;

        if let Some(active_map_name) = result.loaded_map_name {
            if result.map_requested_from_cli {
                info!("Loaded map from command line: {}", active_map_name);
            } else if result.start_in_menu {
                info!("Loaded startup shell map: {}", active_map_name);
            }

            Self::apply_heightmap_hint(&mut self.render_pipeline, &self.game_logic);
            Self::apply_skybox_hint(&mut self.render_pipeline, &self.game_logic);
            // Keep loading->menu transition non-blocking: shell startup should paint menu first.
            // Heightmap hint remains queued in RenderPipeline and can load lazily after startup.
            if !result.start_in_menu {
                if let Err(err) = self.render_pipeline.load_heightmap_from_hint(
                    &self.graphics_system.device_arc(),
                    &self.graphics_system.queue_arc(),
                    Some(self.game_logic.world_bounds()),
                ) {
                    warn!(
                        "Failed to preload startup heightmap hint for '{}': {}",
                        active_map_name, err
                    );
                }
            }
            Self::reinitialize_minimap_renderer(
                &mut self.render_pipeline,
                &self.graphics_system,
                &mut self.game_logic,
            )?;
            Self::apply_map_lighting(
                &mut self.graphics_system,
                &mut self.render_pipeline,
                &self.game_logic,
            );
            let startup_camera_defaults = Self::configured_startup_camera_defaults();
            (self.camera_target, self.camera_position, self.camera_zoom) =
                Self::bootstrap_camera_for_loaded_map(
                    &self.game_logic,
                    self.current_player_id,
                    startup_camera_defaults,
                );
            self.sync_orbit_from_camera_transform();
        }

        if result.start_in_menu {
            self.ui_manager.transition_to_screen(Screen::MainMenu);
            self.wgpu_ui_system.set_state(UISystemState::MainMenu);
        }

        if let Some(target_state) = self.startup_target_state.take() {
            // Apply the post-load state transition immediately so we do not render additional
            // loading/world-only frames after shell/menu resources are already initialized.
            self.transition_to_state(target_state);
        }
        self.startup_load_state = StartupLoadState::Complete;
        self.last_loading_title_update = None;
        self.update_shell_loading_progress(1.0, Some("Startup complete"));
        self.startup_last_reported_progress = 1.0;
        self.startup_last_progress_change_at = Instant::now();
        self.startup_last_stall_warning_at = None;
        self.hide_shell_loading_overlay();
        self.window
            .set_title("Command & Conquer Generals Zero Hour");
        self.window.request_redraw();
        Ok(())
    }

    fn update_startup_loading(&mut self) -> Result<()> {
        let mut result: Option<std::result::Result<StartupLoadResult, String>> = None;
        let mut visual_phase = None::<String>;
        let mut visual_progress = None::<f32>;
        match &mut self.startup_load_state {
            StartupLoadState::Idle | StartupLoadState::Complete => return Ok(()),
            StartupLoadState::InProgress {
                receiver,
                started_at,
                last_worker_progress,
                last_worker_phase,
                last_worker_logged_bucket,
            } => {
                loop {
                    match receiver.try_recv() {
                        Ok(StartupLoadMessage::Progress { progress, phase }) => {
                            let clamped = progress.clamp(0.0, 0.96);
                            if clamped > *last_worker_progress {
                                *last_worker_progress = clamped;
                            }
                            if last_worker_phase.as_deref() != Some(phase.as_str()) {
                                info!(
                                    "Startup worker phase: {} ({:.0}%)",
                                    phase,
                                    (*last_worker_progress) * 100.0
                                );
                            }
                            let bucket = ((*last_worker_progress * 100.0).floor() as i32)
                                .div_euclid(5)
                                .clamp(0, 20) as u8;
                            if bucket > *last_worker_logged_bucket {
                                info!(
                                    "Startup worker progress: {:.0}% ({})",
                                    (*last_worker_progress) * 100.0,
                                    phase
                                );
                                *last_worker_logged_bucket = bucket;
                            }
                            *last_worker_phase = Some(phase);
                        }
                        Ok(StartupLoadMessage::Complete(complete)) => {
                            info!(
                                "Startup shell/game load completed in {:.2}s",
                                started_at.elapsed().as_secs_f32()
                            );
                            result = Some(complete);
                            break;
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            return Err(anyhow::anyhow!("startup load worker disconnected"));
                        }
                    }
                }

                if result.is_none() {
                    let elapsed = started_at.elapsed().as_secs_f32();
                    let (fallback_phase, fallback_progress) = Self::loading_visual_phase(elapsed);
                    let chosen_progress = (*last_worker_progress).max(fallback_progress);
                    let chosen_phase = last_worker_phase
                        .as_deref()
                        .unwrap_or(fallback_phase)
                        .to_string();
                    visual_phase = Some(chosen_phase);
                    visual_progress = Some(chosen_progress);
                }
            }
        }

        if let (Some(phase), Some(progress)) = (visual_phase, visual_progress) {
            self.update_shell_loading_progress(progress, Some(&phase));
            self.observe_startup_progress(progress, &phase);
            if self
                .last_loading_title_update
                // Avoid hammering native window-title updates during startup; on macOS these
                // updates can become expensive when issued every frame.
                .map(|last| last.elapsed() >= Duration::from_millis(350))
                .unwrap_or(true)
            {
                self.window.set_title(&format!(
                    "Command & Conquer Generals Zero Hour - Loading {phase} ({:.0}%)",
                    progress * 100.0
                ));
                self.last_loading_title_update = Some(Instant::now());
            }
            self.window.request_redraw();
            return Ok(());
        }

        match result.expect("startup completion result missing") {
            Ok(load_result) => self.finalize_startup_map_load(load_result),
            Err(err) => Err(anyhow::anyhow!(err)),
        }
    }

    pub async fn new(window: Arc<Window>, command_line: Arc<CommandLineArgs>) -> Result<Self> {
        let total_timer = InitTimer::new("🎮 Engine initialization");
        info!("🎮 Initializing Command & Conquer Generals Zero Hour Game Engine");
        info!("📋 Starting subsystem initialization sequence...");

        let debug_overlay = command_line.wants_debug_overlay();
        if command_line.no_audio {
            info!("🔇 Audio disabled via -noaudio");
        }
        if command_line.quick_start {
            info!("⚡ QuickStart enabled: skipping intro sequences (handled by SAGE runtime).");
        }

        Self::apply_command_line_overrides(&command_line);

        init_game_state_system()
            .map_err(|err| anyhow::anyhow!("Game state manager init failed: {err}"))?;

        // Initialize subsystems first (matches C++ GameEngine initialization order)
        // Subsystem manager is initialized globally, just verify it's available
        match get_subsystem_manager() {
            Some(handle) => {
                let manager = handle.lock();
                if manager.is_initialized() {
                    info!("✅ Core subsystems initialized");
                } else {
                    info!("ℹ️ Subsystem manager available but not initialized");
                }
            }
            None => {
                info!("ℹ️ Subsystem manager not initialized, continuing without subsystems");
            }
        }

        let size = window.inner_size();

        // Initialize WW3D engine to own the swapchain/device
        let mut engine_config = EngineConfig::default();
        engine_config.width = size.width.max(1);
        engine_config.height = size.height.max(1);

        if let Err(err) = ww3d_engine::init_with_window(window.clone(), engine_config).await {
            if !matches!(err, EngineError::AlreadyInitialised) {
                return Err(anyhow::anyhow!("Failed to initialize WW3D engine: {err:?}"));
            }
        }

        // Initialize C++ SAGE equivalent graphics system
        info!("🎨 Initializing GraphicsSystem (C++ SAGE equivalent)...");
        let graphics_timer = InitTimer::new("✅ GraphicsSystem initialized");
        let device =
            ww3d_engine::device().map_err(|e| anyhow::anyhow!("WW3D device unavailable: {e:?}"))?;
        let queue =
            ww3d_engine::queue().map_err(|e| anyhow::anyhow!("WW3D queue unavailable: {e:?}"))?;
        let color_format = ww3d_engine::color_format()
            .map_err(|e| anyhow::anyhow!("WW3D color format unavailable: {e:?}"))?;
        let depth_format = ww3d_engine::depth_format()
            .map_err(|e| anyhow::anyhow!("WW3D depth format unavailable: {e:?}"))?;
        let mut graphics_system = GraphicsSystem::new(device, queue, color_format, depth_format)?;
        graphics_timer.finish();

        // Initialize render pipeline
        info!("🔧 Initializing RenderPipeline (C++ SAGE equivalent)...");
        let pipeline_timer = InitTimer::new("✅ RenderPipeline initialized");
        let mut render_pipeline = RenderPipeline::initialize(&graphics_system)?;
        pipeline_timer.finish();

        // Keep event-loop bootstrap responsive: defer heavy asset-manager setup to the startup
        // loading worker, which reports incremental progress milestones.
        info!("🎨 Deferring C&C Asset Manager initialization to startup loading worker...");
        let asset_duration = Duration::ZERO;

        // Model preloading will be done after graphics system is ready
        // This is handled in the run loop after engine creation
        // Models are preloaded later; keep placeholder timer for consistency if needed.

        // No direct wgpu initialization needed - graphics system handles this

        // Initialize platform-specific message handling
        let message_handler = create_platform_message_handler();
        let mut message_processor = WindowMessageProcessor::new(message_handler);
        message_processor.attach_window(window.clone());

        // Initialize audio system unless disabled
        let (audio_output, audio_handle) = if command_line.no_audio {
            (None, None)
        } else {
            match OutputStream::try_default() {
                Ok((output, handle)) => (Some(output), Some(handle)),
                Err(e) => {
                    warn!("Failed to initialize audio output: {e}; continuing without audio");
                    (None, None)
                }
            }
        };

        let mut ui_sound_cache: HashMap<String, Arc<[u8]>> = HashMap::new();
        if audio_handle.is_some() {
            if let Some(manager_arc) = crate::assets::manager::get_asset_manager() {
                let mut manager = manager_arc.lock().unwrap();
                for &path in &[
                    crate::ui::sound_files::BUTTON_CLICK,
                    crate::ui::sound_files::BUTTON_HOVER,
                ] {
                    match manager.extract_file(path).await {
                        Ok(data) => {
                            ui_sound_cache
                                .insert(path.to_string(), Arc::from(data.into_boxed_slice()));
                        }
                        Err(err) => {
                            debug!("UI sound '{}' unavailable: {}", path, err);
                        }
                    }
                }
            }
        }

        // Initialize game systems
        let mut game_logic = GameLogic::initialize();
        let combat_system = CombatSystem::new();
        let (world_min, world_max) = game_logic.world_bounds();
        let world_width = (world_max.x - world_min.x).abs().max(1.0);
        let world_height = (world_max.z - world_min.z).abs().max(1.0);
        let pathfinding_system =
            PathfindingSystem::new_with_origin(world_min, world_width, world_height);
        let resource_manager = ResourceManager::new();
        let mut save_file_manager = SaveFileManager::new();
        save_file_manager
            .init()
            .map_err(|err| anyhow::anyhow!("Save file manager init failed: {err}"))?;

        // Initialize minimap renderer now that we know the world bounds.
        let world_bounds = game_logic.world_bounds();
        render_pipeline.initialize_minimap_renderer(
            graphics_system.device_arc(),
            graphics_system.queue_arc(),
            world_bounds,
        )?;

        let mut camera_target = Vec3::ZERO;
        let mut camera_position = Vec3::new(0.0, 310.0, -403.99988);
        let mut camera_zoom = 1.0;
        let projection_matrix = Mat4::perspective_rh(
            DEFAULT_VIEW_FOV_RADIANS,
            size.width as f32 / size.height as f32,
            DEFAULT_VIEW_NEAR_CLIP,
            DEFAULT_VIEW_FAR_CLIP,
        );

        // TEMPORARY: Create fallback cube for debugging objects without W3D models
        let (fallback_cube_vertex_buffer, fallback_cube_index_buffer, fallback_cube_index_count) =
            Self::create_fallback_cube(graphics_system.device());

        let start_in_menu = !command_line.quick_start && command_line.map_name.is_none();
        let startup_shell_map = start_in_menu
            .then(Self::configured_startup_shell_map)
            .flatten();
        let cli_map = command_line.map_name.as_deref();
        let map_to_load = cli_map
            .map(str::to_string)
            .or(startup_shell_map)
            .unwrap_or_else(|| DEFAULT_SKIRMISH_MAP.to_string());
        let startup_load_state = Self::spawn_startup_map_load(
            start_in_menu,
            map_to_load,
            cli_map.is_some(),
            command_line.player_name.clone(),
            graphics_system.device_arc(),
            graphics_system.queue_arc(),
        );

        let camera_offset = camera_position - camera_target;
        let camera_orbit_distance = camera_offset.length().max(1.0);
        let camera_pitch_radians = camera_offset
            .y
            .atan2(Vec2::new(camera_offset.x, camera_offset.z).length());
        let camera_yaw_radians = camera_offset.x.atan2(camera_offset.z);
        let view_matrix = Mat4::look_at_rh(camera_position, camera_target, Vec3::Y);

        let pending_shell_model_prewarm = if start_in_menu {
            // C++ shell startup does not run this extra Rust-only synchronous prewarm loop.
            // Keep shell-scene warmup disabled here and rely on the render pipeline's
            // incremental non-blocking budget instead so the menu can paint first.
            VecDeque::new()
        } else {
            info!("Skipping blocking startup model preload for gameplay startup");
            VecDeque::new()
        };

        let mut ui_manager = UIManager::new(size.width, size.height);
        if command_line.quick_start {
            ui_manager.enable_quick_start();
        }
        ui_manager
            .initialize()
            .map_err(|err| anyhow::anyhow!("failed to initialize startup UI: {err}"))?;
        ui_manager.transition_to_screen(Screen::Loading);
        let mut wgpu_ui_system = WgpuUISystem::new(window.as_ref())
            .await
            .map_err(|err| anyhow::anyhow!("failed to initialize runtime UI backend: {err}"))?;
        wgpu_ui_system.set_state(UISystemState::Loading);
        let initial_state = GameState::Loading;
        let pending_state = None;

        let mut engine = Self {
            window: window.clone(),
            command_line,

            // C++ SAGE equivalent rendering subsystems
            graphics_system,
            render_pipeline,

            message_processor,
            audio_output,
            audio_handle,
            background_music: None,
            sound_effects: Vec::new(),
            ui_sound_cache,

            // Default boot flow should land in the menu unless explicitly quick-starting.
            current_state: initial_state,
            pending_state,
            startup_load_state,
            startup_target_state: Some(if start_in_menu {
                GameState::Menu
            } else {
                GameState::Playing
            }),
            startup_start_in_menu: start_in_menu,
            last_loading_title_update: None,
            startup_last_reported_progress: 0.0,
            startup_last_progress_change_at: Instant::now(),
            startup_last_stall_warning_at: None,
            last_caustic_warmup_attempt: None,

            game_logic,
            combat_system,
            pathfinding_system,
            resource_manager,
            save_file_manager,
            camera_position,
            camera_target,
            camera_zoom,
            camera_zoom_target: None,
            camera_zoom_start: camera_zoom,
            camera_zoom_duration: 0.0,
            camera_zoom_elapsed: 0.0,
            camera_zoom_ease_in: 0.0,
            camera_zoom_ease_out: 0.0,
            camera_orbit_distance,
            camera_pitch_radians,
            camera_pitch_target: None,
            camera_pitch_start: camera_pitch_radians,
            camera_pitch_duration: 0.0,
            camera_pitch_elapsed: 0.0,
            camera_pitch_ease_in: 0.0,
            camera_pitch_ease_out: 0.0,
            camera_yaw_radians,
            camera_yaw_target: None,
            camera_yaw_start: camera_yaw_radians,
            camera_yaw_duration: 0.0,
            camera_yaw_elapsed: 0.0,
            camera_yaw_ease_in: 0.0,
            camera_yaw_ease_out: 0.0,
            camera_shake_offset: Vec3::ZERO,
            screen_shake_intensity: 0.0,
            screen_shake_angle_cos: 0.0,
            screen_shake_angle_sin: 0.0,
            script_camera_shakers: Vec::new(),
            script_fps_limit: None,
            script_fps_limit_last_tick: None,
            camera_slave_mode: None,
            view_matrix,
            projection_matrix,
            keys_pressed: HashSet::new(),
            mouse_position: (0.0, 0.0),
            mouse_world_position: Vec3::ZERO,
            is_dragging: false,
            selection_start: None,
            selected_objects: Vec::new(),
            control_groups: HashMap::new(),
            current_player_id: 0,
            game_paused: false,
            show_debug_info: debug_overlay,
            show_health_bars: true,
            frame_counter: 0,
            fps: 0.0,
            last_frame_timing: None,
            frame_clock: FrameClock::new(),
            menu_loading_tick_accumulator: Duration::ZERO,
            menu_loading_last_tick: Instant::now(),
            diagnostics_overlay: None,
            ui_manager,
            wgpu_ui_system,
            game_hud: GameHUD::new(),
            active_menu_shell_hook: None,
            gpui_menu_bridge: None,
            models_loaded: true, // Already loaded during init
            pending_shell_model_prewarm,
            menu_enter_frame: if start_in_menu { Some(0) } else { None },
            shell_ui_enqueued_frame: None,
            last_menu_stall_warning: None,
            match_over: false,
            victory_summary: None,
        };

        // Start background music
        // DISABLED: Using proper AssetManager audio system instead of synthetic tones
        // engine.start_background_music();

        // Display subsystem status
        if let Some(subsystem_manager) = get_subsystem_manager() {
            let stats = subsystem_manager.lock().get_stats();
            info!("📊 Subsystem Status:");
            info!("  ✅ {} subsystems initialized", stats.total_subsystems);
            if let Some(init_time) = stats.initialization_time {
                info!("  ⏱️ Total init time: {:.2}ms", init_time.as_millis());
            }
        }

        info!("🎉 C&C Game Engine with Enhanced Subsystem Architecture initialized successfully!");
        let total_duration = total_timer.finish();
        info!(
            "⏱️ Total Engine Initialization Time: {:.2}s",
            total_duration.as_secs_f32()
        );
        if asset_duration > Duration::ZERO {
            info!("   Asset Manager: {:.2}s", asset_duration.as_secs_f32());
        } else {
            info!("   Asset Manager: deferred to startup loading worker");
        }
        info!("🎮 Controls:");
        info!("  WASD - Move camera");
        info!("  Mouse - Select units");
        info!("  Right click - Move/Attack command");
        info!("  SPACE - Pause game");
        info!("  F1 - Toggle debug info");
        info!("  M - Toggle music");
        info!("  ESC - Exit game");

        engine
            .window
            .set_title("Command & Conquer Generals Zero Hour - Loading...");
        engine.ensure_shell_loading_overlay();
        engine.update_shell_loading_progress(0.0, Some("Loading assets..."));

        Ok(engine)
    }

    fn apply_command_line_overrides(command_line: &CommandLineArgs) {
        let mut applied = false;

        if let Some(handle) = get_subsystem_manager() {
            let mut manager = handle.lock();
            if let Some(subsystem) = manager.get_mut::<GlobalDataSubsystem>() {
                if let Some(global) = subsystem.get_global_data_mut() {
                    if command_line.quick_start {
                        global.apply_quick_start();
                    }
                    if let Some(lang) = command_line.language.as_deref() {
                        global.set_language(lang);
                    }
                    if let Some(mod_name) = command_line.mod_name.as_deref() {
                        global.set_active_mod(mod_name);
                    }
                    applied = true;
                }
            }
        }

        if !applied {
            debug!("GlobalData subsystem unavailable; command line overrides skipped");
        }

        let language = command_line.language.as_deref().unwrap_or("English");
        localization::set_language(language);
    }

    /// Pre-load all unit models into the graphics system
    async fn preload_unit_models_to_graphics_system(
        graphics_system: &mut GraphicsSystem,
    ) -> Result<()> {
        info!("🎮 Pre-loading C&C unit models into graphics system...");

        // Initialize a temporary game logic instance to get the templates
        let mut temp_game_logic = GameLogic::initialize();
        // Need to setup templates since initialize() doesn't do it
        temp_game_logic.start_new_game(crate::game_logic::GameMode::Skirmish);
        let templates = temp_game_logic.get_templates();

        // List of all unit types that need models loaded
        let unit_types = vec![
            // USA units
            "USA_Ranger",
            "USA_MissileDefender",
            "USA_Humvee",
            "USA_CrusaderTank",
            "USA_PaladinTank",
            "USA_Raptor",
            // GLA units
            "GLA_Soldier",
            "GLA_RPGTrooper",
            "GLA_Technical",
            "GLA_ScorpionTank",
            "GLA_MarauderTank",
            // China units
            "China_RedGuard",
            "China_TankHunter",
            "China_BattlemasterTank",
            "China_OverlordTank",
            "China_MiG",
            "China_Helix",
            // Buildings
            "CommandCenter",
            "SupplyCenter",
            "PowerPlant",
            "Barracks",
            "WarFactory",
        ];

        if let Some(asset_manager_arc) = get_asset_manager() {
            let mut asset_manager = asset_manager_arc.lock().unwrap();
            let mut loaded_count = 0;
            let total_units = unit_types.len();

            for unit_type in &unit_types {
                println!("📋 Loading W3D model for template: {}", unit_type);

                // Look up the template to get the correct model name
                if let Some(template) = templates.get(*unit_type) {
                    if let Some(model_name) = &template.model_name {
                        println!(
                            "🎯 Template '{}' maps to W3D model: '{}'",
                            unit_type, model_name
                        );

                        // Try to load the W3D model using the correct filename
                        match asset_manager.load_w3d_model_async(model_name).await {
                            Ok(model) => {
                                println!("✅ Successfully loaded W3D model: '{}' for template '{}' ({} meshes, {} total vertices)",
                                    model_name,
                                    unit_type,
                                    model.meshes.len(),
                                    model.meshes.iter().map(|m| m.vertices.len()).sum::<usize>()
                                );
                                // Cache the model in graphics system using both keys
                                graphics_system.cache_model(unit_type.to_string(), model.clone());
                                graphics_system.cache_model(model_name.clone(), model);
                                loaded_count += 1;
                            }
                            Err(e) => {
                                println!("❌ CRITICAL: Failed to load W3D model '{}' for template '{}': {}", model_name, unit_type, e);
                                println!(
                                    "❌ This means '{}' units will not be visible in game!",
                                    unit_type
                                );
                                // Continue loading other models even if one fails
                            }
                        }
                    } else {
                        println!("⚠️ CRITICAL: Template '{}' has no model_name defined - units will be invisible!", unit_type);
                    }
                } else {
                    println!(
                        "❌ CRITICAL: Template '{}' not found in templates!",
                        unit_type
                    );
                }
            }

            info!(
                "📦 Successfully pre-loaded {}/{} unit models into graphics system",
                loaded_count, total_units
            );
        } else {
            error!("❌ Asset manager not available for model preloading");
        }

        Ok(())
    }

    /// Pre-load all unit models that will be used in the game
    async fn preload_unit_models(loaded_models: &mut HashMap<String, Arc<W3DModel>>) -> Result<()> {
        info!("🎮 Pre-loading C&C unit models...");

        // Initialize a temporary game logic instance to get the templates
        let mut temp_game_logic = GameLogic::initialize();
        // Need to setup templates since initialize() doesn't do it
        temp_game_logic.start_new_game(crate::game_logic::GameMode::Skirmish);
        let templates = temp_game_logic.get_templates();

        // List of all unit types that need models loaded
        let unit_types = vec![
            // USA units
            "USA_Ranger",
            "USA_MissileDefender",
            "USA_Humvee",
            "USA_CrusaderTank",
            "USA_PaladinTank",
            "USA_Raptor",
            // GLA units
            "GLA_Soldier",
            "GLA_RPGTrooper",
            "GLA_Technical",
            "GLA_ScorpionTank",
            "GLA_MarauderTank",
            // China units
            "China_RedGuard",
            "China_TankHunter",
            "China_BattlemasterTank",
            "China_OverlordTank",
            "China_MiG",
            "China_Helix",
            // Buildings
            "CommandCenter",
            "SupplyCenter",
            "PowerPlant",
            "Barracks",
            "WarFactory",
        ];

        if let Some(asset_manager_arc) = get_asset_manager() {
            let mut asset_manager = asset_manager_arc.lock().unwrap();
            let mut loaded_count = 0;
            let total_units = unit_types.len();

            for unit_type in &unit_types {
                println!("📋 Loading W3D model for template: {}", unit_type);

                // Look up the template to get the correct model name
                if let Some(template) = templates.get(*unit_type) {
                    if let Some(model_name) = &template.model_name {
                        println!(
                            "🎯 Template '{}' maps to W3D model: '{}'",
                            unit_type, model_name
                        );

                        // Try to load the W3D model using the correct filename
                        match asset_manager.load_w3d_model_async(model_name).await {
                            Ok(model) => {
                                println!("✅ Successfully loaded W3D model: '{}' for template '{}' ({} meshes, {} total vertices)",
                                    model_name,
                                    unit_type,
                                    model.meshes.len(),
                                    model.meshes.iter().map(|m| m.vertices.len()).sum::<usize>()
                                );
                                // Store the model using both the template name AND the model name as keys
                                // This ensures compatibility with both template-based and model-based lookups
                                loaded_models
                                    .insert(unit_type.to_string(), Arc::new(model.clone()));
                                loaded_models.insert(model_name.clone(), Arc::new(model));
                                loaded_count += 1;
                            }
                            Err(e) => {
                                println!("❌ CRITICAL: Failed to load W3D model '{}' for template '{}': {}", model_name, unit_type, e);
                                println!(
                                    "❌ This means '{}' units will not be visible in game!",
                                    unit_type
                                );
                                // Continue loading other models even if one fails
                            }
                        }
                    } else {
                        println!("⚠️ CRITICAL: Template '{}' has no model_name defined - units will be invisible!", unit_type);
                    }
                } else {
                    println!(
                        "❌ CRITICAL: Template '{}' not found in templates!",
                        unit_type
                    );
                }
            }

            info!(
                "📦 Successfully pre-loaded {}/{} unit models",
                loaded_count, total_units
            );
        } else {
            error!("❌ Asset manager not available for model preloading");
        }

        Ok(())
    }

    /// Create GPU buffers for all loaded W3D models
    fn create_model_buffers(
        loaded_models: &HashMap<String, Arc<W3DModel>>,
        device: &wgpu::Device,
        model_buffers: &mut HashMap<String, (wgpu::Buffer, wgpu::Buffer, u32)>,
    ) -> Result<()> {
        info!(
            "🔧 Creating GPU buffers for {} loaded models...",
            loaded_models.len()
        );

        // Keep track of processed models to avoid duplicates
        let mut processed_models: std::collections::HashSet<*const W3DModel> =
            std::collections::HashSet::new();

        for (model_key, w3d_model) in loaded_models {
            // Skip if we've already processed this exact model instance
            let model_ptr = w3d_model.as_ref() as *const W3DModel;
            if processed_models.contains(&model_ptr) {
                continue;
            }
            processed_models.insert(model_ptr);

            for (mesh_idx, mesh) in w3d_model.meshes.iter().enumerate() {
                let mesh_key = format!("{}_{}", model_key, mesh_idx);

                // Skip if buffer already exists
                if model_buffers.contains_key(&mesh_key) {
                    continue;
                }

                // Convert W3D vertices to C++ SAGE VertexFormatXYZNDUV2 format
                let material_color = mesh.material.diffuse_color;
                let vertices: Vec<VertexXYZNDUV2> = mesh
                    .vertices
                    .iter()
                    .map(|v| {
                        // Pack diffuse color as RGBA bytes (D3D8 style)
                        let r = ((v.color[0] * material_color.x * 255.0) as u32).min(255);
                        let g = ((v.color[1] * material_color.y * 255.0) as u32).min(255);
                        let b = ((v.color[2] * material_color.z * 255.0) as u32).min(255);
                        let a = ((v.color[3] * 255.0) as u32).min(255);
                        let diffuse_packed = (a << 24) | (r << 16) | (g << 8) | b;

                        VertexXYZNDUV2 {
                            position: v.position,
                            normal: v.normal,
                            diffuse: diffuse_packed,
                            tex_coords0: v.uv,       // Primary texture coordinates
                            tex_coords1: [0.0, 0.0], // Secondary UV for multi-texturing
                        }
                    })
                    .collect();

                // Create vertex buffer
                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{} Vertex Buffer", mesh_key)),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

                // Convert indices to u16 format
                let indices: Vec<u16> = mesh.indices.iter().map(|&i| i as u16).collect();
                let index_count = indices.len() as u32;

                // Create index buffer
                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{} Index Buffer", mesh_key)),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

                let buffer_data = (vertex_buffer, index_buffer, index_count);
                model_buffers.insert(mesh_key.clone(), buffer_data);

                info!(
                    "✅ Created GPU buffers for mesh: {} ({} vertices, {} indices)",
                    mesh_key,
                    vertices.len(),
                    index_count
                );
            }
        }

        info!(
            "📦 Created GPU buffers for {} model meshes total",
            model_buffers.len()
        );
        Ok(())
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            if let Err(err) = ww3d_engine::resize(new_size.width.max(1), new_size.height.max(1)) {
                warn!("WW3D resize failed: {err:?}");
            }

            // Update projection matrix
            self.projection_matrix = Mat4::perspective_rh(
                DEFAULT_VIEW_FOV_RADIANS,
                new_size.width as f32 / new_size.height as f32,
                DEFAULT_VIEW_NEAR_CLIP,
                DEFAULT_VIEW_FAR_CLIP,
            );
            self.ui_manager.resize(new_size.width, new_size.height);
            self.wgpu_ui_system.resize(new_size);
        }
    }

    /// Process platform-specific window events through message handler
    pub fn process_platform_event(&mut self, event: &Event<()>) -> Result<bool> {
        self.message_processor.process_event(event)
    }

    /// Check if quit has been requested through platform message handling
    pub fn is_quit_requested(&self) -> bool {
        // Check if the platform-specific handler has requested quit
        // This would require access to the handler, which we'll implement later
        false
    }

    /// Set fullscreen mode and notify platform handler
    pub fn set_fullscreen(&mut self, fullscreen: bool) -> Result<()> {
        info!("🖥️ Setting fullscreen mode: {}", fullscreen);

        // Update the message processor's fullscreen state
        self.message_processor.set_fullscreen(fullscreen);

        // In a complete implementation, we would:
        // 1. Change the winit window to fullscreen/windowed
        // 2. Reconfigure the surface
        // 3. Update render targets

        if fullscreen {
            info!("Switching to fullscreen mode");
            // self.window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        } else {
            info!("Switching to windowed mode");
            // self.window.set_fullscreen(None);
        }

        Ok(())
    }

    /// Get current application focus state
    pub fn is_application_active(&self) -> bool {
        self.message_processor.is_active()
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: key,
                        state,
                        ..
                    },
                ..
            } => {
                let route_keyboard_to_legacy_ui =
                    matches!(self.current_state, GameState::Playing | GameState::Paused);
                match state {
                    ElementState::Pressed => {
                        self.keys_pressed.insert(key.clone());
                        if route_keyboard_to_legacy_ui {
                            if let Some(ui_key) = Self::to_ui_key_code(key) {
                                let _ = self.ui_manager.handle_key_press(ui_key);
                            }
                        }
                        self.handle_key_press(key);
                    }
                    ElementState::Released => {
                        self.keys_pressed.remove(key);
                    }
                }
                true
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let use_wgpu_menu_input = self.current_state != GameState::Exiting;
                let (ui_x, ui_y) = if use_wgpu_menu_input {
                    let (x, y) = self.resolve_wgpu_ui_input_coordinates(
                        self.mouse_position.0,
                        self.mouse_position.1,
                    );
                    let _ = self.wgpu_ui_system.handle_mouse_move(x, y);
                    (x, y)
                } else {
                    (self.mouse_position.0, self.mouse_position.1)
                };
                let x = self.mouse_position.0 as i32;
                let y = self.mouse_position.1 as i32;
                let route_mouse_to_legacy_ui =
                    matches!(self.current_state, GameState::Playing | GameState::Paused);
                if route_mouse_to_legacy_ui {
                    let ui_button = Self::to_ui_mouse_button(*button);
                    if let Some(ui_button) = ui_button {
                        let _ = self.ui_manager.handle_mouse_click(x, y, ui_button);
                    }
                }
                if use_wgpu_menu_input {
                    let ui_event = self.wgpu_ui_system.handle_mouse_click(
                        ui_x,
                        ui_y,
                        Self::to_wgpu_ui_button(*button),
                        *state == ElementState::Pressed,
                    );
                    if matches!(self.current_state, GameState::Menu | GameState::Loading)
                        && *state == ElementState::Pressed
                        && *button == MouseButton::Left
                        && matches!(ui_event, UISystemEvent::None)
                    {
                        let passive_hit = self
                            .wgpu_ui_system
                            .element_name_at_position(ui_x, ui_y)
                            .unwrap_or("<none>");
                        let interactive_hit = self
                            .wgpu_ui_system
                            .interactive_element_name_at_position(ui_x, ui_y)
                            .unwrap_or("<none>");
                        debug!(
                            "Menu/loading left click produced no action at raw=({:.1}, {:.1}) resolved=({:.1}, {:.1}) scale={:.2}; passive_hit={} interactive_hit={}",
                            self.mouse_position.0,
                            self.mouse_position.1,
                            ui_x,
                            ui_y,
                            self.window.scale_factor(),
                            passive_hit,
                            interactive_hit
                        );
                    }
                    if self.handle_runtime_ui_event(ui_event) {
                        return true;
                    }
                }

                if matches!(self.current_state, GameState::Playing | GameState::Paused) {
                    match (button, state) {
                        (MouseButton::Left, ElementState::Pressed) => {
                            self.handle_left_click();
                        }
                        (MouseButton::Left, ElementState::Released) => {
                            self.handle_left_release();
                        }
                        (MouseButton::Right, ElementState::Pressed) => {
                            self.handle_right_click();
                        }
                        _ => {}
                    }
                }
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_position = (position.x as f32, position.y as f32);
                if matches!(self.current_state, GameState::Playing | GameState::Paused) {
                    self.update_mouse_world_position();
                    self.ui_manager
                        .handle_mouse_move(position.x as i32, position.y as i32);
                }
                if self.current_state != GameState::Exiting {
                    let (ui_x, ui_y) = self
                        .resolve_wgpu_ui_input_coordinates(position.x as f32, position.y as f32);
                    let _ = self.wgpu_ui_system.handle_mouse_move(ui_x, ui_y);
                }
                true
            }
            _ => false,
        }
    }

    fn resolve_wgpu_ui_input_coordinates(&self, x: f32, y: f32) -> (f32, f32) {
        let scale_factor = self.window.scale_factor() as f32;
        if !scale_factor.is_finite() || scale_factor <= f32::EPSILON {
            return (x, y);
        }

        let candidates = if (scale_factor - 1.0).abs() < f32::EPSILON {
            [(x, y), (x, y), (x, y)]
        } else {
            [
                (x, y),
                (x * scale_factor, y * scale_factor),
                (x / scale_factor, y / scale_factor),
            ]
        };

        let mut passive_hit: Option<(f32, f32)> = None;
        for (candidate_x, candidate_y) in candidates {
            if self
                .wgpu_ui_system
                .interactive_element_name_at_position(candidate_x, candidate_y)
                .is_some()
            {
                return (candidate_x, candidate_y);
            }

            if passive_hit.is_none()
                && self
                    .wgpu_ui_system
                    .element_name_at_position(candidate_x, candidate_y)
                    .is_some()
            {
                passive_hit = Some((candidate_x, candidate_y));
            }
        }

        passive_hit.unwrap_or((x, y))
    }

    fn to_ui_mouse_button(button: MouseButton) -> Option<crate::ui::MouseButton> {
        match button {
            MouseButton::Left => Some(crate::ui::MouseButton::Left),
            MouseButton::Right => Some(crate::ui::MouseButton::Right),
            MouseButton::Middle => Some(crate::ui::MouseButton::Middle),
            MouseButton::Back => Some(crate::ui::MouseButton::Other(4)),
            MouseButton::Forward => Some(crate::ui::MouseButton::Other(5)),
            MouseButton::Other(id) => Some(crate::ui::MouseButton::Other(id as u8)),
        }
    }

    fn to_wgpu_ui_button(button: MouseButton) -> u32 {
        match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            MouseButton::Back => 4,
            MouseButton::Forward => 5,
            MouseButton::Other(id) => id as u32,
        }
    }

    fn to_ui_key_code(key: &Key) -> Option<crate::ui::KeyCode> {
        match key {
            Key::Named(NamedKey::Escape) => Some(crate::ui::KeyCode::Escape),
            Key::Named(NamedKey::Enter) => Some(crate::ui::KeyCode::Enter),
            Key::Named(NamedKey::Space) => Some(crate::ui::KeyCode::Space),
            Key::Named(NamedKey::Tab) => Some(crate::ui::KeyCode::Tab),
            Key::Named(NamedKey::Backspace) => Some(crate::ui::KeyCode::Backspace),
            Key::Named(NamedKey::Delete) => Some(crate::ui::KeyCode::Delete),
            Key::Named(NamedKey::ArrowLeft) => Some(crate::ui::KeyCode::Left),
            Key::Named(NamedKey::ArrowRight) => Some(crate::ui::KeyCode::Right),
            Key::Named(NamedKey::ArrowUp) => Some(crate::ui::KeyCode::Up),
            Key::Named(NamedKey::ArrowDown) => Some(crate::ui::KeyCode::Down),
            Key::Named(NamedKey::F1) => Some(crate::ui::KeyCode::F1),
            Key::Named(NamedKey::F2) => Some(crate::ui::KeyCode::F2),
            Key::Named(NamedKey::F3) => Some(crate::ui::KeyCode::F3),
            Key::Named(NamedKey::F4) => Some(crate::ui::KeyCode::F4),
            Key::Named(NamedKey::F5) => Some(crate::ui::KeyCode::F5),
            Key::Named(NamedKey::F6) => Some(crate::ui::KeyCode::F6),
            Key::Named(NamedKey::F7) => Some(crate::ui::KeyCode::F7),
            Key::Named(NamedKey::F8) => Some(crate::ui::KeyCode::F8),
            Key::Named(NamedKey::F9) => Some(crate::ui::KeyCode::F9),
            Key::Named(NamedKey::F10) => Some(crate::ui::KeyCode::F10),
            Key::Named(NamedKey::F11) => Some(crate::ui::KeyCode::F11),
            Key::Named(NamedKey::F12) => Some(crate::ui::KeyCode::F12),
            Key::Character(ch) if ch.len() == 1 => {
                let c = ch.chars().next()?;
                match c.to_ascii_uppercase() {
                    'A' => Some(crate::ui::KeyCode::A),
                    'B' => Some(crate::ui::KeyCode::B),
                    'C' => Some(crate::ui::KeyCode::C),
                    'D' => Some(crate::ui::KeyCode::D),
                    'E' => Some(crate::ui::KeyCode::E),
                    'F' => Some(crate::ui::KeyCode::F),
                    'G' => Some(crate::ui::KeyCode::G),
                    'H' => Some(crate::ui::KeyCode::H),
                    'I' => Some(crate::ui::KeyCode::I),
                    'J' => Some(crate::ui::KeyCode::J),
                    'K' => Some(crate::ui::KeyCode::K),
                    'L' => Some(crate::ui::KeyCode::L),
                    'M' => Some(crate::ui::KeyCode::M),
                    'N' => Some(crate::ui::KeyCode::N),
                    'O' => Some(crate::ui::KeyCode::O),
                    'P' => Some(crate::ui::KeyCode::P),
                    'Q' => Some(crate::ui::KeyCode::Q),
                    'R' => Some(crate::ui::KeyCode::R),
                    'S' => Some(crate::ui::KeyCode::S),
                    'T' => Some(crate::ui::KeyCode::T),
                    'U' => Some(crate::ui::KeyCode::U),
                    'V' => Some(crate::ui::KeyCode::V),
                    'W' => Some(crate::ui::KeyCode::W),
                    'X' => Some(crate::ui::KeyCode::X),
                    'Y' => Some(crate::ui::KeyCode::Y),
                    'Z' => Some(crate::ui::KeyCode::Z),
                    '0' => Some(crate::ui::KeyCode::Key0),
                    '1' => Some(crate::ui::KeyCode::Key1),
                    '2' => Some(crate::ui::KeyCode::Key2),
                    '3' => Some(crate::ui::KeyCode::Key3),
                    '4' => Some(crate::ui::KeyCode::Key4),
                    '5' => Some(crate::ui::KeyCode::Key5),
                    '6' => Some(crate::ui::KeyCode::Key6),
                    '7' => Some(crate::ui::KeyCode::Key7),
                    '8' => Some(crate::ui::KeyCode::Key8),
                    '9' => Some(crate::ui::KeyCode::Key9),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn handle_runtime_ui_event(&mut self, event: UISystemEvent) -> bool {
        match event {
            UISystemEvent::None => false,
            UISystemEvent::StartGame { mode, faction } => {
                self.start_game_from_ui(mode, faction, DEFAULT_SKIRMISH_MAP.to_string());
                self.request_state_change(GameState::Playing);
                true
            }
            UISystemEvent::ExitGame => {
                self.request_state_change(GameState::Exiting);
                true
            }
            UISystemEvent::ShowOptions => {
                self.ui_manager.transition_to_screen(Screen::Options);
                true
            }
            UISystemEvent::LoadGame => {
                self.ui_manager.transition_to_screen(Screen::LoadGame);
                true
            }
            UISystemEvent::BackToMainMenu => {
                self.request_state_change(GameState::Menu);
                true
            }
            UISystemEvent::PauseToggle => {
                self.toggle_pause();
                true
            }
            UISystemEvent::ButtonClicked { .. } => true,
        }
    }

    pub fn update_with_timing(&mut self, timing: &FrameTiming) {
        if matches!(self.current_state, GameState::Menu | GameState::Loading) {
            // C++ shell/loading progression is driven by the engine's fixed update cadence, not
            // by renderer timing. WW3D timing can remain effectively stuck during startup/menu
            // and starve shell scripts/model streaming if we trust it here.
            self.update_with_frame_clock();
            return;
        }
        let dt = self.apply_frame_timing(*timing);
        self.update_internal(dt);
    }

    /// Allows external orchestrators (e.g., integration diagnostics pipeline) to push
    /// the latest subsystem health snapshot for the in-game debug overlay.
    pub fn set_diagnostics_overlay(&mut self, stats: DiagnosticsOverlayStats) {
        self.diagnostics_overlay = Some(stats);
    }

    #[cfg(feature = "integration-diagnostics")]
    pub fn set_integration_diagnostics(&mut self, diag: &SystemDiagnostics) {
        self.diagnostics_overlay = Some(DiagnosticsOverlayStats::from_system(diag));
    }

    /// Clears any externally provided diagnostics snapshot, falling back to
    /// locally-derived estimates.
    pub fn clear_diagnostics_overlay(&mut self) {
        self.diagnostics_overlay = None;
    }

    /// Advance simulation using the internal fallback clock (no WW3D timing).
    pub fn update_with_frame_clock(&mut self) {
        // Shell/loading logic must tick at a stable 30 FPS cadence. The event loop can deliver
        // redraws much faster than that, and advancing on every redraw overdrives shell scripts
        // and menu transitions (visible as menu flicker/disappear behavior).
        const SHELL_MENU_STEP: Duration = Duration::from_nanos(33_333_333);

        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.menu_loading_last_tick);
        self.menu_loading_last_tick = now;
        self.menu_loading_tick_accumulator =
            (self.menu_loading_tick_accumulator + elapsed).min(Duration::from_millis(250));

        if self.menu_loading_tick_accumulator < SHELL_MENU_STEP {
            return;
        }
        self.menu_loading_tick_accumulator -= SHELL_MENU_STEP;

        let clock_timing = self.frame_clock.advance_fixed(SHELL_MENU_STEP);
        let timing = Self::to_engine_timing(clock_timing, Instant::now());
        let dt = self.apply_frame_timing(timing);
        self.update_internal(dt);
    }

    pub fn update(&mut self, dt: f32) {
        let delta = Duration::from_secs_f32(dt.max(0.0));
        let clock_timing = self.frame_clock.advance_fixed(delta);
        let timing = Self::to_engine_timing(clock_timing, Instant::now());
        let adjusted_dt = self.apply_frame_timing(timing);
        self.update_internal(adjusted_dt);
    }

    fn apply_frame_timing(&mut self, timing: FrameTiming) -> f32 {
        if matches!(self.current_state, GameState::Menu | GameState::Loading) {
            // Shell/loading frame cadence is managed by update_with_frame_clock() and event-loop
            // pacing. Running gameplay script FPS spin-waits here can stall the UI thread.
            self.script_fps_limit_last_tick = None;
        } else {
            self.apply_script_frame_limit();
        }
        NetworkClock::override_with_duration(timing.total_time);
        let dt = timing.delta_seconds().max(0.0);
        self.last_frame_timing = Some(timing);
        let incoming_frame = timing.frame_number as u32;
        self.frame_counter = if incoming_frame > self.frame_counter {
            incoming_frame
        } else {
            self.frame_counter.saturating_add(1)
        };
        if timing.fps > 0.0 {
            self.fps = timing.fps;
        } else if dt > 0.0 {
            self.fps = 1.0 / dt;
        }
        dt
    }

    /// Get current game state
    pub fn get_state(&self) -> GameState {
        self.current_state
    }

    /// Request state transition - will be applied at next update cycle
    /// Matches C++ GameEngine::setQuitting() pattern for deferred state changes
    pub fn request_state_change(&mut self, new_state: GameState) {
        if new_state != self.current_state {
            info!(
                "State transition requested: {:?} -> {:?}",
                self.current_state, new_state
            );
            self.pending_state = Some(new_state);
        }
    }

    /// Process pending state transitions
    /// Called at beginning of update cycle to handle state changes
    fn process_state_transitions(&mut self) {
        if let Some(new_state) = self.pending_state.take() {
            self.transition_to_state(new_state);
        }
    }

    /// Execute state transition with proper setup/cleanup
    /// Matches C++ GameEngine reset() and initialization patterns
    fn transition_to_state(&mut self, new_state: GameState) {
        let old_state = self.current_state;

        info!("State transition: {:?} -> {:?}", old_state, new_state);

        // Exit current state
        match old_state {
            GameState::Menu => {
                debug!("Exiting Menu state");
            }
            GameState::Loading => {
                debug!("Exiting Loading state");
                self.hide_shell_loading_overlay();
            }
            GameState::Playing => {
                debug!("Exiting Playing state");
                // Could pause audio, save state, etc.
            }
            GameState::Paused => {
                debug!("Exiting Paused state");
            }
            GameState::Exiting => {
                debug!("Already exiting");
            }
        }

        // Enter new state
        match new_state {
            GameState::Menu => {
                info!("Entering Menu state");
                // C++ shell menus keep the shell map simulation alive behind the UI.
                self.game_paused = false;
                self.game_logic.set_paused(false);
                self.active_menu_shell_hook = None;
                self.hide_gameplay_layouts();
                self.ui_manager.transition_to_screen(crate::ui::Screen::MainMenu);
                self.wgpu_ui_system.set_state(UISystemState::MainMenu);
            }
            GameState::Loading => {
                info!("Entering Loading state");
                self.shutdown_gpui_menu_bridge();
                // Show loading screen, prepare assets
                self.ensure_shell_loading_overlay();
                self.update_shell_loading_progress(0.0, Some("Loading assets..."));
                self.ui_manager
                    .transition_to_screen(crate::ui::Screen::Loading);
                self.wgpu_ui_system.set_state(UISystemState::Loading);
            }
            GameState::Playing => {
                info!("Entering Playing state");
                self.shutdown_gpui_menu_bridge();
                // Start game logic, enable input
                self.game_paused = false;
                self.game_logic.set_paused(false);
                self.ensure_gameplay_layouts();
                self.ui_manager
                    .transition_to_screen(crate::ui::Screen::GameHUD);
                self.wgpu_ui_system.set_state(UISystemState::InGame);
            }
            GameState::Paused => {
                info!("Entering Paused state");
                self.shutdown_gpui_menu_bridge();
                // Freeze game logic, show pause menu
                self.game_paused = true;
                self.game_logic.set_paused(true);
                self.ui_manager
                    .transition_to_screen(crate::ui::Screen::PauseMenu);
                self.wgpu_ui_system.set_state(UISystemState::PauseMenu);
            }
            GameState::Exiting => {
                info!("Entering Exiting state - beginning shutdown");
                self.shutdown_gpui_menu_bridge();
                // Cleanup will happen in drop
            }
        }

        self.current_state = new_state;
        if matches!(new_state, GameState::Menu | GameState::Loading) {
            self.menu_loading_tick_accumulator = Duration::ZERO;
            self.menu_loading_last_tick = Instant::now();
        }
        if new_state == GameState::Menu {
            self.menu_enter_frame = Some(self.current_startup_logic_frame());
        } else {
            self.menu_enter_frame = None;
            self.shell_ui_enqueued_frame = None;
            self.active_menu_shell_hook = None;
        }
    }

    /// Check if engine should quit
    /// Matches C++ GameEngine::isQuitting()
    pub fn is_quitting(&self) -> bool {
        self.current_state == GameState::Exiting
    }

    fn network_frame_data_ready_gate(multiplayer_session_active: bool) -> Option<bool> {
        with_subsystem_mut::<NetworkSubsystem, _>(|network| {
            let has_network_backend = crate::network::has_active_network_interface();
            if multiplayer_session_active && has_network_backend {
                let frame_ready = crate::network::active_session_frame_data_ready().unwrap_or(true);
                network.set_session_state(true, frame_ready);
                Some(network.is_frame_data_ready())
            } else {
                // C++ parity: without a live network session object, treat as offline gate.
                network.set_session_state(false, true);
                None
            }
        })
        .flatten()
    }

    fn should_update_game_logic_frame(
        game_paused: bool,
        network_frame_data_ready: Option<bool>,
    ) -> bool {
        match network_frame_data_ready {
            Some(frame_ready) => frame_ready,
            None => !game_paused,
        }
    }

    fn update_runtime_subsystems(&mut self, dt: f32) {
        if let Some(subsystem_manager) = get_subsystem_manager() {
            let mut guard = subsystem_manager.lock();
            if let Some(timing) = self.last_frame_timing {
                if let Err(e) = guard.update_all_with_timing(&timing) {
                    error!("Error updating subsystems: {}", e);
                }
            } else if let Err(e) = guard.update_all(dt) {
                error!("Error updating subsystems: {}", e);
            }
        }
    }

    fn update_internal(&mut self, dt: f32) {
        // Process any pending state transitions first
        self.process_state_transitions();

        // Early exit if we're shutting down
        if self.is_quitting() {
            return;
        }

        self.maybe_trigger_deferred_caustic_warmup();

        let dt = dt.max(0.0);
        let visual_dt = dt * self.game_logic.visual_speed_multiplier().max(0.0);
        // C++ parity: radar/audio/client/message/network/cd updates happen each frame.
        self.update_runtime_subsystems(dt);
        // State-based update logic - matches C++ GameEngine::update() conditional updates
        match self.current_state {
            GameState::Menu => {
                self.cleanup_sound_effects();
                if self.game_logic.isInShellGame() && !self.game_paused {
                    let shell_update_started = Instant::now();
                    // Keep shell map/scripts alive in menu without allowing large fixed-step
                    // catch-up loops to block the UI thread.
                    self.game_logic.update_shell_with_budget(dt, 2);
                    if let Some(fps) = self.game_logic.take_script_fps_limit_request() {
                        self.apply_script_fps_limit_request(fps);
                    }
                    let shell_elapsed = shell_update_started.elapsed();
                    if shell_elapsed >= Duration::from_millis(40) {
                        let fixed_diag = self.game_logic.fixed_step_diagnostics();
                        warn!(
                            "Slow shell menu tick: {:?} (state={:?}, frame={}, fixed_steps={}, budget_hit={}, acc_ms={:.2})",
                            shell_elapsed,
                            self.current_state,
                            self.frame_counter,
                            fixed_diag.steps_run,
                            fixed_diag.budget_hit,
                            fixed_diag.accumulated_time_seconds * 1000.0
                        );
                    }
                }
                self.wgpu_ui_system.set_state(UISystemState::MainMenu);
                self.wgpu_ui_system.update();
                if let Err(err) = self.ui_manager.update(dt) {
                    warn!("UI manager update failed in menu state: {}", err);
                }
                return;
            }
            GameState::Loading => {
                // In loading: minimal updates, mainly for loading screen animations
                if let Err(err) = self.update_startup_loading() {
                    error!("Startup loading failed: {}", err);
                    self.request_state_change(GameState::Exiting);
                    return;
                }
                if self.current_state != GameState::Loading {
                    // Loading completed and transitioned this frame; avoid re-applying loading UI.
                    return;
                }
                self.wgpu_ui_system.set_state(UISystemState::Loading);
                self.wgpu_ui_system.update();
                // After loading completes, the state will transition to Playing
                // This is handled by the initialization code setting pending_state
                return;
            }
            GameState::Paused => {
                // In paused: update UI and camera, but not game logic
                // (matches C++ where TheGameLogic->isGamePaused() prevents update)
                self.update_camera(visual_dt);
                self.cleanup_sound_effects();
                self.wgpu_ui_system.set_state(UISystemState::PauseMenu);
                self.wgpu_ui_system.update();
                if let Err(err) = self.ui_manager.update(dt) {
                    warn!("UI manager update failed in paused state: {}", err);
                }
                return;
            }
            GameState::Playing => {
                // Full update - continue below
            }
            GameState::Exiting => {
                // Exiting: no updates needed
                return;
            }
        }

        // Full update cycle for Playing state (matches C++ GameEngine::update())

        // C++ parity gate:
        //   (Network == NULL && !isGamePaused()) || (Network && Network->isFrameDataReady()).
        let network_frame_data_ready =
            Self::network_frame_data_ready_gate(self.game_logic.isInMultiplayerGame());
        if Self::should_update_game_logic_frame(self.game_paused, network_frame_data_ready) {
            // Update game logic first
            if let Some(timing) = self.last_frame_timing {
                self.game_logic.update_with_timing(&timing);
            } else {
                self.game_logic.update_with_dt(dt);
            }
            if let Some(fps) = self.game_logic.take_script_fps_limit_request() {
                self.apply_script_fps_limit_request(fps);
            }

            // C++ parity: when script time-freeze is active, gameplay simulation should not
            // advance outside script evaluation.
            if !self.game_logic.is_time_frozen_for_simulation() {
                // Update combat system
                let hits = self
                    .combat_system
                    .update_projectiles(dt, self.game_logic.get_objects_mut());

                // Play sound effects for hits
                if !hits.is_empty() {
                    self.play_sound_effect(SoundType::Hit);
                }

                // Update pathfinding for moving units
                let object_ids: Vec<ObjectId> =
                    self.game_logic.get_objects().keys().copied().collect();

                for object_id in object_ids {
                    // Move units along their paths
                    let _path_completed = self.pathfinding_system.move_unit_along_path(
                        object_id,
                        self.game_logic.get_objects_mut(),
                        dt,
                    );
                }
            }
        }

        // Update camera
        self.update_camera(visual_dt);

        // Update audio
        self.cleanup_sound_effects();
        if self.current_state == GameState::Playing {
            self.wgpu_ui_system.set_state(UISystemState::InGame);
        }
        self.wgpu_ui_system.update();

        // Process queued commands in game logic
        self.game_logic.process_commands();

        if let Some(focus) = self.game_logic.take_camera_focus_request() {
            self.center_camera_on(focus);
        }

        if let Some(focus) = self.game_logic.camera_follow_target_position() {
            self.center_camera_on(focus);
        }

        if self.game_logic.take_camera_zoom_reset() {
            self.camera_zoom = self.compute_default_camera_zoom_for_target(
                self.camera_target,
                self.game_logic.script_default_camera_max_height(),
            );
            self.camera_zoom_target = None;
            self.camera_zoom_start = self.camera_zoom;
            self.camera_zoom_duration = 0.0;
            self.camera_zoom_elapsed = 0.0;
            self.camera_zoom_ease_in = 0.0;
            self.camera_zoom_ease_out = 0.0;
            self.apply_script_camera_pitch_request(CameraPitchRequest {
                pitch: self.game_logic.script_default_camera_pitch(),
                duration_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
        }

        if let Some(request) = self.game_logic.take_camera_zoom_request() {
            if request.duration_seconds <= 0.0 {
                self.camera_zoom = request.zoom;
                self.camera_zoom_target = None;
                self.camera_zoom_start = self.camera_zoom;
                self.camera_zoom_duration = 0.0;
                self.camera_zoom_elapsed = 0.0;
                self.camera_zoom_ease_in = 0.0;
                self.camera_zoom_ease_out = 0.0;
            } else {
                self.camera_zoom_start = self.camera_zoom;
                self.camera_zoom_target = Some(request.zoom);
                self.camera_zoom_duration = request.duration_seconds;
                self.camera_zoom_elapsed = 0.0;
                self.camera_zoom_ease_in = request.ease_in_seconds.max(0.0);
                self.camera_zoom_ease_out = request.ease_out_seconds.max(0.0);
            }
        }

        if let Some(request) = self.game_logic.take_camera_pitch_request() {
            self.apply_script_camera_pitch_request(request);
        }

        if let Some(request) = self.game_logic.take_camera_rotate_request() {
            self.apply_script_camera_rotate_request(request);
        }

        if let Some(request) = self.game_logic.take_camera_look_toward_request() {
            self.apply_camera_look_toward_request(request);
        }

        if let Some(request) = self.game_logic.take_camera_slave_mode_enable_request() {
            self.camera_slave_mode = Some(request);
        }

        if self.game_logic.take_camera_slave_mode_disable_request() {
            self.camera_slave_mode = None;
        }

        for request in self.game_logic.take_screen_shake_requests() {
            self.enqueue_script_screen_shake(request.intensity);
        }

        for request in self.game_logic.take_camera_add_shaker_requests() {
            self.enqueue_script_camera_shaker(request);
        }

        // Main applies these script requests inside GameLogic evaluation (with GameClient bridges
        // when enabled). Drain pending mirrors so they don't accumulate frame-to-frame.
        let _ = self.game_logic.take_view_guardband_request();
        let _ = self.game_logic.take_camera_bw_mode_request();
        let _ = self.game_logic.take_camera_motion_blur_requests();

        for popup in self.game_logic.take_popup_message_requests() {
            if popup.pause {
                self.game_paused = true;
                self.game_logic.set_paused(true);
            }
            if popup.pause_music {
                if let Some(sink) = self.background_music.take() {
                    sink.stop();
                }
            }
        }

        if self.game_logic.take_music_stop_request() {
            if let Some(sink) = self.background_music.take() {
                sink.stop();
            }
        }

        // Broadcast defeat notifications so UI/systems mirror C++ VictoryConditions flow
        let defeated_players = self.game_logic.take_defeat_events();
        for player_id in defeated_players {
            if let Some(player) = self.game_logic.get_player(player_id) {
                let message = localization::localize_with_args(
                    "hud.message.player_defeated",
                    "{player} has been defeated!",
                    &[("player", player.name.as_str())],
                );
                info!("Player {} ({}) has been defeated", player.name, player_id);
                self.game_hud.push_info_message(&message);
                self.game_logic
                    .queue_radar_message_for_team(player.team, message.clone());
                self.game_logic.play_ui_sound("GUIMessageReceived");
            } else {
                info!("Player {} has been defeated", player_id);
            }
            fow_rendering::reveal_entire_map_for_player(player_id);
            script_events::push_event(ScriptEvent::PlayerDefeated { player_id });
            script_events::push_event(ScriptEvent::RevealMapForPlayer { player_id });
        }

        let alliance_events = self.game_logic.take_alliance_events();
        let local_player_id = self.game_logic.local_player_id();
        let mut observer_notified = false;
        for event in alliance_events {
            let is_local = local_player_id == Some(event.player_id);
            if !is_local && local_player_id.is_some() {
                continue;
            }
            if !is_local && observer_notified {
                continue;
            }

            let (key, fallback) = match event.state {
                AllianceState::AlliedVictory if is_local => {
                    ("hud.message.allied_victory", "Your alliance has triumphed!")
                }
                AllianceState::AlliedDefeat if is_local => (
                    "hud.message.allied_defeat",
                    "Your alliance has been defeated!",
                ),
                AllianceState::AlliedVictory => (
                    "hud.message.observer_allied_victory",
                    "An alliance has won the battle.",
                ),
                AllianceState::AlliedDefeat => (
                    "hud.message.observer_allied_defeat",
                    "An alliance has been defeated.",
                ),
                AllianceState::Active => continue,
            };

            let message = localization::localize(key, fallback);
            self.game_hud.push_info_message(&message);
            if let Some(event_player) = self.game_logic.get_player(event.player_id) {
                self.game_logic
                    .queue_radar_message_for_team(event_player.team, message.clone());
            } else {
                self.game_logic.queue_radar_message(message.clone());
            }
            self.game_logic.play_ui_sound("GUIMessageReceived");
            if !is_local {
                observer_notified = true;
            }

            if matches!(event.state, AllianceState::AlliedDefeat) {
                fow_rendering::reveal_entire_map_for_player(event.player_id);
                script_events::push_event(ScriptEvent::RevealMapForPlayer {
                    player_id: event.player_id,
                });
            }
            script_events::push_event(ScriptEvent::AllianceStateChanged {
                player_id: event.player_id,
                state: event.state,
            });
        }

        if !self.match_over
            && self.current_state == GameState::Playing
            && !self.game_logic.isInShellGame()
        {
            if let Some(condition) = self.game_logic.evaluate_victory_condition() {
                match condition {
                    VictoryCondition::Winner(id) => self.show_victory_screen(Some(id)),
                    VictoryCondition::Draw => self.show_victory_screen(None),
                }
            }
        }

        // Update UI system (kept for current shell compatibility)
        // Note: shell/window UI is being aligned with GPUI parity work.
        // self.ui_manager.update(dt).unwrap_or_else(|e| {
        //     error!("UI Manager update failed: {}", e);
        // });
        // self.game_hud.update(dt).unwrap_or_else(|e| {
        //     error!("Game HUD update failed: {}", e);
        // });
    }

    pub fn render(&mut self) -> Result<()> {
        let render_started = Instant::now();

        if !matches!(self.current_state, GameState::Loading | GameState::Menu) {
            let mut ui_state = self.game_logic.update_ui_state(self.current_player_id);
            if !ui_state.radar_events.is_empty() {
                for evt in &ui_state.radar_events {
                    self.game_hud
                        .add_radar_message(&evt.text, evt.position, evt.kind);
                }
            } else {
                for msg in &ui_state.radar_messages {
                    self.game_hud.push_radar_message(msg);
                }
            }
            let new_script_messages = self.game_logic.take_new_script_messages();
            for msg in &new_script_messages {
                self.game_hud.push_script_message(msg);
            }
            ui_state.current_game_time = self.game_logic.get_total_play_time();
            ui_state.fps = self.fps;
            ui_state.frame_time_ms = if self.fps > 0.0 {
                1000.0 / self.fps
            } else {
                0.0
            };
            ui_state.performance_score = (ui_state.fps / 60.0).clamp(0.0, 1.5);
            if let Some(diag) = &self.diagnostics_overlay {
                ui_state.diagnostics = Some(diag.clone());
            } else {
                ui_state.diagnostics = Some(DiagnosticsOverlayStats::from_overall(
                    ui_state.performance_score * 100.0,
                ));
            }
            ui_state.show_debug_overlay = self.show_debug_info;
            if let Some(manager_arc) = get_asset_manager() {
                if let Ok(manager) = manager_arc.lock() {
                    let stats = manager.get_statistics();
                    ui_state.assets_loaded = stats.archive_stats.total_files as u64;
                    ui_state.asset_memory_mb = 0.0;
                    ui_state.asset_cache_usage = 0.0;
                }
            }
            self.process_ui_events();
            ui_state.minimap_texture_id = self.render_pipeline.get_minimap_texture_id();
            ui_state.minimap_coordinates = self.render_pipeline.get_minimap_coordinates().cloned();
            self.update_minimap_viewport(&mut ui_state);
            let world_bounds = self.game_logic.world_bounds();
            self.game_hud
                .update_radar_pings(&ui_state.radar_pings, world_bounds.0, world_bounds.1);
            for msg in &ui_state.radar_messages {
                self.game_hud.push_radar_message(msg);
            }
            for evt in &ui_state.radar_events {
                self.game_hud
                    .add_radar_message(&evt.text, evt.position, evt.kind);
            }

            ui_state.match_over = self.match_over;
            if let Some(summary) = &self.victory_summary {
                ui_state.victory_summary = Some(summary.clone());
                ui_state.player_outcome = summary
                    .player_results
                    .iter()
                    .find(|result| result.player_id == self.current_player_id)
                    .map(|result| result.outcome);
            } else {
                ui_state.victory_summary = None;
                ui_state.player_outcome = None;
            }
        }

        if matches!(
            self.current_state,
            GameState::Menu | GameState::Loading | GameState::Paused
        ) {
            self.wgpu_ui_system
                .render()
                .map_err(|err| anyhow::anyhow!("runtime UI backend render failed: {err}"))?;
            self.drain_renderer_attachments();
            return Ok(());
        }

        // Execute the main game render pipeline using the WW3D frame.
        let render_time_delta = if self.game_logic.is_time_frozen_for_simulation() {
            0.0
        } else {
            self.last_frame_timing
                .map(|t| t.delta_seconds())
                .unwrap_or(0.0)
                * self.game_logic.visual_speed_multiplier().max(0.0)
        };
        let allow_sync_model_loads =
            !matches!(self.current_state, GameState::Menu | GameState::Loading);
        let deferred_startup_model_load_budget = Self::startup_deferred_model_load_budget(
            self.current_state,
            self.shell_start_frame(),
            self.current_startup_logic_frame(),
        );
        let skip_world_scene = self.should_skip_world_scene_for_shell_menu();
        self.render_pipeline.execute(
            &mut self.graphics_system,
            &self.game_logic,
            &self.view_matrix,
            &self.projection_matrix,
            self.camera_position,
            render_time_delta,
            allow_sync_model_loads,
            deferred_startup_model_load_budget,
            skip_world_scene,
        )?;

        self.drain_renderer_attachments();
        Ok(())
    }

    fn update_minimap_viewport(&self, ui_state: &mut GameUIState) {
        let (world_min, world_max) = self.game_logic.world_bounds();
        let world_extent_x = (world_max.x - world_min.x).max(1.0);
        let world_extent_z = (world_max.z - world_min.z).max(1.0);

        let half_width = 200.0 / self.camera_zoom.max(0.01);
        let half_height = 150.0 / self.camera_zoom.max(0.01);

        let min_x = ((self.camera_target.x - half_width) - world_min.x) / world_extent_x;
        let max_x = ((self.camera_target.x + half_width) - world_min.x) / world_extent_x;
        let min_y = ((self.camera_target.z - half_height) - world_min.z) / world_extent_z;
        let max_y = ((self.camera_target.z + half_height) - world_min.z) / world_extent_z;

        ui_state.minimap_viewport = crate::ui::normalized_minimap_rect(min_x, min_y, max_x, max_y);
    }

    /// Process UI events emitted by UIManager and apply to engine/game state.
    fn process_ui_events(&mut self) {
        while let Some(event) = self.ui_manager.pop_event() {
            match event {
                UIEvent::StartGame { mode, faction, map } => {
                    self.start_game_from_ui(mode, faction, map);
                }
                UIEvent::LoadGame(slot) => {
                    self.load_game_from_ui(&slot);
                }
                UIEvent::SaveGame { slot, display_name } => {
                    self.save_game_from_ui(&slot, &display_name);
                }
                UIEvent::RestartMission => {
                    self.restart_mission_from_ui();
                }
                UIEvent::PlaySoundEffectPath(path) => {
                    self.play_ui_sound_effect(path);
                }
                UIEvent::TogglePause => {
                    self.toggle_pause();
                }
                UIEvent::ExitToMenu => {
                    info!("UI requested exit to menu");
                    self.game_paused = false;
                    self.ui_manager.transition_to_screen(Screen::MainMenu);
                    self.wgpu_ui_system.set_state(UISystemState::MainMenu);
                }
                UIEvent::ExitGame => {
                    info!("UI requested exit");
                    self.request_state_change(GameState::Exiting);
                }
                UIEvent::ChangeScreen(screen) => {
                    self.ui_manager.transition_to_screen(screen);
                    match screen {
                        Screen::MainMenu => self.wgpu_ui_system.set_state(UISystemState::MainMenu),
                        Screen::Loading => self.wgpu_ui_system.set_state(UISystemState::Loading),
                        Screen::GameHUD => self.wgpu_ui_system.set_state(UISystemState::InGame),
                        Screen::PauseMenu => {
                            self.wgpu_ui_system.set_state(UISystemState::PauseMenu)
                        }
                        Screen::Victory => self.wgpu_ui_system.set_state(UISystemState::Victory),
                        _ => {}
                    }
                }
                UIEvent::FocusCamera(world_pos) => {
                    self.center_camera_on(world_pos);
                }
                _ => {}
            }
        }
    }

    fn restart_mission_from_ui(&mut self) {
        let map = self.game_logic.get_current_map_name().to_string();
        let mode = self.game_logic.game_mode();
        let faction = self
            .game_logic
            .get_player(self.current_player_id)
            .map(|p| p.team.get_name().to_string())
            .unwrap_or_else(|| "USA".to_string());

        info!(
            "UI requested restart: mode={:?}, faction={}, map={}",
            mode, faction, map
        );
        self.start_game_from_ui(mode, faction, map);
    }

    fn map_ai_difficulty_to_save(difficulty: crate::ai::AIDifficulty) -> GameDifficulty {
        match difficulty {
            crate::ai::AIDifficulty::Easy => GameDifficulty::Easy,
            crate::ai::AIDifficulty::Medium => GameDifficulty::Medium,
            crate::ai::AIDifficulty::Hard | crate::ai::AIDifficulty::Brutal => GameDifficulty::Hard,
        }
    }

    fn save_game_from_ui(&mut self, slot: &str, display_name: &str) {
        let slot = slot.trim();
        if slot.is_empty() {
            return;
        }

        let map_name = self.game_logic.get_current_map_name().to_string();
        let difficulty = Self::map_ai_difficulty_to_save(self.game_logic.get_difficulty());
        let play_time = std::time::Duration::from_secs_f32(self.game_logic.get_total_play_time());

        let team_name = self
            .game_logic
            .get_player(self.current_player_id)
            .map(|player| player.team.get_name().to_string())
            .unwrap_or_else(|| "Neutral".to_string());

        let save_info = SaveGameInfo {
            filename: slot.to_string(),
            display_name: display_name.to_string(),
            description: display_name.to_string(),
            map_name,
            campaign_side: Some(team_name),
            mission_number: None,
            save_date: SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            play_time,
            difficulty,
            save_type: SaveFileType::Normal,
        };

        if let Err(err) = self
            .save_file_manager
            .save_game(slot, &self.game_logic, &save_info)
        {
            warn!("Save failed for '{}': {}", slot, err);
        } else {
            info!("Saved game to slot '{}'", slot);
        }
    }

    fn load_game_from_ui(&mut self, slot: &str) {
        let slot = slot.trim();
        if slot.is_empty() {
            return;
        }

        self.ui_manager.transition_to_screen(Screen::Loading);
        match self.save_file_manager.load_game(slot, &mut self.game_logic) {
            Ok(save_info) => {
                info!(
                    "Loaded save '{}' (map={}, name={})",
                    slot, save_info.map_name, save_info.display_name
                );

                self.game_logic.set_paused(false);
                self.game_paused = false;
                self.match_over = false;
                self.victory_summary = None;
                self.selected_objects.clear();

                Self::apply_heightmap_hint(&mut self.render_pipeline, &self.game_logic);
                Self::apply_skybox_hint(&mut self.render_pipeline, &self.game_logic);
                if let Err(err) = Self::reinitialize_minimap_renderer(
                    &mut self.render_pipeline,
                    &self.graphics_system,
                    &mut self.game_logic,
                ) {
                    warn!(
                        "Failed to reinitialize minimap renderer after load: {}",
                        err
                    );
                }
                Self::apply_map_lighting(
                    &mut self.graphics_system,
                    &mut self.render_pipeline,
                    &self.game_logic,
                );

                self.ui_manager.transition_to_screen(Screen::GameHUD);
            }
            Err(err) => {
                warn!("Load failed for '{}': {}", slot, err);
                self.ui_manager.transition_to_screen(Screen::MainMenu);
            }
        }
    }

    fn play_ui_sound_effect(&mut self, path: String) {
        let Some(bytes) = self.ui_sound_cache.get(&path).cloned() else {
            return;
        };
        let Some(handle) = self.audio_handle.as_ref() else {
            return;
        };
        let Ok(sink) = Sink::try_new(handle) else {
            return;
        };

        let cursor = std::io::Cursor::new(bytes);
        let Ok(decoder) = rodio::Decoder::new(cursor) else {
            return;
        };
        let source = decoder.convert_samples::<f32>();
        sink.append(source);
        self.sound_effects.push(sink);
    }

    /// Restart the simulation with UI-selected parameters and refresh view/minimap.
    fn start_game_from_ui(&mut self, mode: GameMode, faction: String, map: String) {
        let faction_team = Self::team_from_faction(&faction);
        let map_name = if map.trim().is_empty() {
            DEFAULT_SKIRMISH_MAP.to_string()
        } else {
            map
        };

        info!(
            "UI requested start: mode={:?}, faction={}, map={}",
            mode,
            faction_team.get_name(),
            map_name
        );

        self.game_logic.start_new_game(mode);
        // Ensure local player uses the chosen team.
        let _ = self
            .game_logic
            .set_player_team(self.current_player_id, faction_team);
        if !self.game_logic.load_map(&map_name) {
            warn!("Failed to load map '{}', falling back to default", map_name);
            let _ = self.game_logic.load_map(DEFAULT_SKIRMISH_MAP);
        }

        // Reset transient state.
        self.game_logic.set_paused(false);
        self.game_paused = false;
        self.match_over = false;
        self.victory_summary = None;
        self.selected_objects.clear();

        // Update minimap/world bounds and camera to the new map.
        Self::apply_heightmap_hint(&mut self.render_pipeline, &self.game_logic);
        Self::apply_skybox_hint(&mut self.render_pipeline, &self.game_logic);
        if let Err(err) = Self::reinitialize_minimap_renderer(
            &mut self.render_pipeline,
            &self.graphics_system,
            &mut self.game_logic,
        ) {
            warn!("Failed to reinitialize minimap renderer: {}", err);
        }

        // Apply map lighting if provided by map settings.
        Self::apply_map_lighting(
            &mut self.graphics_system,
            &mut self.render_pipeline,
            &self.game_logic,
        );

        let startup_camera_defaults = Self::configured_startup_camera_defaults();
        (self.camera_target, self.camera_position, self.camera_zoom) =
            Self::bootstrap_camera_for_loaded_map(
                &self.game_logic,
                self.current_player_id,
                startup_camera_defaults,
            );
        self.sync_orbit_from_camera_transform();
        self.ui_manager.transition_to_screen(Screen::GameHUD);
        self.wgpu_ui_system.set_state(UISystemState::InGame);
    }

    fn apply_map_lighting(
        graphics_system: &mut GraphicsSystem,
        render_pipeline: &mut RenderPipeline,
        game_logic: &GameLogic,
    ) {
        if let Some(meta) = game_logic.last_parsed_map_settings() {
            let fog_color = meta.sky_color.or(meta.sun_color);
            render_pipeline.set_environment_lighting(
                meta.sun_direction,
                meta.sun_color,
                meta.ambient_color,
                fog_color,
                meta.fog_start.zip(meta.fog_end),
            );
            graphics_system.set_lighting(
                meta.ambient_color,
                meta.sun_color,
                meta.sun_direction,
                meta.sky_color,
            );
        } else {
            render_pipeline.clear_environment_lighting();
        }
    }

    fn apply_heightmap_hint(render_pipeline: &mut RenderPipeline, game_logic: &GameLogic) {
        if let Some(path) = game_logic
            .heightmap_hint()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
        {
            render_pipeline.set_heightmap_hint(Some(path));
        }
    }

    fn apply_skybox_hint(render_pipeline: &mut RenderPipeline, game_logic: &GameLogic) {
        render_pipeline.set_skybox_enabled(game_logic.is_skybox_enabled());
        if let Some(meta) = game_logic.last_parsed_map_settings() {
            if let Some(textures) = meta.skybox_textures {
                render_pipeline.set_skybox_hint(textures);
            }
        }
    }

    fn reinitialize_minimap_renderer(
        render_pipeline: &mut RenderPipeline,
        graphics_system: &GraphicsSystem,
        game_logic: &mut GameLogic,
    ) -> anyhow::Result<()> {
        let mut world_bounds = game_logic.world_bounds();
        render_pipeline.initialize_minimap_renderer(
            graphics_system.device_arc(),
            graphics_system.queue_arc(),
            world_bounds,
        )?;

        let world_width = (world_bounds.1.x - world_bounds.0.x).abs();
        let world_height = (world_bounds.1.z - world_bounds.0.z).abs();
        if world_width <= 1.0 || world_height <= 1.0 {
            if let Some((w, h)) = render_pipeline.heightmap_world_size() {
                game_logic.override_world_size(w, h);
                world_bounds = game_logic.world_bounds();
            }
        }

        render_pipeline.sync_heightmap_world_bounds(world_bounds);
        render_pipeline.update_minimap_world_bounds(world_bounds);
        Ok(())
    }

    /// Convert a UI faction string into a Team.
    fn team_from_faction(faction: &str) -> Team {
        match faction.to_ascii_lowercase().as_str() {
            "usa" | "us" | "america" => Team::USA,
            "gla" => Team::GLA,
            "china" => Team::China,
            _ => Team::USA,
        }
    }

    fn handle_minimap_interaction(&mut self, interaction: MinimapInteraction) {
        let pointer = Vec2::new(interaction.screen_position.x, interaction.screen_position.y);
        let Some(world_pos) = self.render_pipeline.handle_minimap_click(pointer) else {
            return;
        };

        match interaction.kind {
            MinimapActionKind::LeftClick | MinimapActionKind::LeftDrag => {
                self.center_camera_on(world_pos);
            }
            MinimapActionKind::RightClick => {
                self.issue_minimap_move(world_pos);
            }
        }
    }

    fn script_pitch_to_radians(pitch: f32) -> f32 {
        // Script pitch semantics: 1.0 is default, 0.0 trends toward horizon, >1.0 toward ground.
        let clamped = pitch.clamp(-0.25, 2.0);
        let degrees = if clamped <= 1.0 {
            5.0 + clamped * 40.0
        } else {
            45.0 + (clamped - 1.0) * 40.0
        };
        degrees
            .to_radians()
            .clamp(5.0_f32.to_radians(), 85.0_f32.to_radians())
    }

    fn parabolic_ease(param: f32, ease_in_time: f32, ease_out_time: f32) -> f32 {
        let param = param.clamp(0.0, 1.0);
        let mut in_t = ease_in_time.clamp(0.0, 1.0);
        let out_t = 1.0 - ease_out_time.clamp(0.0, 1.0);
        if in_t > out_t {
            in_t = out_t;
        }
        let v0 = 1.0 + out_t - in_t;
        if param < in_t {
            if in_t <= 0.0 {
                0.0
            } else {
                param * param / (v0 * in_t)
            }
        } else if param <= out_t {
            (in_t + 2.0 * (param - in_t)) / v0
        } else {
            let denom = (1.0 - out_t).max(f32::EPSILON);
            (in_t
                + 2.0 * (out_t - in_t)
                + (2.0 * (param - out_t) + out_t * out_t - param * param) / denom)
                / v0
        }
    }

    fn apply_camera_orbit_transform(&mut self) {
        self.camera_pitch_radians = self
            .camera_pitch_radians
            .clamp(5.0_f32.to_radians(), 85.0_f32.to_radians());
        self.camera_orbit_distance = self.camera_orbit_distance.max(1.0);

        let horizontal = self.camera_orbit_distance * self.camera_pitch_radians.cos();
        let offset = Vec3::new(
            horizontal * self.camera_yaw_radians.sin(),
            self.camera_orbit_distance * self.camera_pitch_radians.sin(),
            horizontal * self.camera_yaw_radians.cos(),
        );
        self.camera_position = self.camera_target + offset + self.camera_shake_offset;
        self.view_matrix = Mat4::look_at_rh(self.camera_position, self.camera_target, Vec3::Y);
    }

    fn sync_orbit_from_camera_transform(&mut self) {
        let offset = self.camera_position - self.camera_target;
        self.camera_orbit_distance = offset.length().max(1.0);
        let horizontal = Vec2::new(offset.x, offset.z).length();
        self.camera_pitch_radians = offset
            .y
            .atan2(horizontal.max(f32::EPSILON))
            .clamp(5.0_f32.to_radians(), 85.0_f32.to_radians());
        self.camera_yaw_radians = offset.x.atan2(offset.z);

        self.camera_pitch_target = None;
        self.camera_pitch_start = self.camera_pitch_radians;
        self.camera_pitch_duration = 0.0;
        self.camera_pitch_elapsed = 0.0;
        self.camera_pitch_ease_in = 0.0;
        self.camera_pitch_ease_out = 0.0;

        self.camera_yaw_target = None;
        self.camera_yaw_start = self.camera_yaw_radians;
        self.camera_yaw_duration = 0.0;
        self.camera_yaw_elapsed = 0.0;
        self.camera_yaw_ease_in = 0.0;
        self.camera_yaw_ease_out = 0.0;

        self.apply_camera_orbit_transform();
    }

    fn apply_script_camera_pitch_request(&mut self, request: CameraPitchRequest) {
        let target_pitch = Self::script_pitch_to_radians(request.pitch);
        if request.duration_seconds <= 0.0 {
            self.camera_pitch_radians = target_pitch;
            self.camera_pitch_target = None;
            self.camera_pitch_start = self.camera_pitch_radians;
            self.camera_pitch_duration = 0.0;
            self.camera_pitch_elapsed = 0.0;
            self.camera_pitch_ease_in = 0.0;
            self.camera_pitch_ease_out = 0.0;
            self.apply_camera_orbit_transform();
            return;
        }

        self.camera_pitch_start = self.camera_pitch_radians;
        self.camera_pitch_target = Some(target_pitch);
        self.camera_pitch_duration = request.duration_seconds;
        self.camera_pitch_elapsed = 0.0;
        self.camera_pitch_ease_in = request.ease_in_seconds.max(0.0);
        self.camera_pitch_ease_out = request.ease_out_seconds.max(0.0);
    }

    fn apply_script_camera_rotate_request(&mut self, request: CameraRotateRequest) {
        let target_yaw = self.camera_yaw_radians + request.rotations * TAU;
        if request.duration_seconds <= 0.0 {
            self.camera_yaw_radians = target_yaw;
            self.camera_yaw_target = None;
            self.camera_yaw_start = self.camera_yaw_radians;
            self.camera_yaw_duration = 0.0;
            self.camera_yaw_elapsed = 0.0;
            self.camera_yaw_ease_in = 0.0;
            self.camera_yaw_ease_out = 0.0;
            self.apply_camera_orbit_transform();
            return;
        }

        self.camera_yaw_start = self.camera_yaw_radians;
        self.camera_yaw_target = Some(target_yaw);
        self.camera_yaw_duration = request.duration_seconds;
        self.camera_yaw_elapsed = 0.0;
        self.camera_yaw_ease_in = request.ease_in_seconds.max(0.0);
        self.camera_yaw_ease_out = request.ease_out_seconds.max(0.0);
    }

    fn apply_script_fps_limit_request(&mut self, fps: i32) {
        let global_default =
            with_subsystem_mut::<GlobalDataSubsystem, _>(|subsystem| -> Option<i32> {
                let global = subsystem.get_global_data_mut()?;
                global.use_fps_limit = true;
                Some(global.frames_per_second_limit)
            })
            .flatten();

        {
            let mut global = game_engine::common::global_data::write();
            global.writable.use_fps_limit = true;
        }

        let resolved_fps = if fps <= 0 {
            global_default.unwrap_or_else(|| {
                game_engine::common::global_data::read()
                    .writable
                    .frames_per_second_limit
            })
        } else {
            fps
        };

        self.script_fps_limit = u32::try_from(resolved_fps).ok().filter(|fps| *fps > 0);
        self.script_fps_limit_last_tick = None;
    }

    fn effective_fps_limit_for_frame(
        script_fps_limit: Option<u32>,
        global_use_fps_limit: bool,
        global_frames_per_second_limit: i32,
        visual_speed_multiplier: f32,
        tivo_fast_mode: bool,
        in_replay_game: bool,
    ) -> Option<u32> {
        if let Some(script_fps) = script_fps_limit.filter(|fps| *fps > 0) {
            return Some(script_fps);
        }

        // C++ parity: skip frame limiting when tactical time multiplier is above normal.
        if visual_speed_multiplier > 1.0 {
            return None;
        }

        if !global_use_fps_limit {
            return None;
        }

        // C++ parity: TiVO fast mode disables frame limiting for replay playback.
        if tivo_fast_mode && in_replay_game {
            return None;
        }

        u32::try_from(global_frames_per_second_limit)
            .ok()
            .filter(|fps| *fps > 0)
    }

    fn apply_script_frame_limit(&mut self) {
        let global_data = game_engine::common::global_data::read();
        let max_fps = Self::effective_fps_limit_for_frame(
            self.script_fps_limit,
            global_data.writable.use_fps_limit,
            global_data.writable.frames_per_second_limit,
            self.game_logic.visual_speed_multiplier(),
            global_data.tivo_fast_mode,
            self.game_logic.isInReplayGame(),
        );
        drop(global_data);

        let Some(max_fps) = max_fps else {
            self.script_fps_limit_last_tick = None;
            return;
        };

        // Mirrors C++ GameEngine::execute frame pacing: (1000 / fps) - 1, Sleep(0) loop.
        let limit_ms = (1000.0 / max_fps as f32 - 1.0).max(0.0);
        if limit_ms <= 0.0 {
            self.script_fps_limit_last_tick = Some(Instant::now());
            return;
        }

        let limit = Duration::from_millis(limit_ms as u64);
        if let Some(previous) = self.script_fps_limit_last_tick {
            let mut now = Instant::now();
            while now.duration_since(previous) < limit {
                std::thread::yield_now();
                now = Instant::now();
            }
            self.script_fps_limit_last_tick = Some(now);
        } else {
            self.script_fps_limit_last_tick = Some(Instant::now());
        }
    }

    fn screen_shake_value_for_type(shake_type: i32) -> f32 {
        let data = game_engine::common::global_data::read();
        match shake_type.clamp(0, 5) {
            0 => data.shake_subtle_intensity,
            1 => data.shake_normal_intensity,
            2 => data.shake_strong_intensity,
            3 => data.shake_severe_intensity,
            4 => data.shake_cine_extreme_intensity,
            _ => data.shake_cine_insane_intensity,
        }
    }

    fn enqueue_script_screen_shake(&mut self, intensity: i32) {
        let shake_value = Self::screen_shake_value_for_type(intensity);
        if !shake_value.is_finite() || shake_value <= 0.0 {
            return;
        }

        let seed = self
            .frame_counter
            .wrapping_mul(1_664_525)
            .wrapping_add((intensity as u32).wrapping_mul(1_013_904_223));
        let angle = (seed as f32 / u32::MAX as f32) * TAU;
        self.screen_shake_angle_cos = angle.cos();
        self.screen_shake_angle_sin = angle.sin();

        self.screen_shake_intensity += shake_value;
        let data = game_engine::common::global_data::read();
        if self.screen_shake_intensity > data.max_shake_intensity {
            // C++ parity from W3DView::shake: overflow clamps to fixed 3.0.
            self.screen_shake_intensity = 3.0;
        }
    }

    fn enqueue_script_camera_shaker(&mut self, request: CameraAddShakerRequest) {
        if !request.position.is_finite()
            || !request.amplitude.is_finite()
            || !request.duration_seconds.is_finite()
            || !request.radius.is_finite()
        {
            return;
        }
        if request.duration_seconds <= 0.0 || request.radius <= 0.0 || request.amplitude <= 0.0 {
            return;
        }

        self.script_camera_shakers.push(ScriptCameraShaker::new(
            request.position,
            request.radius,
            request.duration_seconds,
            request.amplitude,
        ));
    }

    fn update_script_camera_shake(&mut self, dt: f32) -> bool {
        let previous = self.camera_shake_offset;
        let mut offset = Vec3::ZERO;

        if self.screen_shake_intensity > 0.01 {
            offset.x += self.screen_shake_intensity * self.screen_shake_angle_cos;
            offset.z += self.screen_shake_intensity * self.screen_shake_angle_sin;
            self.screen_shake_intensity *= 0.75;
            self.screen_shake_angle_cos = -self.screen_shake_angle_cos;
            self.screen_shake_angle_sin = -self.screen_shake_angle_sin;
        } else {
            self.screen_shake_intensity = 0.0;
            self.screen_shake_angle_cos = 0.0;
            self.screen_shake_angle_sin = 0.0;
        }

        if dt > 0.0 {
            for shaker in &mut self.script_camera_shakers {
                shaker.elapsed_seconds += dt.max(0.0);
            }
        }
        self.script_camera_shakers
            .retain(|s| s.elapsed_seconds < s.duration_seconds);

        let camera_position = self.camera_position;
        for shaker in &self.script_camera_shakers {
            let dist = Vec2::new(
                camera_position.x - shaker.epicenter.x,
                camera_position.z - shaker.epicenter.z,
            )
            .length();
            if dist > shaker.radius {
                continue;
            }

            let distance_factor = (1.0 - dist / shaker.radius).clamp(0.0, 1.0);
            let life = (1.0 - shaker.elapsed_seconds / shaker.duration_seconds).clamp(0.0, 1.0);
            let amplitude_world = shaker.amplitude_degrees.to_radians().sin().abs()
                * self.camera_orbit_distance.max(1.0)
                * 0.5;
            let magnitude = amplitude_world * distance_factor * life;
            if magnitude <= f32::EPSILON {
                continue;
            }

            let t = shaker.elapsed_seconds.max(0.0);
            let omega = TAU * shaker.frequency_hz;
            let phase_a = shaker.phase + omega * t;
            let phase_b = shaker.phase * 1.37 + omega * 0.79 * t;

            offset.x += phase_a.sin() * magnitude;
            offset.z += phase_a.cos() * magnitude;
            offset.y += phase_b.sin() * magnitude * 0.2;
        }

        self.camera_shake_offset = offset;
        (self.camera_shake_offset - previous).length_squared() > 0.000001
    }

    fn normalize_signed_angle(mut angle: f32) -> f32 {
        while angle > PI {
            angle -= TAU;
        }
        while angle < -PI {
            angle += TAU;
        }
        angle
    }

    fn apply_camera_look_toward_request(&mut self, request: CameraLookTowardWaypointRequest) {
        let to_target = request.position - self.camera_target;
        let horiz = Vec2::new(to_target.x, to_target.z);
        if horiz.length_squared() <= f32::EPSILON {
            return;
        }

        let target_yaw = to_target.x.atan2(to_target.z);
        let mut delta = Self::normalize_signed_angle(target_yaw - self.camera_yaw_radians);
        if request.reverse_rotation {
            if delta >= 0.0 {
                delta -= TAU;
            } else {
                delta += TAU;
            }
        }
        let target_yaw = self.camera_yaw_radians + delta;

        if request.duration_seconds <= 0.0 {
            self.camera_yaw_radians = target_yaw;
            self.camera_yaw_target = None;
            self.camera_yaw_start = self.camera_yaw_radians;
            self.camera_yaw_duration = 0.0;
            self.camera_yaw_elapsed = 0.0;
            self.camera_yaw_ease_in = 0.0;
            self.camera_yaw_ease_out = 0.0;
            self.apply_camera_orbit_transform();
            return;
        }

        self.camera_yaw_start = self.camera_yaw_radians;
        self.camera_yaw_target = Some(target_yaw);
        self.camera_yaw_duration = request.duration_seconds;
        self.camera_yaw_elapsed = 0.0;
        self.camera_yaw_ease_in = request.ease_in_seconds.max(0.0);
        self.camera_yaw_ease_out = request.ease_out_seconds.max(0.0);
    }

    fn center_camera_on(&mut self, world_pos: Vec3) {
        let clamped = self.clamp_to_world_bounds(world_pos);
        let ground_height = self
            .game_logic
            .terrain_height_at(clamped)
            .unwrap_or(self.camera_target.y);
        self.camera_target.x = clamped.x;
        self.camera_target.y = ground_height;
        self.camera_target.z = clamped.z;
        self.apply_camera_orbit_transform();
    }

    fn issue_minimap_move(&mut self, world_pos: Vec3) {
        if self.selected_objects.is_empty() {
            return;
        }

        let clamped = self.clamp_to_world_bounds(world_pos);
        self.game_logic
            .command_move(self.current_player_id, clamped);
        self.play_sound_effect(SoundType::Command);
    }

    fn clamp_to_world_bounds(&self, mut position: Vec3) -> Vec3 {
        let (world_min, world_max) = self.game_logic.world_bounds();
        position.x = position.x.clamp(world_min.x, world_max.x);
        position.z = position.z.clamp(world_min.z, world_max.z);
        position
    }

    fn drain_renderer_attachments(&mut self) {
        match ww3d_renderer_3d::Renderer::with_global_mut(|renderer| {
            Ok(renderer.take_pending_attachments())
        }) {
            Ok(records) if !records.is_empty() => {
                AttachmentDispatcher::dispatch(records);
            }
            Ok(_) => {}
            Err(err) => {
                warn!("Failed to dispatch WW3D attachments: {err}");
            }
        }
    }

    fn debug_show_victory(&mut self, winner: Option<u32>) {
        info!("Debug: showing victory screen (winner: {:?})", winner);
        self.show_victory_screen(winner);
    }

    fn show_victory_screen(&mut self, winner: Option<u32>) {
        let summary = self.game_logic.build_victory_summary(winner);
        self.victory_summary = Some(summary.clone());
        self.game_paused = true;
        self.match_over = true;
        match winner {
            Some(id) if id == self.current_player_id => {
                self.ui_manager.set_victory_with_summary(id, Some(summary));
            }
            Some(_) => {
                self.ui_manager.set_defeat_with_summary(Some(summary));
            }
            None => {
                self.ui_manager.set_draw_with_summary(Some(summary));
            }
        }
    }

    fn reset_match_state(&mut self) {
        info!("Resetting gameplay state after match completion");
        self.drain_renderer_attachments();

        self.game_logic.reset();
        self.combat_system.clear();
        self.resource_manager = ResourceManager::new();

        let (world_min, world_max) = self.game_logic.world_bounds();
        let world_width = (world_max.x - world_min.x).abs().max(1.0);
        let world_height = (world_max.z - world_min.z).abs().max(1.0);
        self.pathfinding_system =
            PathfindingSystem::new_with_origin(world_min, world_width, world_height);

        self.selected_objects.clear();
        self.keys_pressed.clear();
        self.mouse_position = (0.0, 0.0);
        self.mouse_world_position = Vec3::ZERO;
        self.selection_start = None;
        self.current_player_id = 1;

        for sink in &self.sound_effects {
            sink.stop();
        }
        self.sound_effects.clear();
        if let Some(sink) = self.background_music.take() {
            sink.stop();
        }

        self.match_over = false;
        self.game_paused = false;
        self.victory_summary = None;
        self.ui_manager.clear_victory_screen();
        self.diagnostics_overlay = None;

        self.frame_counter = 0;
        self.fps = 0.0;
        self.last_frame_timing = None;
        self.frame_clock = FrameClock::new();
        NetworkClock::clear_override();

        self.game_hud = GameHUD::new();
        let size = self.window.inner_size();
        self.game_hud.resize(size.width, size.height);

        self.camera_position = Vec3::new(0.0, 200.0, 200.0);
        self.camera_target = Vec3::new(0.0, 0.0, 0.0);
        self.camera_zoom = 1.0;
        self.camera_zoom_target = None;
        self.camera_zoom_start = self.camera_zoom;
        self.camera_zoom_duration = 0.0;
        self.camera_zoom_elapsed = 0.0;
        self.camera_zoom_ease_in = 0.0;
        self.camera_zoom_ease_out = 0.0;
        self.camera_shake_offset = Vec3::ZERO;
        self.screen_shake_intensity = 0.0;
        self.screen_shake_angle_cos = 0.0;
        self.screen_shake_angle_sin = 0.0;
        self.script_camera_shakers.clear();
        self.script_fps_limit = None;
        self.script_fps_limit_last_tick = None;
        self.camera_slave_mode = None;
        self.sync_orbit_from_camera_transform();
        let aspect = size.width.max(1) as f32 / size.height.max(1) as f32;
        self.projection_matrix = Mat4::perspective_rh(
            DEFAULT_VIEW_FOV_RADIANS,
            aspect,
            DEFAULT_VIEW_NEAR_CLIP,
            DEFAULT_VIEW_FAR_CLIP,
        );
    }

    fn exit_to_main_menu_from_victory(&mut self) {
        self.reset_match_state();
        self.ui_manager.transition_to_screen(Screen::MainMenu);
        self.wgpu_ui_system.set_state(UISystemState::MainMenu);
    }

    fn handle_key_press(&mut self, key: &Key) {
        if !matches!(self.current_state, GameState::Playing | GameState::Paused) {
            match key {
                Key::Character(c) if c == "m" || c == "M" => {
                    self.toggle_background_music();
                }
                Key::Named(NamedKey::F11) => {
                    let current_fullscreen = self.window.fullscreen().is_some();
                    if let Err(e) = self.set_fullscreen(!current_fullscreen) {
                        error!("Failed to toggle fullscreen: {}", e);
                    } else {
                        info!("Toggled fullscreen mode: {}", !current_fullscreen);
                    }
                }
                Key::Named(NamedKey::Escape) => {
                    info!("Escape pressed in Menu/Loading - exiting");
                    self.request_state_change(GameState::Exiting);
                }
                _ => {}
            }
            return;
        }

        match key {
            Key::Named(NamedKey::Space) => {
                self.toggle_pause();
            }
            Key::Character(digit)
                if digit.len() == 1 && digit.chars().all(|c| c.is_ascii_digit()) =>
            {
                let group_num = digit.chars().next().unwrap().to_digit(10).unwrap() as u8;
                let ctrl_down = self.keys_pressed.contains(&Key::Named(NamedKey::Control));

                if ctrl_down {
                    // Assign control group.
                    if self.selected_objects.is_empty() {
                        self.control_groups.remove(&group_num);
                        info!("Cleared control group {}", group_num);
                    } else {
                        self.control_groups
                            .insert(group_num, self.selected_objects.clone());
                        info!(
                            "Assigned {} units to control group {}",
                            self.selected_objects.len(),
                            group_num
                        );
                    }
                } else {
                    // Select control group.
                    let stored = self
                        .control_groups
                        .get(&group_num)
                        .cloned()
                        .unwrap_or_default();
                    if stored.is_empty() {
                        info!("Control group {} is empty", group_num);
                        return;
                    }

                    let mut selection = Vec::new();
                    if let Some(player) = self.game_logic.get_player(self.current_player_id) {
                        let team = player.team;
                        for id in stored {
                            if let Some(obj) = self.game_logic.find_object(id) {
                                if obj.team == team && obj.is_selectable() && obj.is_alive() {
                                    selection.push(id);
                                }
                            }
                        }
                    }

                    self.game_logic
                        .select_objects(self.current_player_id, selection.clone());
                    self.selected_objects = selection;
                    self.play_sound_effect(SoundType::Select);
                }
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("a")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Control)) =>
            {
                // Ctrl+A: select all selectable objects for current player team.
                let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                    return;
                };
                let team = player.team;

                let mut selection = Vec::new();
                for (&id, obj) in self.game_logic.get_objects() {
                    if obj.team == team && obj.is_selectable() && obj.is_alive() {
                        selection.push(id);
                    }
                }

                self.game_logic
                    .select_objects(self.current_player_id, selection.clone());
                self.selected_objects = selection;
                self.play_sound_effect(SoundType::Select);
            }
            Key::Named(NamedKey::Delete) => {
                // Debug: delete selected units.
                if self.selected_objects.is_empty() {
                    return;
                }
                for id in self.selected_objects.clone() {
                    self.game_logic.destroy_object(id);
                }
                self.selected_objects.clear();
                self.game_logic
                    .select_objects(self.current_player_id, Vec::new());
            }
            Key::Named(NamedKey::Tab) => {
                // Cycle selection through own selectable objects.
                let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                    return;
                };
                let team = player.team;

                let mut all: Vec<ObjectId> = self
                    .game_logic
                    .get_objects()
                    .iter()
                    .filter(|(_, obj)| obj.team == team && obj.is_selectable() && obj.is_alive())
                    .map(|(&id, _)| id)
                    .collect();
                all.sort_by_key(|id| id.0);
                if all.is_empty() {
                    return;
                }

                let next = if let Some(current) = self.selected_objects.first().copied() {
                    all.iter()
                        .position(|id| *id == current)
                        .map(|idx| all[(idx + 1) % all.len()])
                        .unwrap_or(all[0])
                } else {
                    all[0]
                };

                self.selected_objects = vec![next];
                self.game_logic
                    .select_objects(self.current_player_id, vec![next]);
                self.play_sound_effect(SoundType::Select);
            }
            Key::Named(NamedKey::F1) => {
                self.show_debug_info = !self.show_debug_info;
                info!(
                    "Debug info: {}",
                    if self.show_debug_info { "ON" } else { "OFF" }
                );
            }
            Key::Character(c) if c == "m" || c == "M" => {
                self.toggle_background_music();
            }
            Key::Character(c) if c.eq_ignore_ascii_case("v") => {
                self.debug_show_victory(Some(self.current_player_id));
            }
            Key::Character(c) if c.eq_ignore_ascii_case("l") => {
                let winner = self.game_logic.first_opponent_id(self.current_player_id);
                self.debug_show_victory(winner);
            }
            Key::Character(c) if c.eq_ignore_ascii_case("d") => {
                self.debug_show_victory(None);
            }
            Key::Named(NamedKey::Escape) => {
                info!("Escape key pressed - should exit game");
            }
            Key::Named(NamedKey::F11) => {
                // Toggle fullscreen mode
                let current_fullscreen = self.window.fullscreen().is_some();
                if let Err(e) = self.set_fullscreen(!current_fullscreen) {
                    error!("Failed to toggle fullscreen: {}", e);
                } else {
                    info!("Toggled fullscreen mode: {}", !current_fullscreen);
                }
            }
            _ => {}
        }
    }

    fn handle_left_click(&mut self) {
        self.is_dragging = true;
        self.selection_start = Some(self.mouse_world_position);

        let mouse_pos = self.mouse_world_position;
        let clicked_object = self.find_object_at_position(mouse_pos, &self.game_logic, false);

        if let Some(object_id) = clicked_object {
            // Select this object
            self.game_logic
                .select_objects(self.current_player_id, vec![object_id]);
            self.selected_objects = vec![object_id];
            self.play_sound_effect(SoundType::Select);
        } else {
            // Clear selection
            self.selected_objects.clear();
            self.game_logic
                .select_objects(self.current_player_id, Vec::new());
        }
    }

    fn handle_left_release(&mut self) {
        self.is_dragging = false;

        let Some(start) = self.selection_start.take() else {
            return;
        };

        let end = self.mouse_world_position;

        // If the mouse didn't move enough, the click selection was already handled on mouse-down.
        let drag_distance = Vec2::new(end.x - start.x, end.z - start.z).length();
        if drag_distance < 5.0 {
            return;
        }

        let min_x = start.x.min(end.x);
        let max_x = start.x.max(end.x);
        let min_z = start.z.min(end.z);
        let max_z = start.z.max(end.z);

        let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));

        let mut selection: Vec<ObjectId> = if shift_down {
            self.selected_objects.clone()
        } else {
            Vec::new()
        };

        let Some(player) = self.game_logic.get_player(self.current_player_id) else {
            return;
        };
        let player_team = player.team;

        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != player_team {
                continue;
            }
            if !obj.is_selectable() {
                continue;
            }
            let pos = obj.get_position();
            if pos.x < min_x || pos.x > max_x || pos.z < min_z || pos.z > max_z {
                continue;
            }
            if !selection.contains(&id) {
                selection.push(id);
            }
        }

        self.game_logic
            .select_objects(self.current_player_id, selection.clone());
        self.selected_objects = selection;
        self.play_sound_effect(SoundType::Select);
    }

    fn handle_right_click(&mut self) {
        let mouse_pos = self.mouse_world_position;

        // Normal right-click behavior when no pending command
        if self.selected_objects.is_empty() {
            return;
        }

        let mut should_attack = false;
        let mut attack_target_id = None;

        // Check if clicking on an enemy unit (attack command)
        let target_object = self.find_object_at_position(mouse_pos, &self.game_logic, true);

        if let Some(target_id) = target_object {
            if let Some(target) = self.game_logic.find_object(target_id) {
                // Check if it's an enemy
                if let Some(player) = self.game_logic.get_player(self.current_player_id) {
                    if target.team != player.team && target.is_kind_of(KindOf::Attackable) {
                        should_attack = true;
                        attack_target_id = Some(target_id);
                    }
                }
            }
        }

        // Now handle the command
        if should_attack {
            if let Some(target_id) = attack_target_id {
                self.game_logic
                    .command_attack(self.current_player_id, target_id);
                self.play_sound_effect(SoundType::Command);
            }
        } else {
            // Issue move command to clicked position
            self.game_logic
                .command_move(self.current_player_id, mouse_pos);
            self.play_sound_effect(SoundType::Command);
        }
    }

    fn update_camera(&mut self, dt: f32) {
        let camera_speed = 300.0 * dt;

        let mut movement = Vec3::ZERO;
        if self.camera_slave_mode.is_none() {
            if self.keys_pressed.contains(&Key::Character("w".into())) {
                movement.z -= camera_speed;
            }
            if self.keys_pressed.contains(&Key::Character("s".into())) {
                movement.z += camera_speed;
            }
            if self.keys_pressed.contains(&Key::Character("a".into())) {
                movement.x -= camera_speed;
            }
            if self.keys_pressed.contains(&Key::Character("d".into())) {
                movement.x += camera_speed;
            }
        }

        let mut camera_changed = false;

        if movement.length() > 0.0 {
            self.camera_target += movement;
            camera_changed = true;
        }

        if let Some(mode) = self.camera_slave_mode.as_ref() {
            let target = self
                .game_logic
                .get_objects()
                .values()
                .find(|obj| {
                    obj.is_alive()
                        && obj
                            .template_name
                            .eq_ignore_ascii_case(&mode.thing_template_name)
                })
                .map(|obj| obj.get_position());
            if let Some(target) = target {
                let clamped = self.clamp_to_world_bounds(target);
                if (self.camera_target.x - clamped.x).abs() > 0.001
                    || (self.camera_target.z - clamped.z).abs() > 0.001
                {
                    self.camera_target.x = clamped.x;
                    self.camera_target.z = clamped.z;
                    camera_changed = true;
                }
            }
        }

        if let Some(target) = self.camera_zoom_target {
            if self.camera_zoom_duration <= 0.0 {
                self.camera_zoom = target;
                self.camera_zoom_target = None;
            } else {
                self.camera_zoom_elapsed += dt;
                let t = (self.camera_zoom_elapsed / self.camera_zoom_duration).clamp(0.0, 1.0);
                let eased = Self::parabolic_ease(
                    t,
                    self.camera_zoom_ease_in / self.camera_zoom_duration,
                    self.camera_zoom_ease_out / self.camera_zoom_duration,
                );
                self.camera_zoom =
                    self.camera_zoom_start + (target - self.camera_zoom_start) * eased;
                if t >= 1.0 {
                    self.camera_zoom_target = None;
                }
            }
        }

        if let Some(target) = self.camera_pitch_target {
            if self.camera_pitch_duration <= 0.0 {
                self.camera_pitch_radians = target;
                self.camera_pitch_target = None;
                camera_changed = true;
            } else {
                self.camera_pitch_elapsed += dt;
                let t = (self.camera_pitch_elapsed / self.camera_pitch_duration).clamp(0.0, 1.0);
                let eased = Self::parabolic_ease(
                    t,
                    self.camera_pitch_ease_in / self.camera_pitch_duration,
                    self.camera_pitch_ease_out / self.camera_pitch_duration,
                );
                self.camera_pitch_radians =
                    self.camera_pitch_start + (target - self.camera_pitch_start) * eased;
                camera_changed = true;
                if t >= 1.0 {
                    self.camera_pitch_target = None;
                }
            }
        }

        if let Some(target) = self.camera_yaw_target {
            if self.camera_yaw_duration <= 0.0 {
                self.camera_yaw_radians = target;
                self.camera_yaw_target = None;
                camera_changed = true;
            } else {
                self.camera_yaw_elapsed += dt;
                let t = (self.camera_yaw_elapsed / self.camera_yaw_duration).clamp(0.0, 1.0);
                let eased = Self::parabolic_ease(
                    t,
                    self.camera_yaw_ease_in / self.camera_yaw_duration,
                    self.camera_yaw_ease_out / self.camera_yaw_duration,
                );
                self.camera_yaw_radians =
                    self.camera_yaw_start + (target - self.camera_yaw_start) * eased;
                camera_changed = true;
                if t >= 1.0 {
                    self.camera_yaw_target = None;
                }
            }
        }

        let shake_dt = if self.game_logic.is_time_frozen_for_simulation() {
            0.0
        } else {
            dt
        };
        if self.update_script_camera_shake(shake_dt) {
            camera_changed = true;
        }

        if camera_changed {
            self.apply_camera_orbit_transform();
        }
    }

    fn update_mouse_world_position(&mut self) {
        // Convert screen coordinates to world coordinates using current world bounds.
        // This keeps click mapping stable across different map sizes and resolutions.
        let size = self.window.inner_size();
        let normalized_x = (self.mouse_position.0 / size.width.max(1) as f32).clamp(0.0, 1.0);
        let normalized_y = (self.mouse_position.1 / size.height.max(1) as f32).clamp(0.0, 1.0);

        let (world_min, world_max) = self.game_logic.world_bounds();
        let world_width = (world_max.x - world_min.x).max(1.0);
        let world_height = (world_max.z - world_min.z).max(1.0);
        let world_x = world_min.x + normalized_x * world_width;
        let world_z = world_min.z + normalized_y * world_height;
        self.mouse_world_position = Vec3::new(world_x, 0.0, world_z);
    }

    fn find_object_at_position(
        &self,
        position: Vec3,
        game_logic: &GameLogic,
        command_context: bool,
    ) -> Option<ObjectId> {
        const BASE_SELECTION_RADIUS: f32 = 20.0;

        let player_team = game_logic
            .get_player(self.current_player_id)
            .map(|p| p.team);
        let has_selected_units = !self.selected_objects.is_empty();
        let prioritize_enemy_targets = command_context && has_selected_units;
        let mut best: Option<(ObjectId, u8, f32)> = None; // (id, priority, distance)

        for (&id, obj) in game_logic.get_objects() {
            if !obj.is_alive() {
                continue;
            }

            let distance = obj.get_position().distance(position);
            let radius = BASE_SELECTION_RADIUS.max(obj.selection_radius);
            if distance > radius {
                continue;
            }

            let priority = if prioritize_enemy_targets {
                match player_team {
                    Some(team) if obj.team != team && obj.is_attackable() => 0,
                    Some(team) if obj.team == team && obj.is_selectable() => 1,
                    _ if obj.is_attackable() => 2,
                    _ if obj.is_selectable() => 3,
                    _ => continue,
                }
            } else {
                match player_team {
                    Some(team) if obj.team == team && obj.is_selectable() => 0,
                    Some(_) => continue,
                    None if obj.is_selectable() => 0,
                    None => continue,
                }
            };

            match best {
                Some((_, best_priority, best_distance))
                    if priority > best_priority
                        || (priority == best_priority && distance >= best_distance) => {}
                _ => best = Some((id, priority, distance)),
            }
        }

        best.map(|(id, _, _)| id)
    }

    fn update_unit_pathfinding(&mut self, dt: f32, game_logic: &mut GameLogic) {
        let object_ids: Vec<ObjectId> = game_logic.get_objects().keys().copied().collect();

        for object_id in object_ids {
            // Move units along their paths
            let path_completed = self.pathfinding_system.move_unit_along_path(
                object_id,
                game_logic.get_objects_mut(),
                dt,
            );

            if path_completed {
                // Unit reached destination - could trigger completion events here
            }
        }
    }

    fn render_game_objects<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        // Collect objects to render to avoid borrowing conflicts
        let objects: Vec<_> = self.game_logic.get_objects().values().cloned().collect();
        log::trace!("Rendering {} objects in scene", objects.len());
        for obj in &objects {
            if obj.is_alive() {
                self.render_object(obj, render_pass);
            }
        }
    }

    fn render_object<'a>(&'a self, obj: &Object, render_pass: &mut wgpu::RenderPass<'a>) {
        // Get the correct model name from the template using the new helper method
        let model_name = obj.get_template().get_model_name();

        log::trace!(
            "Render object {} template '{}' model '{}' (cached={})",
            obj.id,
            obj.template_name,
            model_name,
            self.graphics_system.get_model(model_name).is_some()
        );

        // Try both the model name and template name as keys for backwards compatibility
        let w3d_model = self
            .graphics_system
            .get_model(model_name)
            .or_else(|| self.graphics_system.get_model(&obj.template_name));

        // Render proper W3D models from loaded assets
        if let Some(w3d_model) = w3d_model {
            // Calculate total vertices/indices from all meshes
            let total_vertices: usize = w3d_model
                .meshes
                .iter()
                .map(|mesh| mesh.vertices.len())
                .sum();
            let total_indices: usize = w3d_model.meshes.iter().map(|mesh| mesh.indices.len()).sum();

            log::trace!("Rendering W3D model: {} (template: {}) with {} vertices, {} indices across {} meshes",
                model_name, obj.template_name, total_vertices, total_indices, w3d_model.meshes.len());

            // Rendering is now handled by the graphics pipeline system.
            log::trace!("Resolved W3D model '{}' for object {}", model_name, obj.id);
        } else {
            // Keep this non-fatal: templates may reference optional/variant models.
            log::debug!(
                "No W3D model resolved for object {} template '{}' (model '{}')",
                obj.id,
                obj.template_name,
                model_name
            );
        }
    }

    fn render_selection_indicators(&self, _render_pass: &mut wgpu::RenderPass) {
        // Render selection circles around selected objects
        for &object_id in &self.selected_objects {
            if let Some(_obj) = self.game_logic.find_object(object_id) {
                // Render selection circle (simplified)
                // In a full implementation, this would render a proper selection indicator
            }
        }
    }

    fn render_projectiles(&self, _render_pass: &mut wgpu::RenderPass) {
        // Render active projectiles
        for _projectile in self.combat_system.get_projectiles().values() {
            // Render projectile (simplified point for now)
            // In a full implementation, this would render proper projectile models
        }
    }

    fn render_ui(&self, _render_pass: &mut wgpu::RenderPass) {
        if let Err(err) = self.ui_manager.render() {
            log::warn!("UI manager render failed: {}", err);
        }
        log::trace!(
            "UI overlay rendered for {} selected units",
            self.selected_objects.len()
        );
    }

    fn toggle_pause(&mut self) {
        self.game_paused = !self.game_paused;

        self.game_logic.set_paused(self.game_paused);

        info!(
            "Game {}",
            if self.game_paused {
                "PAUSED"
            } else {
                "RESUMED"
            }
        );

        // Notify UI
        self.ui_manager.queue_event(if self.game_paused {
            UIEvent::ChangeScreen(Screen::PauseMenu)
        } else {
            UIEvent::ChangeScreen(Screen::GameHUD)
        });
    }

    fn start_background_music(&mut self) {
        let handle = match &self.audio_handle {
            Some(handle) => handle,
            None => {
                info!("Background music skipped (-noaudio)");
                return;
            }
        };

        let sink = match Sink::try_new(handle) {
            Ok(sink) => sink,
            Err(err) => {
                error!("Failed to create music sink: {err}");
                return;
            }
        };

        // Create ambient RTS music
        let sample_rate = 44_100;
        let duration = 30.0; // 30 second loop
        let samples: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                let base = (t * 220.0 * 2.0 * std::f32::consts::PI).sin() * 0.05;
                let harmony1 = (t * 330.0 * 2.0 * std::f32::consts::PI).sin() * 0.03;
                let harmony2 = (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.02;
                base + harmony1 + harmony2
            })
            .collect();

        let source = rodio::buffer::SamplesBuffer::new(1, sample_rate, samples).repeat_infinite();
        sink.append(source);

        self.background_music = Some(sink);
        info!("Background music started");
    }

    fn toggle_background_music(&mut self) {
        if self.audio_handle.is_none() {
            info!("Background music unavailable (-noaudio)");
            return;
        }

        if let Some(music) = &self.background_music {
            if music.is_paused() {
                music.play();
                info!("Background music resumed");
            } else {
                music.pause();
                info!("Background music paused");
            }
        } else {
            // DISABLED: Using proper AssetManager audio system instead of synthetic tones
            // self.start_background_music();
            info!("Background music would be started, but synthetic audio is disabled");
        }
    }

    fn play_sound_effect(&mut self, sound_type: SoundType) {
        let handle = match &self.audio_handle {
            Some(handle) => handle,
            None => {
                return;
            }
        };

        let sink = match Sink::try_new(handle) {
            Ok(sink) => sink,
            Err(err) => {
                error!("Failed to create sound effect sink: {err}");
                return;
            }
        };

        let (frequency, duration) = match sound_type {
            SoundType::Select => (800.0, 0.1),
            SoundType::Command => (600.0, 0.15),
            SoundType::Hit => (300.0, 0.2),
            SoundType::Explosion => (150.0, 0.5),
            SoundType::Build => (1000.0, 0.3),
        };

        let sample_rate = 44_100;
        let samples: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                let envelope = 1.0 - (t / duration); // Fade out
                (t * frequency * 2.0 * std::f32::consts::PI).sin() * 0.2 * envelope
            })
            .collect();

        let source = rodio::buffer::SamplesBuffer::new(1, sample_rate, samples);
        sink.append(source);
        self.sound_effects.push(sink);
    }

    fn cleanup_sound_effects(&mut self) {
        self.sound_effects.retain(|sink| !sink.empty());
    }

    /// Get or create a texture bind group for a material (delegated to graphics system)
    fn get_material_bind_group(
        &mut self,
        material: &crate::assets::W3DMaterial,
    ) -> Option<&wgpu::BindGroup> {
        // Delegate to graphics system which handles material bind group management
        self.graphics_system.get_material_bind_group(material)
    }

    /// Async texture loading method (for future implementation)
    /// This would be called from a background thread to load textures from BIG archives
    async fn load_texture_async(
        &mut self,
        texture_name: &str,
        material_name: &str,
    ) -> Result<(), String> {
        // Texture loading is now handled by the graphics system
        // This method is kept for future implementation of async texture streaming
        println!(
            "🎨 Async texture loading requested for: {} ({})",
            texture_name, material_name
        );
        println!("   (Currently handled by graphics system material management)");
        Ok(())
    }

    // TEMPORARY: Create a simple colored cube for debugging objects without W3D models
    fn create_fallback_cube(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer, u32) {
        // C++ SAGE compatible cube vertices using VertexFormatXYZNDUV2
        let vertices = vec![
            // Front face
            VertexXYZNDUV2 {
                position: [-2.5, -2.5, 2.5],
                normal: [0.0, 0.0, 1.0],
                diffuse: 0xFF0000FF,
                tex_coords0: [0.0, 0.0],
                tex_coords1: [0.0, 0.0],
            }, // Red
            VertexXYZNDUV2 {
                position: [2.5, -2.5, 2.5],
                normal: [0.0, 0.0, 1.0],
                diffuse: 0xFF00FF00,
                tex_coords0: [1.0, 0.0],
                tex_coords1: [1.0, 0.0],
            }, // Green
            VertexXYZNDUV2 {
                position: [2.5, 2.5, 2.5],
                normal: [0.0, 0.0, 1.0],
                diffuse: 0xFFFF0000,
                tex_coords0: [1.0, 1.0],
                tex_coords1: [1.0, 1.0],
            }, // Blue
            VertexXYZNDUV2 {
                position: [-2.5, 2.5, 2.5],
                normal: [0.0, 0.0, 1.0],
                diffuse: 0xFF00FFFF,
                tex_coords0: [0.0, 1.0],
                tex_coords1: [0.0, 1.0],
            }, // Yellow
            // Back face
            VertexXYZNDUV2 {
                position: [-2.5, -2.5, -2.5],
                normal: [0.0, 0.0, -1.0],
                diffuse: 0xFFFF00FF,
                tex_coords0: [0.0, 0.0],
                tex_coords1: [0.0, 0.0],
            }, // Magenta
            VertexXYZNDUV2 {
                position: [2.5, -2.5, -2.5],
                normal: [0.0, 0.0, -1.0],
                diffuse: 0xFFFFFF00,
                tex_coords0: [1.0, 0.0],
                tex_coords1: [1.0, 0.0],
            }, // Cyan
            VertexXYZNDUV2 {
                position: [2.5, 2.5, -2.5],
                normal: [0.0, 0.0, -1.0],
                diffuse: 0xFFFFFFFF,
                tex_coords0: [1.0, 1.0],
                tex_coords1: [1.0, 1.0],
            }, // White
            VertexXYZNDUV2 {
                position: [-2.5, 2.5, -2.5],
                normal: [0.0, 0.0, -1.0],
                diffuse: 0xFF808080,
                tex_coords0: [0.0, 1.0],
                tex_coords1: [0.0, 1.0],
            }, // Gray
        ];

        let indices: Vec<u16> = vec![
            0, 1, 2, 2, 3, 0, // Front
            4, 5, 6, 6, 7, 4, // Back
            7, 3, 0, 0, 4, 7, // Left
            1, 5, 6, 6, 2, 1, // Right
            3, 2, 6, 6, 7, 3, // Top
            0, 1, 5, 5, 4, 0, // Bottom
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Fallback Cube Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Fallback Cube Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        (vertex_buffer, index_buffer, indices.len() as u32)
    }

    /// C++ SAGE D3D8-style shader - matches original VertexFormatXYZNDUV2 and lighting model
    pub fn get_shader_source() -> &'static str {
        r#"
// C++ SAGE GlobalUniforms equivalent
struct SAGEUniforms {
    view_projection: mat4x4<f32>,
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    camera_position: vec4<f32>,
    time: f32,
    ambient_light: vec3<f32>,
    sun_direction: vec3<f32>,
    sun_color: vec3<f32>,
    _padding: f32,
}

// C++ SAGE MaterialProperties equivalent
struct MaterialProperties {
    diffuse_color: vec4<f32>,
    specular_color: vec4<f32>,
    emissive_color: vec4<f32>,
    opacity: f32,
    shininess: f32,
    stage0_uv_scale: vec2<f32>,
    stage1_uv_scale: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> sage_uniforms: SAGEUniforms;

@group(1) @binding(0)
var stage0_texture: texture_2d<f32>;  // Primary diffuse texture (stage 0)
@group(1) @binding(1)
var stage0_sampler: sampler;

@group(2) @binding(0)
var<uniform> material_properties: MaterialProperties;

// C++ SAGE VertexFormatXYZNDUV2 input - matches D3DVERTEXELEMENT9 declarations
struct VertexInput {
    @location(0) position: vec3<f32>,     // XYZ position
    @location(1) normal: vec3<f32>,       // Normal vector
    @location(2) diffuse: vec4<f32>,      // Diffuse color (unpacked from u32)
    @location(3) tex_coords0: vec2<f32>,  // Primary UV coordinates
    @location(4) tex_coords1: vec2<f32>,  // Secondary UV coordinates
}

// Vertex shader output - matches C++ vertex shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) tex_coords0: vec2<f32>,
    @location(3) tex_coords1: vec2<f32>,
    @location(4) vertex_diffuse: vec4<f32>,
    @location(5) view_direction: vec3<f32>,
}

// C++ SAGE vertex shader - matches D3D8 vertex shader behavior
@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Transform vertex to world space (identity transform for now)
    var world_position = vec4<f32>(input.position, 1.0);
    out.world_position = world_position.xyz;

    // Transform normal to world space
    out.world_normal = normalize(input.normal);

    // Pass through texture coordinates
    out.tex_coords0 = input.tex_coords0;
    out.tex_coords1 = input.tex_coords1;

    // Pass through vertex diffuse color
    out.vertex_diffuse = input.diffuse;

    // Calculate view direction for specular lighting
    out.view_direction = normalize(sage_uniforms.camera_position.xyz - out.world_position);

    // Transform to clip space
    out.clip_position = sage_uniforms.view_projection * world_position;

    return out;
}

// C++ SAGE pixel shader - matches D3D8 pixel shader with C&C lighting model
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample primary texture (stage 0) - matches C++ texture sampling
    var stage0_color = textureSample(stage0_texture, stage0_sampler, input.tex_coords0);

    // Apply material diffuse color to texture - matches C++ VertexMaterialClass behavior
    // In D3D8, materials multiply textures with diffuse color and vertex color
    var tinted_texture = stage0_color * vec4<f32>(material_properties.diffuse_color.rgb, 1.0);

    // Material base color combination - vertex diffuse further modulates the result
    var base_color = tinted_texture * input.vertex_diffuse;

    // C++ SAGE lighting calculations
    var normal = normalize(input.world_normal);
    var light_dir = normalize(sage_uniforms.sun_direction);
    var view_dir = normalize(input.view_direction);

    // Ambient lighting (always present in C&C)
    var ambient = sage_uniforms.ambient_light;

    // Diffuse lighting (Lambertian) - core C&C lighting
    var diffuse_factor = max(dot(normal, -light_dir), 0.0);
    var diffuse = sage_uniforms.sun_color * diffuse_factor;

    // Specular lighting (Phong) - for shiny surfaces like vehicles
    var reflect_dir = reflect(light_dir, normal);
    var specular_factor = pow(max(dot(view_dir, reflect_dir), 0.0), 32.0); // Default shininess
    var specular = sage_uniforms.sun_color * specular_factor * 0.3; // Moderate specular

    // Final lighting combination - matches C++ SAGE lighting model
    var lighting = ambient + diffuse + specular;
    var final_color = vec4<f32>(base_color.rgb * lighting, base_color.a);

    // Ensure minimum visibility (C&C never goes completely black)
    final_color.r = max(final_color.r, 0.1);
    final_color.g = max(final_color.g, 0.1);
    final_color.b = max(final_color.b, 0.1);

    return final_color;
}
"#
    }
}

#[derive(Debug, Clone, Copy)]
enum SoundType {
    Select,
    Command,
    Hit,
    Explosion,
    Build,
}

struct NoopWake;

impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

/// Run the actual C&C game
pub async fn run_cnc_game(
    event_loop: EventLoop<()>,
    window_attributes: WindowAttributes,
    cmd_args: Arc<CommandLineArgs>,
) -> Result<()> {
    info!("🎮 Starting Command & Conquer Generals Zero Hour - Real Game");

    let mut pending_window_attributes = Some(window_attributes);
    let mut window: Option<Arc<Window>> = None;
    let mut pending_engine_window: Option<Arc<Window>> = None;
    let mut engine_init_future: Option<Pin<Box<dyn Future<Output = Result<CnCGameEngine>>>>> =
        None;
    let mut engine_init_started_at: Option<Instant> = None;
    let mut engine_init_last_log_at: Option<Instant> = None;
    let mut engine: Option<CnCGameEngine> = None;
    let mut shutdown_logged = false;
    let mut next_redraw_at = Instant::now();
    let mut last_slow_frame_log = None::<Instant>;
    const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

    #[cfg(feature = "integration-diagnostics")]
    let mut integration_bridge: Option<IntegrationTelemetryBridge> = None;
    #[cfg(feature = "integration-diagnostics")]
    let runtime_handle = tokio::runtime::Handle::current();

    #[allow(deprecated)]
    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::WaitUntil(next_redraw_at));

        let mut drive_frame = |engine: &mut CnCGameEngine, current_window: &Arc<Window>| {
            let frame_started = Instant::now();
            let mut ww3d_elapsed = Duration::ZERO;
            let frame_timing = if matches!(engine.get_state(), GameState::Playing | GameState::Paused)
            {
                let ww3d_started = Instant::now();
                let timing = match ww3d_engine::update() {
                    Ok(_) => match ww3d_engine::timing() {
                        Ok(timing) => {
                            let sync_ms = (timing.total_seconds() * 1000.0)
                                .clamp(0.0, u32::MAX as f32)
                                as u32;
                            WW3D::sync(sync_ms);
                            Some(timing)
                        }
                        Err(err) => {
                            error!("Failed to fetch WW3D frame timing: {err:?}");
                            None
                        }
                    },
                    Err(err) => {
                        error!("WW3D engine update failed: {err:?}");
                        None
                    }
                };
                ww3d_elapsed = ww3d_started.elapsed();
                timing
            } else {
                None
            };

            let update_started = Instant::now();
            if let Some(timing) = frame_timing {
                #[cfg(feature = "integration-diagnostics")]
                if let Some(bridge) = integration_bridge.as_mut() {
                    if let Err(err) = runtime_handle.block_on(bridge.pump_with_timing(engine, timing))
                    {
                        error!(
                            "Integration telemetry pump failed: {err:?}. Disabling bridge."
                        );
                        integration_bridge = None;
                    }
                }
                engine.update_with_timing(&timing);
            } else {
                engine.update_with_frame_clock();
            }
            let update_elapsed = update_started.elapsed();

            let render_started = Instant::now();
            match engine.render() {
                Ok(_) => {}
                Err(e) => {
                    error!("❌ RENDER ERROR: {:?}", e);
                    if let Some(source_err) = e.source() {
                        if let Some(surface_err) = source_err.downcast_ref::<wgpu::SurfaceError>() {
                            match surface_err {
                                wgpu::SurfaceError::Lost => {
                                    error!("🔄 SURFACE LOST: Attempting resize");
                                    engine.resize(current_window.inner_size());
                                }
                                wgpu::SurfaceError::OutOfMemory => {
                                    error!("💥 OUT OF MEMORY: Exiting");
                                    elwt.exit();
                                }
                                _ => {
                                    error!("🚨 Other surface error: {:?}", surface_err);
                                }
                            }
                        } else {
                            error!("🚨 Non-surface error: {:?}", source_err);
                        }
                    } else {
                        error!("🚨 No source error available");
                    }
                }
            }
            let render_elapsed = render_started.elapsed();

            let frame_elapsed = frame_started.elapsed();
            if frame_elapsed >= Duration::from_millis(120) {
                let should_log = last_slow_frame_log
                    .map(|last| frame_started.duration_since(last) >= Duration::from_millis(500))
                    .unwrap_or(true);
                if should_log {
                    warn!(
                        "Slow frame {:?} in {:?} (ww3d={:?}, update={:?}, render={:?}, startup_progress={:.0}%)",
                        frame_elapsed,
                        engine.get_state(),
                        ww3d_elapsed,
                        update_elapsed,
                        render_elapsed,
                        engine.startup_last_reported_progress * 100.0
                    );
                    last_slow_frame_log = Some(frame_started);
                }
            }
        };

        if matches!(event, Event::Resumed) && engine.is_none() {
            let Some(attributes) = pending_window_attributes.take() else {
                error!("Missing window attributes during startup resume");
                elwt.exit();
                return;
            };

            let created_window = match elwt.create_window(attributes) {
                Ok(window) => Arc::new(window),
                Err(err) => {
                    error!("Failed to create window: {err}");
                    elwt.exit();
                    return;
                }
            };

            info!(
                "Window created: {}x{} ({})",
                created_window.inner_size().width,
                created_window.inner_size().height,
                if created_window.fullscreen().is_some() {
                    "Fullscreen"
                } else {
                    "Windowed"
                }
            );

            created_window.set_visible(true);
            created_window.set_minimized(false);
            created_window.focus_window();
            created_window.request_redraw();
            window = Some(created_window.clone());
            pending_engine_window = Some(created_window);
            return;
        }

        if engine.is_none() {
            match event {
                Event::WindowEvent { ref event, window_id } => {
                    if let Some(current_window) = window.as_ref() {
                        if window_id == current_window.id()
                            && matches!(event, WindowEvent::CloseRequested)
                        {
                            info!("Close requested before engine startup completed");
                            elwt.exit();
                            return;
                        }
                    }
                }
                Event::AboutToWait => {
                    if engine_init_future.is_none() {
                        if let Some(created_window) = pending_engine_window.take() {
                            #[cfg(target_os = "windows")]
                            {
                                use raw_window_handle::HasWindowHandle;
                                if let Ok(handle) = created_window.window_handle() {
                                    if let raw_window_handle::RawWindowHandle::Win32(win) =
                                        handle.as_raw()
                                    {
                                        unsafe {
                                            crate::win_main::APPLICATION_WINDOW =
                                                win.hwnd.get() as *mut std::ffi::c_void;
                                        }
                                        debug!("Win32 window handle stored");
                                    }
                                }
                            }

                            engine_init_started_at = Some(Instant::now());
                            engine_init_last_log_at = None;
                            created_window
                                .set_title("Command & Conquer Generals Zero Hour - Initializing");
                            engine_init_future = Some(Box::pin(CnCGameEngine::new(
                                created_window.clone(),
                                cmd_args.clone(),
                            )));
                        }
                    }

                    if let Some(init_future) = engine_init_future.as_mut() {
                        let waker: Waker = Waker::from(Arc::new(NoopWake));
                        let mut cx = Context::from_waker(&waker);
                        match init_future.as_mut().poll(&mut cx) {
                            Poll::Ready(Ok(new_engine)) => {
                                if let Some(created_window) = window.as_ref() {
                                    info!("C&C Game engine initialized successfully!");
                                    created_window.focus_window();
                                    created_window.request_redraw();
                                }
                                engine_init_future = None;
                                engine_init_started_at = None;
                                engine_init_last_log_at = None;
                                engine = Some(new_engine);
                                #[cfg(feature = "integration-diagnostics")]
                                if cmd_args.wants_integration_diagnostics() {
                                    match pollster::block_on(IntegrationTelemetryBridge::new(
                                        IntegrationConfig::default(),
                                    )) {
                                        Ok(bridge) => {
                                            info!("Integration diagnostics bridge initialized");
                                            integration_bridge = Some(bridge);
                                        }
                                        Err(err) => {
                                            error!(
                                                "Failed to initialize integration diagnostics bridge: {err:?}. Continuing without telemetry overlay."
                                            );
                                        }
                                    }
                                }
                            }
                            Poll::Ready(Err(err)) => {
                                error!("Failed to initialize C&C game engine: {err}");
                                engine_init_future = None;
                                elwt.exit();
                            }
                            Poll::Pending => {
                                if let Some(started_at) = engine_init_started_at {
                                    let should_log = engine_init_last_log_at
                                        .map(|last| {
                                            last.elapsed() >= Duration::from_millis(500)
                                        })
                                        .unwrap_or_else(|| started_at.elapsed() >= Duration::from_millis(500));
                                    if should_log {
                                        info!(
                                            "Engine bootstrap still in progress ({:.2}s elapsed)",
                                            started_at.elapsed().as_secs_f32()
                                        );
                                        engine_init_last_log_at = Some(Instant::now());
                                    }
                                }
                            }
                        }
                    }

                    if let Some(created_window) = window.as_ref() {
                        created_window.request_redraw();
                    }
                    elwt.set_control_flow(ControlFlow::WaitUntil(next_redraw_at));
                }
                _ => {}
            }
            return;
        }

        let Some(current_window) = window.as_ref() else {
            return;
        };
        let Some(engine) = engine.as_mut() else {
            return;
        };

        if engine.is_quitting() {
            if !shutdown_logged {
                info!("Engine shutting down");
                shutdown_logged = true;
            }
            elwt.exit();
            return;
        }

        match engine.process_platform_event(&event) {
            Ok(handled) => {
                if handled {
                    return;
                }
            }
            Err(e) => {
                error!("Platform message handling error: {}", e);
            }
        }

        if engine.is_quit_requested() {
            info!("Platform requested quit");
            engine.request_state_change(GameState::Exiting);
            return;
        }

        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == current_window.id() => {
                if !engine.input(event) {
                    match event {
                        WindowEvent::CloseRequested => {
                            info!("Close requested by window");
                            engine.request_state_change(GameState::Exiting);
                        }
                        WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    state: ElementState::Pressed,
                                    logical_key: Key::Named(NamedKey::Escape),
                                    ..
                                },
                            ..
                        } => match engine.get_state() {
                            GameState::Playing => {
                                info!("Escape pressed in Playing state - pausing");
                                engine.request_state_change(GameState::Paused);
                            }
                            GameState::Paused => {
                                info!("Escape pressed in Paused state - resuming");
                                engine.request_state_change(GameState::Playing);
                            }
                            GameState::Menu | GameState::Loading => {
                                info!("Escape pressed in Menu/Loading - exiting");
                                engine.request_state_change(GameState::Exiting);
                            }
                            GameState::Exiting => {}
                        },
                        WindowEvent::Resized(physical_size) => {
                            engine.resize(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { .. } => {
                            // Keep UI/layout hit-testing in sync on HiDPI transitions (macOS).
                            engine.resize(current_window.inner_size());
                        }
                        WindowEvent::RedrawRequested => {
                            drive_frame(engine, current_window);
                        }
                        _ => {}
                    }
                }
            }
            Event::AboutToWait => {
                let now = Instant::now();
                if now >= next_redraw_at {
                    current_window.request_redraw();
                    next_redraw_at = now + FRAME_INTERVAL;
                }
                elwt.set_control_flow(ControlFlow::WaitUntil(next_redraw_at));
            }
            Event::LoopExiting => {
                #[cfg(feature = "integration-diagnostics")]
                if let Some(bridge) = integration_bridge.take() {
                    if let Err(err) = runtime_handle.block_on(bridge.shutdown()) {
                        error!("Failed to shut down integration telemetry bridge: {err:?}");
                    }
                }
            }
            _ => {}
        }
    })?;

    info!("C&C Game ended successfully");
    Ok(())
}

impl CnCGameEngine {
    fn collect_shell_scene_models_for_prewarm(
        game_logic: &GameLogic,
        camera_target: Vec3,
        max_unique_models: usize,
    ) -> VecDeque<String> {
        use std::collections::HashSet;

        let mut candidates: Vec<(f32, String)> = game_logic
            .get_objects()
            .values()
            .filter(|object| object.is_alive())
            .filter_map(|object| {
                let model_name = object.get_template().get_model_name();
                if model_name.is_empty() {
                    return None;
                }

                Some((
                    object.get_position().distance_squared(camera_target),
                    model_name.to_string(),
                ))
            })
            .collect();

        candidates.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.cmp(&b.1))
        });

        let mut selected = Vec::new();
        let mut seen = HashSet::new();
        for (_, model_name) in candidates {
            if seen.insert(model_name.clone()) {
                selected.push(model_name);
            }
            if selected.len() >= max_unique_models {
                break;
            }
        }

        if !selected.is_empty() {
            info!(
                "Queued {} shell-scene models for incremental startup prewarm",
                selected.len()
            );
        }

        selected.into()
    }

    /// Preload all models used by game objects into the graphics system
    async fn preload_all_models(
        graphics_system: &mut GraphicsSystem,
        game_logic: &GameLogic,
    ) -> anyhow::Result<()> {
        use std::collections::HashSet;

        println!("🎨 PRELOAD: Starting model preloading for all game objects...");

        // Collect unique model names from all game objects
        let mut unique_models: HashSet<String> = HashSet::new();

        for (object_id, object) in game_logic.get_objects() {
            if !object.is_alive() {
                continue;
            }

            let model_name = object.get_template().get_model_name();
            unique_models.insert(model_name.to_string());

            // Object uses model (logging disabled)
        }

        println!("📦 Loading {} models...", unique_models.len());

        // Load each model into graphics system
        let mut loaded_count = 0;
        let mut failed_count = 0;
        let total_models = unique_models.len();

        for (index, model_name) in unique_models.iter().enumerate() {
            println!(
                "🎯 Loading model {}/{}: {} (starting...)",
                index + 1,
                total_models,
                model_name
            );

            match Self::load_model_into_graphics_system_blocking(graphics_system, model_name) {
                Ok(true) => {
                    loaded_count += 1;
                    println!(
                        "✅ Model {}/{} loaded successfully",
                        index + 1,
                        total_models
                    );
                }
                Ok(false) => {
                    println!(
                        "⚠️  Model {}/{} already loaded (skipping)",
                        index + 1,
                        total_models
                    );
                }
                Err(e) => {
                    failed_count += 1;
                    eprintln!("❌ Model '{}' failed: {}", model_name, e);
                    eprintln!("   Continuing with next model...");
                }
            }

            println!(
                "📊 Progress: {}/{} models processed, {} loaded, {} failed",
                index + 1,
                total_models,
                loaded_count,
                failed_count
            );

            // Small delay between models like working test
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        println!(
            "✅ Loaded {} models ({} failed)",
            loaded_count, failed_count
        );

        // Preload textures using WW3D Asset Manager definitions from INI files
        // This gets actual texture names from object definitions
        info!("🎨 Preloading textures from WW3D Asset Manager definitions...");
        if let Err(e) = Self::preload_ww3d_textures(graphics_system).await {
            warn!("⚠️ Failed to preload WW3D textures: {}", e);
            // Continue anyway - textures will use fallback
        }

        Ok(())
    }

    /// Preload textures from all cached models using C++ approach - material names as texture files
    async fn preload_model_textures(graphics_system: &mut GraphicsSystem) -> anyhow::Result<()> {
        use std::collections::HashSet;

        log::info!(
            "🎨 TEXTURE: Loading textures using C++ approach - material names as texture filenames"
        );

        // Get all models from graphics system cache and collect material names as texture names
        let mut texture_names: HashSet<String> = HashSet::new();

        // Get all cached models from graphics system
        for (model_name, model) in graphics_system.get_all_models() {
            log::debug!(
                "🔍 TEXTURE: Scanning model '{}' for referenced stage textures...",
                model_name
            );

            Self::collect_material_textures(model, &mut texture_names);

            for mesh in &model.meshes {
                // Direct material reference on mesh (fallback path)
                if let Some(ref tex_name) = mesh.material.texture_name {
                    if Self::is_valid_texture_name(tex_name) {
                        texture_names.insert(tex_name.clone());
                        log::debug!("  📄 Found mesh embedded texture: {}", tex_name);
                    }
                }

                // Authoritative per-pass stage texture names (preferred)
                for (pass_idx, stage_sets) in mesh.per_pass_stage_texture_names.iter().enumerate() {
                    for (stage_idx, names) in stage_sets.iter().enumerate() {
                        let mut stage_populated = false;
                        for texture_name in names {
                            if Self::is_valid_texture_name(texture_name) {
                                texture_names.insert(texture_name.clone());
                                stage_populated = true;
                                log::debug!(
                                    "  📄 Pass {} Stage {} texture: {}",
                                    pass_idx,
                                    stage_idx,
                                    texture_name
                                );
                            }
                        }

                        if !stage_populated {
                            for fallback in mesh.stage_texture_names_from_ids(pass_idx, stage_idx) {
                                if Self::is_valid_texture_name(&fallback) {
                                    texture_names.insert(fallback.clone());
                                    log::debug!(
                                        "  📄 Pass {} Stage {} texture (from IDs): {}",
                                        pass_idx,
                                        stage_idx,
                                        fallback
                                    );
                                }
                            }
                        }
                    }
                }

                if mesh.per_pass_stage_texture_names.is_empty()
                    && !mesh.per_pass_stage_texture_ids.is_empty()
                {
                    for (pass_idx, stages) in mesh.per_pass_stage_texture_ids.iter().enumerate() {
                        for stage_idx in 0..stages.len() {
                            for fallback in mesh.stage_texture_names_from_ids(pass_idx, stage_idx) {
                                if Self::is_valid_texture_name(&fallback) {
                                    texture_names.insert(fallback.clone());
                                    log::debug!(
                                        "  📄 Pass {} Stage {} texture (from IDs): {}",
                                        pass_idx,
                                        stage_idx,
                                        fallback
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        log::info!(
            "🎨 TEXTURE: Found {} unique material-based textures to load",
            texture_names.len()
        );
        log::info!(
            "🎨 TEXTURE: First 10 texture names: {:?}",
            texture_names.iter().take(10).collect::<Vec<_>>()
        );

        if texture_names.is_empty() {
            log::warn!("⚠️  TEXTURE: No material names found - using fallback model name approach");

            // Fallback: Try using model names as texture names (common in C&C)
            for (model_name, _) in graphics_system.get_all_models() {
                if !model_name.is_empty() && model_name != "cube" {
                    texture_names.insert(model_name.clone());
                    log::debug!("  📄 Fallback: Using model name as texture: {}", model_name);
                }
            }

            if texture_names.is_empty() {
                log::warn!("⚠️  TEXTURE: Still no texture candidates - skipping preload");
                return Ok(());
            }
        }

        // Load each texture with improved safety measures
        if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
            let mut loaded_count = 0;
            let mut failed_count = 0;
            let total_textures = texture_names.len().min(20); // Limit to first 20 textures to prevent overwhelming
            let texture_names: Vec<_> = texture_names.iter().take(20).collect();

            log::info!(
                "🎨 TEXTURE: Starting preload of {} textures (limited for safety)",
                total_textures
            );

            for (index, texture_name) in texture_names.iter().enumerate() {
                log::debug!(
                    "🎯 Loading texture {}/{}: {}",
                    index + 1,
                    total_textures,
                    texture_name
                );

                // Shorter timeout to prevent hangs
                let texture_timeout = tokio::time::Duration::from_millis(500);
                let load_result = tokio::time::timeout(texture_timeout, async {
                    // Try to get lock with timeout to prevent deadlocks
                    match asset_manager_arc.try_lock() {
                        Ok(mut asset_manager) => {
                            let _ = asset_manager
                                .load_texture(
                                    graphics_system.device(),
                                    graphics_system.queue(),
                                    texture_name,
                                )
                                .await;
                            true
                        }
                        Err(_) => {
                            log::warn!(
                                "Could not acquire asset manager lock for texture: {}",
                                texture_name
                            );
                            false
                        }
                    }
                })
                .await;

                match load_result {
                    Ok(_) => {
                        loaded_count += 1;
                        log::debug!(
                            "✅ Texture {}/{} loaded: {}",
                            index + 1,
                            total_textures,
                            texture_name
                        );
                    }
                    Err(_) => {
                        failed_count += 1;
                        log::warn!("⏰ Texture '{}' timeout (2s)", texture_name);
                    }
                }

                // Small delay between textures
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }

            log::info!(
                "✅ TEXTURE PRELOAD: Loaded {} textures ({} failed/timeout)",
                loaded_count,
                failed_count
            );
        } else {
            log::error!("❌ TEXTURE PRELOAD: Asset manager not available");
        }

        Ok(())
    }

    fn collect_material_textures(model: &Arc<W3DModel>, texture_names: &mut HashSet<String>) {
        for (material_name, material) in &model.materials {
            if Self::is_valid_texture_name(material_name) {
                texture_names.insert(material_name.clone());
                log::debug!("  📄 Found material-as-texture: {}", material_name);
            }

            if let Some(ref texture_name) = material.texture_name {
                if Self::is_valid_texture_name(texture_name) {
                    texture_names.insert(texture_name.clone());
                    log::debug!("  📄 Found explicit material texture: {}", texture_name);
                }
            }

            for stage_idx in 0..MAX_STAGE_TEXTURES {
                if let Some(stage_texture) = GraphicsSystem::stage_texture_name(material, stage_idx)
                {
                    if Self::is_valid_texture_name(stage_texture) {
                        texture_names.insert(stage_texture.clone());
                        log::debug!(
                            "  📄 Material stage{} texture: {}",
                            stage_idx,
                            stage_texture
                        );
                    }
                }
            }
        }
    }

    fn is_valid_texture_name(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        if name.eq_ignore_ascii_case("default") {
            return false;
        }
        name.parse::<usize>().is_err()
    }

    /// Preload textures using WW3D Asset Manager definitions
    /// This loads textures defined in INI object definitions from INIZH.big
    async fn preload_ww3d_textures(graphics_system: &mut GraphicsSystem) -> anyhow::Result<()> {
        info!("🎨 TEXTURE: Preloading textures from WW3D Asset Manager definitions...");

        if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
            // First, get the list of texture filenames
            let texture_filenames = {
                let asset_manager = asset_manager_arc.lock().unwrap();
                asset_manager.get_all_texture_filenames()
            };

            info!(
                "🎨 TEXTURE: WW3D Asset Manager has {} unique texture filenames to load",
                texture_filenames.len()
            );

            // Show first 20 texture names for debugging
            for (index, name) in texture_filenames.iter().take(20).enumerate() {
                debug!("  📄 Texture {}: {}", index + 1, name);
            }

            if texture_filenames.len() > 20 {
                info!("  ... and {} more textures", texture_filenames.len() - 20);
            }

            // Load ALL textures (matching C++ behavior - no artificial limit)
            let mut loaded_count = 0;
            let mut failed_count = 0;
            let total_to_load = texture_filenames.len(); // Load all textures upfront like C++

            info!(
                "🎨 TEXTURE: Loading ALL {} textures from BIG archives (matching C++ behavior)...",
                total_to_load
            );

            for (index, texture_name) in texture_filenames.iter().enumerate() {
                debug!(
                    "🎯 Loading WW3D texture {}/{}: {}",
                    index + 1,
                    total_to_load,
                    texture_name
                );

                // Try to load the texture with timeout
                let load_future = async {
                    match asset_manager_arc.lock() {
                        Ok(mut asset_manager) => {
                            // Load the texture asynchronously
                            match asset_manager
                                .load_texture_async(
                                    graphics_system.device(),
                                    graphics_system.queue(),
                                    texture_name,
                                )
                                .await
                            {
                                Ok(_) => {
                                    debug!("✅ Loaded texture: {}", texture_name);
                                    true
                                }
                                Err(e) => {
                                    warn!("⚠️ Failed to load texture {}: {}", texture_name, e);
                                    false
                                }
                            }
                        }
                        Err(_) => {
                            warn!("Could not lock asset manager for texture: {}", texture_name);
                            false
                        }
                    }
                };

                match tokio::time::timeout(tokio::time::Duration::from_millis(500), load_future)
                    .await
                {
                    Ok(true) => loaded_count += 1,
                    Ok(false) => failed_count += 1,
                    Err(_) => {
                        failed_count += 1;
                        warn!("⏰ Texture '{}' timeout (500ms)", texture_name);
                    }
                }

                // Small delay between textures
                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            }

            info!(
                "✅ WW3D TEXTURE PRELOAD: Loaded {} textures ({} failed/timeout) from {} available",
                loaded_count,
                failed_count,
                texture_filenames.len()
            );
        } else {
            warn!("⚠️ WW3D TEXTURE PRELOAD: Asset manager not available");
        }

        Ok(())
    }

    /// Load a single model into the graphics system
    fn load_model_into_graphics_system_blocking(
        graphics_system: &mut GraphicsSystem,
        model_name: &str,
    ) -> anyhow::Result<bool> {
        // Check if model is already loaded
        if graphics_system.get_model(model_name).is_some() {
            return Ok(false); // Already loaded
        }

        // Get asset manager and load the model
        if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
            // CRITICAL FIX: Load model in a scope to release asset manager lock before cache_model()
            let w3d_model = {
                let mut asset_manager = asset_manager_arc.lock().unwrap();
                match asset_manager.load_w3d_model_blocking(model_name) {
                    Ok(model) => Ok(model),
                    Err(e) => Err(anyhow::anyhow!(
                        "Failed to load W3D model '{}': {}",
                        model_name,
                        e
                    )),
                }
            }?; // Asset manager lock is released here

            // C++ WW3DAssetManager::Get_Texture() behavior: Load textures BEFORE caching model
            // Extract unique texture names from model meshes
            let mut texture_names: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for mesh in &w3d_model.meshes {
                if let Some(tex_name) = &mesh.material.texture_name {
                    texture_names.insert(tex_name.clone());
                }
            }

            if !texture_names.is_empty() {
                // Get device and queue from graphics system
                let device = graphics_system.device();
                let queue = graphics_system.queue();

                // Load each texture from TexturesZH.big
                let mut loaded_count = 0;
                for tex_name in &texture_names {
                    let mut asset_manager = asset_manager_arc.lock().unwrap();
                    let _loaded_key = asset_manager.load_texture_blocking(device, queue, tex_name);
                    // Verify it's in the cache
                    if asset_manager.get_cached_texture(tex_name).is_some() {
                        loaded_count += 1;
                    }
                    drop(asset_manager); // Release lock between textures
                }
                log::debug!(
                    "Loaded {loaded_count}/{} textures for model '{}'",
                    texture_names.len(),
                    model_name
                );
            }
            // Now cache model without holding the asset manager lock - no deadlock possible
            graphics_system.cache_model(model_name.to_string(), w3d_model);
            Ok(true)
        } else {
            anyhow::bail!("Asset manager not available");
        }
    }

    fn pump_shell_scene_model_prewarm(&mut self) {
        if self.current_state != GameState::Menu {
            return;
        }
        // C++ shell startup does not block first visible frames on synchronous shell-scene model
        // warmup. Gate warmup relative to when the menu actually became active, otherwise a late
        // shell activation can still immediately pre-empt the first visible shell frames.
        let Some(menu_enter_frame) = self.shell_start_frame() else {
            return;
        };
        let startup_age = self
            .current_startup_logic_frame()
            .saturating_sub(menu_enter_frame);
        if startup_age < 10 {
            return;
        }
        let prewarm_budget = if startup_age < 30 {
            1
        } else if startup_age < 60 {
            2
        } else if startup_age < 90 {
            3
        } else if startup_age < 180 {
            4
        } else {
            6
        };
        for _ in 0..prewarm_budget {
            let Some(model_name) = self.pending_shell_model_prewarm.pop_front() else {
                return;
            };

            match Self::load_model_into_graphics_system_blocking(
                &mut self.graphics_system,
                &model_name,
            ) {
                Ok(_) => {}
                Err(err) => {
                    warn!(
                        "Shell incremental prewarm failed for model '{}': {}",
                        model_name, err
                    );
                }
            }
        }
    }
}
