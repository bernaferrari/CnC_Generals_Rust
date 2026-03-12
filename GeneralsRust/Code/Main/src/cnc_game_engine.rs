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
use crate::subsystem_manager::{get_subsystem_manager, with_subsystem_mut, GlobalDataSubsystem};
use crate::ui::{
    DiagnosticsOverlayStats, GameHUD, GameUIState, MinimapActionKind, MinimapInteraction, Screen,
    UIEvent, UIManager, VictoryOverlayAction,
};
use crate::util::profiler::InitTimer;
use ::game_engine::common::frame_clock::{FrameClock, FrameTiming as ClockFrameTiming};
use anyhow::Result;
use egui_winit::winit::{
    self,
    event::{ElementState, Event, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowAttributes},
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
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use wgpu::util::DeviceExt;
use ww3d_core::ww3d::WW3D;
use ww3d_engine::{self, EngineConfig, EngineError, FrameTiming};
use ww3d_renderer_3d::core::error::Error as RendererError;

#[cfg(feature = "game_client")]
use game_client::gui::{
    get_shell, set_ui_renderer, with_ui_renderer, with_window_manager, with_window_manager_ref,
};
#[cfg(feature = "game_client")]
use game_client::gui::ui_renderer::UIRenderer as LegacyUIRenderer;
#[cfg(feature = "game_client")]
use game_client::core::SubsystemInterface as GameClientSubsystemInterface;
#[cfg(feature = "network")]
use game_network::time::NetworkClock;

#[cfg(not(feature = "network"))]
struct NetworkClock;

#[cfg(not(feature = "network"))]
impl NetworkClock {
    fn override_with_duration(_duration: Duration) {}
    fn clear_override() {}
}

const DEFAULT_SKIRMISH_MAP: &str = "Defcon6";
const LEGACY_VIEW_FOV_RADIANS: f32 = 50.0_f32.to_radians();
const LEGACY_VIEW_NEAR_CLIP: f32 = 1.0;

fn pack_legacy_mouse_data(x: i32, y: i32) -> u32 {
    ((y as u32) << 16) | ((x as u32) & 0xFFFF)
}
const LEGACY_VIEW_FAR_CLIP: f32 = 20_000.0;

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
    diagnostics_overlay: Option<DiagnosticsOverlayStats>,

    // UI system
    ui_manager: UIManager,
    game_hud: GameHUD,

    // Egui integration for GUI rendering
    egui_context: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: Arc<Mutex<egui_wgpu::Renderer>>,
    egui_hud: crate::ui::EguiHUD,
    active_menu_shell_hook: Option<&'static str>,

    // Model loading state
    models_loaded: bool,
    pending_shell_model_prewarm: VecDeque<String>,
    menu_enter_frame: Option<u64>,
    legacy_shell_enqueued_frame: Option<u64>,
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

impl CnCGameEngine {
    #[cfg(feature = "game_client")]
    fn legacy_shell_active(&self) -> bool {
        let shell = get_shell();
        shell.is_shell_active() && shell.get_screen_count() > 0
    }

    #[cfg(not(feature = "game_client"))]
    fn legacy_shell_active(&self) -> bool {
        false
    }

    fn legacy_shell_owns_menu_ui(&self) -> bool {
        self.current_state == GameState::Menu && self.legacy_shell_active()
    }

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
        self.game_logic.get_current_frame()
    }

    fn shell_start_frame(&self) -> Option<u64> {
        match (self.menu_enter_frame, self.legacy_shell_enqueued_frame) {
            (Some(menu), Some(legacy)) => Some(menu.min(legacy)),
            (Some(menu), None) => Some(menu),
            (None, Some(legacy)) => Some(legacy),
            (None, None) => None,
        }
    }

    fn configured_startup_camera_defaults() -> StartupCameraDefaults {
        if let Some(defaults) =
            crate::subsystem_manager::with_subsystem::<GlobalDataSubsystem, _>(|subsystem| {
                subsystem.get_global_data().map(|global| StartupCameraDefaults {
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
        let (legacy_ground_height, terrain_height_max) =
            Self::sample_startup_camera_heights(game_logic, terrain_target, world_center.y);
        let focus_target = Vec3::new(focus_2d.x, 0.0, focus_2d.y);
        let (focus_ground_height, _) =
            Self::sample_startup_camera_heights(game_logic, focus_target, world_center.y);

        // Keep the C++ zoom/offset sampling from the legacy top-left anchor, but aim the modern
        // Rust camera at the requested scene focus. This remains the closest visible match for the
        // current renderer bridge.
        let camera_target = Vec3::new(focus_2d.x, focus_ground_height, focus_2d.y);
        let camera_offset_z = legacy_ground_height + defaults.camera_height.max(0.0);
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
            legacy_ground_height,
            terrain_height_max,
            defaults,
            1.0,
        );

        // Match W3DView::buildCameraTransform when angle/pitch defaults are zero:
        // source = cameraOffset * zoom; source *= (1 - ground / source.z); then translate.
        let source_z = camera_offset_z * zoom;
        let factor = if source_z.abs() > f32::EPSILON {
            1.0 - (legacy_ground_height / source_z)
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
            legacy_ground_height,
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

    fn compute_default_camera_zoom_for_target(
        &self,
        target: Vec3,
        max_height_scale: f32,
    ) -> f32 {
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
        if !matches!(self.current_state, GameState::Menu | GameState::Loading) {
            return;
        }

        #[cfg(feature = "game_client")]
        let (shell_active, shell_screen_count) = {
            let shell = get_shell();
            (shell.is_shell_active(), shell.get_screen_count())
        };
        #[cfg(not(feature = "game_client"))]
        let (shell_active, shell_screen_count) = (false, 0usize);

        let (
            terrain_chunk_count,
            terrain_heightmap_loaded,
            terrain_total_chunks,
            terrain_visible_chunks,
            terrain_renderable_chunks,
            terrain_pending_chunks,
            terrain_world_size,
            terrain_summary,
        ) = game_client::terrain::terrain_visual::get_terrain_visual()
            .ok()
            .and_then(|guard| {
                guard.as_ref().map(|terrain| {
                    (
                        terrain.chunk_draw_count(),
                        terrain.debug_heightmap_loaded(),
                        terrain.debug_total_chunk_count(),
                        terrain.debug_visible_chunk_count(),
                        terrain.debug_renderable_visible_chunk_count(),
                        terrain.debug_pending_visible_chunk_count(),
                        terrain.world_size(),
                        terrain.debug_chunk_summary(),
                    )
                })
            })
            .unwrap_or_else(|| {
                (
                    0,
                    false,
                    0,
                    0,
                    0,
                    0,
                    (0.0, 0.0),
                    "terrain_visual_unavailable".to_string(),
                )
            });
        let mut nearest_objects = self
            .game_logic
            .get_objects()
            .iter()
            .map(|(id, object)| {
                let distance_sq = object.get_position().distance_squared(self.camera_target);
                (distance_sq, id, object)
            })
            .collect::<Vec<_>>();
        nearest_objects.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(b.1)));
        let view_projection = self.projection_matrix * self.view_matrix;
        let sample_object_ids = nearest_objects
            .iter()
            .take(8)
            .map(|(_, id, _)| **id)
            .collect::<Vec<_>>();
        let sample_objects = nearest_objects
            .into_iter()
            .take(8)
            .map(|(_, id, object)| {
                let position = object.get_position();
                let render_position = gameplay_to_render_transform(object.get_transform_matrix())
                    .w_axis
                    .truncate();
                let clip = view_projection * render_position.extend(1.0);
                let ndc = if clip.w.abs() > f32::EPSILON {
                    clip.truncate() / clip.w
                } else {
                    Vec3::splat(f32::INFINITY)
                };
                format!(
                    "{}:{} pos=({:.1},{:.1},{:.1}) dist={:.1} ndc=({:.2},{:.2},{:.2}) hp={:.1}/{:.1} destroyed={} constructed={} model={}",
                    id,
                    object.template_name,
                    position.x,
                    position.y,
                    position.z,
                    render_position.distance(self.camera_target),
                    ndc.x,
                    ndc.y,
                    ndc.z,
                    object.health.current,
                    object.max_health,
                    object.status.destroyed,
                    !object.status.under_construction,
                    object.get_template().get_model_name(),
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
        let (
            render_shadow_items,
            render_forward_opaque_items,
            render_forward_transparent_items,
            render_water_items,
            render_ui_items,
        ) = self.render_pipeline.debug_render_pass_counts();
        let (forward_draw_calls, forward_meshes_rendered, forward_triangles_rendered) =
            self.render_pipeline.debug_forward_renderer_stats();
        let sample_object_render_items = self
            .render_pipeline
            .debug_render_item_breakdown_for_objects(&sample_object_ids);
        let sample_model_summaries = sample_object_ids
            .iter()
            .filter_map(|id| self.game_logic.get_objects().get(id).map(|obj| obj.get_template().get_model_name()))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter_map(|model_name| self.graphics_system.debug_model_summary(model_name))
            .collect::<Vec<_>>()
            .join(" | ");
        let logic_frame = self.current_startup_logic_frame();
        let startup_frame = self.shell_start_frame().unwrap_or(logic_frame);
        let startup_age = logic_frame.saturating_sub(startup_frame);
        let contents = format!(
            "state={:?}\ngame_mode={:?}\nframe_counter={}\nstartup_frame={}\nstartup_age={}\nmap_loaded={}\nshell_active={}\nshell_screen_count={}\nobject_count={}\nrender_alive_objects={}\nrender_fow_filtered={}\nrender_model_missing={}\nrender_model_budget_skips={}\nrender_zero_mesh_models={}\nrender_missing_model_samples={}\nrender_deferred_model_load_budget={}\nrender_deferred_model_loads={}\nmap={}\ncamera_position={:?}\ncamera_target={:?}\ncamera_zoom={:.3}\ncamera_pitch_rad={:.6}\ncamera_yaw_rad={:.6}\nrender_items_last_frame={}\nrender_shadow_items={}\nrender_forward_opaque_items={}\nrender_forward_transparent_items={}\nrender_water_items={}\nrender_ui_items={}\nforward_draw_calls={}\nforward_meshes_rendered={}\nforward_triangles_rendered={}\nterrain_chunks={}\nterrain_heightmap_loaded={}\nterrain_total_chunks={}\nterrain_visible_chunks={}\nterrain_renderable_chunks={}\nterrain_pending_chunks={}\nterrain_world_size={:?}\nterrain_summary={}\nsample_objects={}\nsample_object_render_items={}\nsample_model_summaries={}\nwindow_size={:?}\n",
            self.current_state,
            self.game_logic.game_mode(),
            self.frame_counter,
            startup_frame,
            startup_age,
            self.game_logic.isInGame(),
            shell_active,
            shell_screen_count,
            self.game_logic.get_objects().len(),
            self.render_pipeline.debug_last_alive_objects(),
            self.render_pipeline.debug_last_fow_filtered(),
            self.render_pipeline.debug_last_model_missing(),
            self.render_pipeline.debug_last_model_budget_skips(),
            self.render_pipeline.debug_last_zero_mesh_models(),
            self.render_pipeline.debug_last_missing_model_samples().join(" | "),
            self.render_pipeline.debug_last_deferred_model_load_budget(),
            self.render_pipeline.debug_last_deferred_model_loads(),
            self.game_logic.get_current_map_name(),
            self.camera_position,
            self.camera_target,
            self.camera_zoom,
            self.camera_pitch_radians,
            self.camera_yaw_radians,
            self.render_pipeline.debug_render_item_count(),
            render_shadow_items,
            render_forward_opaque_items,
            render_forward_transparent_items,
            render_water_items,
            render_ui_items,
            forward_draw_calls,
            forward_meshes_rendered,
            forward_triangles_rendered,
            terrain_chunk_count,
            terrain_heightmap_loaded,
            terrain_total_chunks,
            terrain_visible_chunks,
            terrain_renderable_chunks,
            terrain_pending_chunks,
            terrain_world_size,
            terrain_summary,
            sample_objects,
            sample_object_render_items,
            sample_model_summaries,
            self.window.inner_size(),
        );
        if let Err(err) = fs::write("/tmp/generals_startup_state.txt", &contents) {
            warn!("Failed to write /tmp/generals_startup_state.txt: {err}");
        }
        println!(
            "DEBUG_STARTUP_STATE: state={:?} game_mode={:?} frame={} startup_age={} map={} camera_position={:?} camera_target={:?} camera_zoom={:.3} render_items_last_frame={} deferred_budget={} deferred_loads={} budget_skips={} zero_mesh_models={} terrain_visible_chunks={} terrain_renderable_chunks={} terrain_pending_chunks={} terrain_summary={}",
            self.current_state,
            self.game_logic.game_mode(),
            self.frame_counter,
            startup_age,
            self.game_logic.get_current_map_name(),
            self.camera_position,
            self.camera_target,
            self.camera_zoom,
            self.render_pipeline.debug_render_item_count(),
            self.render_pipeline.debug_last_deferred_model_load_budget(),
            self.render_pipeline.debug_last_deferred_model_loads(),
            self.render_pipeline.debug_last_model_budget_skips(),
            self.render_pipeline.debug_last_zero_mesh_models(),
            terrain_visible_chunks,
            terrain_renderable_chunks,
            terrain_pending_chunks,
            terrain_summary,
        );
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

        // Initialize mesh renderer
        // Initialize egui for GUI rendering
        info!("🎨 Initializing egui GUI system...");
        let egui_context = egui::Context::default();

        // Create egui-winit state for input handling
        let egui_state = egui_winit::State::new(
            egui_context.clone(),
            egui::ViewportId::default(),
            &window,
            None, // No scale factor override
            None, // No max texture side override
            None, // No max texture layers override
        );

        // Create egui-wgpu renderer for GPU rendering
        let egui_renderer = Arc::new(Mutex::new(egui_wgpu::Renderer::new(
            graphics_system.device(),
            graphics_system.color_format(),
            egui_wgpu::RendererOptions::default(),
        )));

        // Create egui HUD instance
        let egui_hud = crate::ui::EguiHUD::new();

        info!("✅ Egui GUI system initialized");

        // Initialize asset manager using graphics system's render device
        info!("🎨 Initializing C&C Asset Manager for loading real W3D models and textures...");
        let asset_duration = {
            let asset_timer = InitTimer::new("✅ Enhanced Asset Manager initialized");
            crate::assets::manager::init_asset_manager(
                graphics_system.device(),
                graphics_system.queue(),
            )
            .await?;
            asset_timer.finish()
        };

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

        {
            let mut renderer_guard = egui_renderer
                .lock()
                .map_err(|_| anyhow::anyhow!("egui renderer mutex poisoned"))?;
            render_pipeline.ensure_minimap_texture_registered(&mut renderer_guard)?;
        }

        let startup_camera_defaults = Self::configured_startup_camera_defaults();
        let mut camera_target = Vec3::ZERO;
        let mut camera_position = Vec3::new(0.0, 310.0, -403.99988);
        let mut camera_zoom = 1.0;
        let projection_matrix = Mat4::perspective_rh(
            LEGACY_VIEW_FOV_RADIANS,
            size.width as f32 / size.height as f32,
            LEGACY_VIEW_NEAR_CLIP,
            LEGACY_VIEW_FAR_CLIP,
        );

        // TEMPORARY: Create fallback cube for debugging objects without W3D models
        let (fallback_cube_vertex_buffer, fallback_cube_index_buffer, fallback_cube_index_count) =
            Self::create_fallback_cube(graphics_system.device());

        let start_in_menu = !command_line.quick_start && command_line.map_name.is_none();
        let startup_shell_map = start_in_menu.then(Self::configured_startup_shell_map).flatten();

        // Start a new game. The C++ startup shell uses GAME_SHELL for the shell map.
        game_logic.start_new_game(if start_in_menu {
            GameMode::Shell
        } else {
            GameMode::Skirmish
        });
        let cli_map = command_line.map_name.as_deref();
        let map_to_load = cli_map
            .map(str::to_string)
            .or(startup_shell_map)
            .unwrap_or_else(|| DEFAULT_SKIRMISH_MAP.to_string());
        let mut loaded_map_name: Option<String> = None;
        if !game_logic.load_map(&map_to_load) {
            warn!(
                "Failed to load map '{}', falling back to default map '{}'",
                map_to_load, DEFAULT_SKIRMISH_MAP
            );
            if map_to_load != DEFAULT_SKIRMISH_MAP && game_logic.load_map(DEFAULT_SKIRMISH_MAP) {
                loaded_map_name = Some(DEFAULT_SKIRMISH_MAP.to_string());
            }
        } else {
            loaded_map_name = Some(map_to_load.clone());
        }

        if let Some(active_map_name) = loaded_map_name {
            if cli_map.is_some() {
                info!("Loaded map from command line: {}", active_map_name);
            } else if start_in_menu {
                info!("Loaded startup shell map: {}", active_map_name);
            }
            Self::apply_heightmap_hint(&mut render_pipeline, &game_logic);
            Self::apply_skybox_hint(&mut render_pipeline, &game_logic);
            // Refresh minimap to reflect actual map bounds.
            Self::reinitialize_minimap_renderer(
                &mut render_pipeline,
                &graphics_system,
                &egui_renderer,
                &mut game_logic,
            )?;
            Self::apply_map_lighting(&mut graphics_system, &mut render_pipeline, &game_logic);
            (camera_target, camera_position, camera_zoom) =
                Self::bootstrap_camera_for_loaded_map(&game_logic, 0, startup_camera_defaults);
        }

        let camera_offset = camera_position - camera_target;
        let camera_orbit_distance = camera_offset.length().max(1.0);
        let camera_pitch_radians = camera_offset
            .y
            .atan2(Vec2::new(camera_offset.x, camera_offset.z).length());
        let camera_yaw_radians = camera_offset.x.atan2(camera_offset.z);
        let view_matrix = Mat4::look_at_rh(camera_position, camera_target, Vec3::Y);

        if let Some(player_name) = command_line.player_name.as_deref() {
            if game_logic.set_player_name(0, player_name) {
                info!("Set local player name to '{}'", player_name);
            } else {
                warn!("Failed to apply player name '{}'", player_name);
            }
        }

        let pending_shell_model_prewarm = if start_in_menu {
            Self::collect_shell_scene_models_for_prewarm(&game_logic, camera_target, 24)
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
        #[cfg(feature = "game_client")]
        if start_in_menu {
            ui_manager.suspend_for_legacy_shell();
        }

        #[cfg(feature = "game_client")]
        {
            let loaded_strings = game_client::game_text::GameText::init_runtime_strings()
                .map_err(|err| anyhow::anyhow!("legacy shell GameText init failed: {err}"))?;
            let sample_back = game_client::game_text::GameText::fetch("GUI:Back");
            eprintln!(
                "DEBUG_STARTUP_GAMETEXT: loaded_strings={} sample_back={:?}",
                loaded_strings, sample_back
            );
            game_engine::common::ini::ini_mapped_image::ImageCollection::load_global(512);
            let imported = game_client::display::image::sync_mapped_images_from_common();
            eprintln!("DEBUG_STARTUP_MAPPED_IMPORT: imported={imported}");

            with_window_manager(|manager| {
                manager.reset();
                manager.set_screen_size(size.width as i32, size.height as i32);
                manager.init();
            });

            if with_ui_renderer(|_| ()).is_none() {
                let renderer = LegacyUIRenderer::new(
                    graphics_system.device_arc(),
                    graphics_system.queue_arc(),
                    graphics_system.color_format(),
                )
                .map_err(|err| anyhow::anyhow!("legacy UI renderer init failed: {err}"))?;
                set_ui_renderer(Arc::new(std::sync::RwLock::new(renderer)));
            }

            if start_in_menu {
                let mut shell = get_shell();
                shell
                    .init()
                    .map_err(|err| anyhow::anyhow!("shell init failed: {err}"))?;
                shell
                    .show_shell(true)
                    .map_err(|err| anyhow::anyhow!("shell show failed: {err}"))?;
                println!(
                    "DEBUG_STARTUP: shell_initialized active={} screens={}",
                    shell.is_shell_active(),
                    shell.get_screen_count()
                );
                ui_manager.suspend_for_legacy_shell();
            }
        }

        println!(
            "DEBUG_STARTUP: quick_start={} map_name={:?} start_in_menu={}",
            command_line.quick_start, command_line.map_name, start_in_menu
        );
        let initial_state = if start_in_menu {
            GameState::Menu
        } else {
            GameState::Loading
        };
        let pending_state = if start_in_menu {
            None
        } else {
            Some(GameState::Playing)
        };

        let engine = Self {
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
            diagnostics_overlay: None,
            ui_manager,
            game_hud: GameHUD::new(),
            egui_context,
            egui_state,
            egui_renderer,
            egui_hud,
            active_menu_shell_hook: None,
            models_loaded: true, // Already loaded during init
            pending_shell_model_prewarm,
            menu_enter_frame: if start_in_menu { Some(0) } else { None },
            legacy_shell_enqueued_frame: None,
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
        info!("   Asset Manager: {:.2}s", asset_duration.as_secs_f32());
        info!("🎮 Controls:");
        info!("  WASD - Move camera");
        info!("  Mouse - Select units");
        info!("  Right click - Move/Attack command");
        info!("  SPACE - Pause game");
        info!("  F1 - Toggle debug info");
        info!("  M - Toggle music");
        info!("  ESC - Exit game");

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
                LEGACY_VIEW_FOV_RADIANS,
                new_size.width as f32 / new_size.height as f32,
                LEGACY_VIEW_NEAR_CLIP,
                LEGACY_VIEW_FAR_CLIP,
            );
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
                let legacy_shell_menu = self.legacy_shell_owns_menu_ui();
                match state {
                    ElementState::Pressed => {
                        self.keys_pressed.insert(key.clone());
                        #[cfg(feature = "game_client")]
                        if legacy_shell_menu {
                            // TODO(ui-refactor): this is the legacy shell input bridge. Future
                            // UI frontends should replace the renderer, not these menu semantics.
                            if let Key::Character(text) = key {
                                if let Some(byte) = text.bytes().next() {
                                    with_window_manager(|manager| {
                                        let _ = manager.process_key_event(byte, 1);
                                    });
                                }
                            }
                            return true;
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
                let legacy_shell_menu = self.legacy_shell_owns_menu_ui();
                #[cfg(feature = "game_client")]
                if legacy_shell_menu {
                    let (x, y) = (self.mouse_position.0 as i32, self.mouse_position.1 as i32);
                    let packed = pack_legacy_mouse_data(x, y);
                    let msg = match (button, state) {
                        (MouseButton::Left, ElementState::Pressed) => Some(game_client::gui::game_window::WindowMessage::LeftDown),
                        (MouseButton::Left, ElementState::Released) => Some(game_client::gui::game_window::WindowMessage::LeftUp),
                        (MouseButton::Right, ElementState::Pressed) => Some(game_client::gui::game_window::WindowMessage::RightDown),
                        (MouseButton::Right, ElementState::Released) => Some(game_client::gui::game_window::WindowMessage::RightUp),
                        (MouseButton::Middle, ElementState::Pressed) => Some(game_client::gui::game_window::WindowMessage::MiddleDown),
                        (MouseButton::Middle, ElementState::Released) => Some(game_client::gui::game_window::WindowMessage::MiddleUp),
                        _ => None,
                    };
                    if let Some(msg) = msg {
                        with_window_manager(|manager| {
                            let _ = manager.process_mouse_event(msg, x, y, packed);
                        });
                    }
                    return true;
                }
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
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_position = (position.x as f32, position.y as f32);
                self.update_mouse_world_position();
                #[cfg(feature = "game_client")]
                if self.legacy_shell_owns_menu_ui() {
                    let packed = pack_legacy_mouse_data(position.x as i32, position.y as i32);
                    with_window_manager(|manager| {
                        let _ = manager.process_mouse_event(
                            game_client::gui::game_window::WindowMessage::MousePos,
                            position.x as i32,
                            position.y as i32,
                            packed,
                        );
                    });
                    return true;
                }
                true
            }
            _ => false,
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
        // Menu/loading progression must advance on a fixed engine tick like the C++ main loop.
        // Using wall-clock microdeltas here can keep accumulated simulation time below the
        // fixed-step threshold forever when redraws arrive back-to-back during startup.
        let clock_timing = self
            .frame_clock
            .advance_fixed(Duration::from_secs_f32(1.0 / 60.0));
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
        self.apply_script_frame_limit();
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
                if self.legacy_shell_active() {
                    self.ui_manager.suspend_for_legacy_shell();
                } else {
                    self.ui_manager
                        .transition_to_screen(crate::ui::Screen::MainMenu);
                }
            }
            GameState::Loading => {
                info!("Entering Loading state");
                // Show loading screen, prepare assets
                self.ui_manager
                    .transition_to_screen(crate::ui::Screen::Loading);
            }
            GameState::Playing => {
                info!("Entering Playing state");
                // Start game logic, enable input
                self.game_paused = false;
                self.game_logic.set_paused(false);
                self.ui_manager
                    .transition_to_screen(crate::ui::Screen::GameHUD);
            }
            GameState::Paused => {
                info!("Entering Paused state");
                // Freeze game logic, show pause menu
                self.game_paused = true;
                self.game_logic.set_paused(true);
                self.ui_manager
                    .transition_to_screen(crate::ui::Screen::PauseMenu);
            }
            GameState::Exiting => {
                info!("Entering Exiting state - beginning shutdown");
                // Cleanup will happen in drop
            }
        }

        self.current_state = new_state;
        if new_state == GameState::Menu {
            self.menu_enter_frame = Some(self.current_startup_logic_frame());
        } else {
            self.menu_enter_frame = None;
            self.legacy_shell_enqueued_frame = None;
            self.active_menu_shell_hook = None;
        }
    }

    /// Check if engine should quit
    /// Matches C++ GameEngine::isQuitting()
    pub fn is_quitting(&self) -> bool {
        self.current_state == GameState::Exiting
    }

    fn update_internal(&mut self, dt: f32) {
        // Process any pending state transitions first
        self.process_state_transitions();

        // Early exit if we're shutting down
        if self.is_quitting() {
            return;
        }

        let dt = dt.max(0.0);
        let visual_dt = dt * self.game_logic.visual_speed_multiplier().max(0.0);
        let mut defer_shell_update = false;

        // State-based update logic - matches C++ GameEngine::update() conditional updates
        match self.current_state {
            GameState::Menu => {
                // The C++ shell keeps the background shell-map game active while menus are up.
                self.cleanup_sound_effects();
                with_window_manager(|manager| manager.update());
                if self.legacy_shell_owns_menu_ui() {
                    self.ui_manager.suspend_for_legacy_shell();
                } else {
                    if let Err(err) = self.ui_manager.update(dt) {
                        warn!("UI manager update failed in menu state: {}", err);
                    }
                    self.egui_hud.update(dt);
                }
                // TODO(ui-refactor): keep legacy shell/window progression isolated here so a
                // future frontend can swap rendering without changing shell behavior.
                defer_shell_update = true;
            }
            GameState::Loading => {
                // In loading: minimal updates, mainly for loading screen animations
                if let Err(err) = self.ui_manager.update(dt) {
                    warn!("UI manager update failed in loading state: {}", err);
                }
                self.egui_hud.update(dt);
                // After loading completes, the state will transition to Playing
                // This is handled by the initialization code setting pending_state
                return;
            }
            GameState::Paused => {
                // In paused: update UI and camera, but not game logic
                // (matches C++ where TheGameLogic->isGamePaused() prevents update)
                self.update_camera(visual_dt);
                self.cleanup_sound_effects();
                if let Err(err) = self.ui_manager.update(dt) {
                    warn!("UI manager update failed in paused state: {}", err);
                }
                self.egui_hud.update(dt);
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

        // Keep shell-scene streaming warm while the legacy menu is active. With startup age now
        // driven by the advancing logic frame, this no longer stalls forever on frame 1.
        if self.current_state == GameState::Menu {
            self.pump_shell_scene_model_prewarm();
        }

        // Full update cycle for Playing state (matches C++ GameEngine::update())

        // Update subsystems first (matches C++ update order: Radar, Audio, Client, Network, CD)
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

        // Game logic update (matches C++ condition: Network==NULL || Network->isFrameDataReady())
        if !self.game_paused {
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

        // Update egui HUD with delta time for animations
        self.egui_hud.update(dt);

        // Clear lingering beacon glow if the logic reports no active beacons.
        if self.game_logic.beacon_count() == 0 {
            self.egui_hud.clear_beacon_highlights();
        }

        // Process UI-generated commands from egui HUD
        if self.egui_hud.has_commands() {
            let ui_commands = self.egui_hud.take_commands();
            for cmd in ui_commands {
                // UI command is already a GameCommand, just queue it directly
                self.game_logic.queue_command(cmd);
                println!("[Engine] Queued UI command to game logic");
            }
        }

        if defer_shell_update {
            if let Err(err) = get_shell().update() {
                warn!("Shell update failed in menu state: {}", err);
            }
        }

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

        if !self.match_over {
            if let Some(condition) = self.game_logic.evaluate_victory_condition() {
                match condition {
                    VictoryCondition::Winner(id) => self.show_victory_screen(Some(id)),
                    VictoryCondition::Draw => self.show_victory_screen(None),
                }
            }
        }

        // Update UI system (kept for legacy compatibility)
        // Note: Legacy UI system will be phased out in favor of egui
        // self.ui_manager.update(dt).unwrap_or_else(|e| {
        //     error!("UI Manager update failed: {}", e);
        // });
        // self.game_hud.update(dt).unwrap_or_else(|e| {
        //     error!("Game HUD update failed: {}", e);
        // });
    }

    pub fn render(&mut self) -> Result<()> {
        // Begin egui frame - collect UI input and prepare for rendering
        let raw_input = self.egui_state.take_egui_input(self.window.as_ref());
        self.egui_context.begin_pass(raw_input);

        let legacy_shell_ui_rendered = match self.current_state {
            GameState::Menu => self.render_legacy_shell_overlay(),
            _ => false,
        };
        let shell_menu_active =
            matches!(self.current_state, GameState::Menu) && legacy_shell_ui_rendered;

        // Keep the legacy shell/menu isolated from the in-game egui HUD.
        let mut ui_state = if shell_menu_active {
            GameUIState::default()
        } else {
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
            ui_state
        };

        match self.current_state {
            GameState::Menu if !legacy_shell_ui_rendered => self.render_main_menu_overlay(),
            GameState::Menu => {}
            GameState::Loading => self.render_loading_overlay(),
            _ => self.egui_hud.render(&self.egui_context, &ui_state),
        }

        if let Some(action) = self.egui_hud.take_victory_action() {
            match action {
                VictoryOverlayAction::ExitToMenu => self.exit_to_main_menu_from_victory(),
            }
        }

        if !shell_menu_active {
            if let Some(rect) = self.egui_hud.minimap_rect() {
                self.render_pipeline.update_minimap_screen_rect(
                    Vec2::new(rect.min.x, rect.min.y),
                    Vec2::new(rect.width(), rect.height()),
                );
            }

            if let Some(interaction) = self.egui_hud.take_minimap_interaction() {
                self.handle_minimap_interaction(interaction);
            }
        }

        // End egui frame and get the shapes to render
        let full_output = self.egui_context.end_pass();

        // Handle platform output (cursor changes, IME events, etc.)
        self.egui_state
            .handle_platform_output(self.window.as_ref(), full_output.platform_output);

        // Prepare egui rendering data
        // Handle potential first-frame issues with tessellation
        let paint_jobs = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.egui_context
                .tessellate(full_output.shapes, self.window.scale_factor() as f32)
        })) {
            Ok(jobs) => jobs,
            Err(_) => {
                warn!("egui tessellation panicked (first frame issue), skipping UI render");
                Vec::new() // Empty paint jobs = no UI to render
            }
        };

        static LOGGED_MENU_PAINT: AtomicBool = AtomicBool::new(false);
        static LOGGED_LOADING_PAINT: AtomicBool = AtomicBool::new(false);
        match self.current_state {
            GameState::Menu if !LOGGED_MENU_PAINT.swap(true, Ordering::Relaxed) => {
                println!(
                    "DEBUG_STARTUP: menu_overlay_paint_jobs={}",
                    paint_jobs.len()
                );
            }
            GameState::Loading if !LOGGED_LOADING_PAINT.swap(true, Ordering::Relaxed) => {
                println!(
                    "DEBUG_STARTUP: loading_overlay_paint_jobs={}",
                    paint_jobs.len()
                );
            }
            _ => {}
        }

        // Convert viewport dimensions for egui
        let window_size = self.window.inner_size();
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [window_size.width, window_size.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        // Update egui textures if needed - safely handle first frame
        for (texture_id, image_delta) in &full_output.textures_delta.set {
            if let Err(_) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if let Ok(mut renderer) = self.egui_renderer.lock() {
                    renderer.update_texture(
                        self.graphics_system.device(),
                        self.graphics_system.queue(),
                        *texture_id,
                        image_delta,
                    );
                }
            })) {
                warn!("egui texture update panicked, skipping texture update");
            }
        }

        if !paint_jobs.is_empty() {
            let egui_renderer = Arc::clone(&self.egui_renderer);
            let paint_jobs_for_pass = paint_jobs.clone();
            let screen_descriptor_for_pass = screen_descriptor;
            self.render_pipeline
                .enqueue_post_frame_callback(move |frame| {
                    static LOGGED_EGUI_CALLBACK: AtomicBool = AtomicBool::new(false);
                    if !LOGGED_EGUI_CALLBACK.swap(true, Ordering::Relaxed) {
                        println!(
                            "DEBUG_STARTUP: egui_post_frame_callback paint_jobs={}",
                            paint_jobs_for_pass.len()
                        );
                    }
                    let mut egui_renderer = egui_renderer.lock().map_err(|_| {
                        RendererError::InvalidOperation("egui renderer poisoned".into())
                    })?;
                    let device = ww3d_engine::device().map_err(|err| {
                        RendererError::RenderError(format!(
                            "failed to acquire WW3D device for egui overlay: {err:?}"
                        ))
                    })?;
                    let queue = ww3d_engine::queue().map_err(|err| {
                        RendererError::RenderError(format!(
                            "failed to acquire WW3D queue for egui overlay: {err:?}"
                        ))
                    })?;
                    let prep_command_buffers = egui_renderer.update_buffers(
                        device.as_ref(),
                        queue.as_ref(),
                        frame.encoder(),
                        &paint_jobs_for_pass,
                        &screen_descriptor_for_pass,
                    );
                    if !prep_command_buffers.is_empty() {
                        warn!(
                            "egui produced {} auxiliary command buffers that are not yet submitted in the WW3D callback path",
                            prep_command_buffers.len()
                        );
                    }
                    let color_view = frame.color_view_arc();
                    let encoder = frame.encoder();
                    let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("egui overlay"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: color_view.as_ref(),
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    let mut render_pass: wgpu::RenderPass<'static> =
                        unsafe { std::mem::transmute(render_pass) };
                    egui_renderer.render(
                        &mut render_pass,
                        &paint_jobs_for_pass,
                        &screen_descriptor_for_pass,
                    );
                    Ok(())
                });
        }

        // Execute the main game render pipeline using the WW3D frame
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
        let deferred_startup_model_load_budget =
            if matches!(self.current_state, GameState::Menu | GameState::Loading) {
                if let Some(menu_enter_frame) = self.shell_start_frame() {
                    let shell_startup_age =
                        self.current_startup_logic_frame().saturating_sub(menu_enter_frame);
                    if shell_startup_age <= 5 {
                        2
                    } else if shell_startup_age <= 15 {
                        4
                    } else if shell_startup_age <= 30 {
                        8
                    } else if shell_startup_age <= 180 {
                        12
                    } else {
                        16
                    }
                } else {
                    0
                }
            } else {
                0
            };
        if let Err(err) = self.render_pipeline.execute(
            &mut self.graphics_system,
            &self.game_logic,
            &self.view_matrix,
            &self.projection_matrix,
            self.camera_position,
            render_time_delta,
            allow_sync_model_loads,
            deferred_startup_model_load_budget,
        ) {
            return Err(err);
        };

        static CAPTURED_STARTUP_FRAME: AtomicBool = AtomicBool::new(false);
        static CAPTURED_SETTLED_SHELL_FRAME: AtomicBool = AtomicBool::new(false);
        static LAST_STARTUP_DEBUG_STAGE: AtomicU32 = AtomicU32::new(0);
        if self.current_state == GameState::Menu
            && !CAPTURED_STARTUP_FRAME.swap(true, Ordering::Relaxed)
        {
            if let Err(err) = ww3d_engine::make_screenshot("/tmp/generals_internal_frame.png") {
                warn!("Failed to queue internal startup screenshot: {err:?}");
            } else {
                println!("DEBUG_STARTUP: queued_internal_screenshot=/tmp/generals_internal_frame.png");
            }
        }
        let startup_logic_frame = self.current_startup_logic_frame();
        if self.current_state == GameState::Menu
            && startup_logic_frame >= 30
            && !CAPTURED_SETTLED_SHELL_FRAME.swap(true, Ordering::Relaxed)
        {
            if let Err(err) =
                ww3d_engine::make_screenshot("/tmp/generals_internal_frame_settled.png")
            {
                warn!("Failed to queue settled shell screenshot: {err:?}");
            } else {
                println!(
                    "DEBUG_STARTUP: queued_settled_screenshot=/tmp/generals_internal_frame_settled.png"
                );
            }
        }

        let startup_debug_stage = if self.current_state == GameState::Menu {
            if startup_logic_frame >= 60 {
                4
            } else if startup_logic_frame >= 30 {
                3
            } else if startup_logic_frame >= 10 {
                2
            } else {
                1
            }
        } else {
            1
        };
        if matches!(self.current_state, GameState::Menu | GameState::Loading) {
            if startup_logic_frame <= 60 {
                self.write_startup_debug_state();
            } else if startup_debug_stage > LAST_STARTUP_DEBUG_STAGE.load(Ordering::Relaxed) {
                LAST_STARTUP_DEBUG_STAGE.store(startup_debug_stage, Ordering::Relaxed);
                self.write_startup_debug_state();
            }
        }

        self.drain_renderer_attachments();

        // Clean up textures that are no longer needed
        for texture_id in &full_output.textures_delta.free {
            if let Ok(mut renderer) = self.egui_renderer.lock() {
                renderer.free_texture(texture_id);
            }
        }

        Ok(())
    }

    fn render_main_menu_overlay(&mut self) {
        fn signal_shell_hook(hook: &str) {
            gamelogic::helpers::TheScriptEngine::signal_ui_interact(hook);
        }

        let current_map_name = self.game_logic.get_current_map_name().to_string();
        let mut start_skirmish = false;
        let mut load_last_map = false;
        let mut exit_requested = false;
        let mut hovered_shell_hook: Option<&'static str> = None;
        let mut selected_shell_hook: Option<&'static str> = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
            .show(&self.egui_context, |ui| {
                let rect = ui.max_rect();
                let painter = ui.painter_at(rect);
                let rule_color = egui::Color32::from_rgba_premultiplied(167, 134, 94, 230);
                let rule_shadow = egui::Color32::from_rgba_premultiplied(38, 30, 21, 210);
                let panel_fill = egui::Color32::from_rgba_premultiplied(0, 0, 0, 126);
                let panel_stroke = egui::Color32::from_rgb(47, 55, 168);
                let glow_stroke = egui::Color32::from_rgba_premultiplied(210, 253, 4, 210);
                let screen_w = rect.width();
                let screen_h = rect.height();

                let line = |painter: &egui::Painter,
                            a: egui::Pos2,
                            b: egui::Pos2,
                            width: f32,
                            color: egui::Color32| {
                    painter.line_segment([a, b], egui::Stroke::new(width, color));
                };

                let top_y = rect.top();
                let top_inner_y = rect.top() + screen_h * 0.10;
                let bottom_inner_y = rect.top() + screen_h * 0.90;
                let bottom_y = rect.bottom();
                let verticals = [0.225_f32, 0.445_f32, 0.6662_f32, 0.885_f32];

                line(
                    &painter,
                    egui::pos2(rect.left(), top_y),
                    egui::pos2(rect.right(), top_y),
                    2.0,
                    rule_color,
                );
                line(
                    &painter,
                    egui::pos2(rect.left(), top_y + 1.0),
                    egui::pos2(rect.right(), top_y + 1.0),
                    2.0,
                    rule_shadow,
                );
                line(
                    &painter,
                    egui::pos2(rect.left(), top_inner_y),
                    egui::pos2(rect.right(), top_inner_y),
                    1.0,
                    rule_color,
                );
                line(
                    &painter,
                    egui::pos2(rect.left(), top_inner_y + screen_h * 0.02),
                    egui::pos2(rect.right(), top_inner_y + screen_h * 0.02),
                    1.0,
                    rule_shadow,
                );
                line(
                    &painter,
                    egui::pos2(rect.left(), bottom_inner_y),
                    egui::pos2(rect.right(), bottom_inner_y),
                    1.0,
                    rule_color,
                );
                line(
                    &painter,
                    egui::pos2(rect.left(), bottom_inner_y + screen_h * 0.02),
                    egui::pos2(rect.right(), bottom_inner_y + screen_h * 0.02),
                    1.0,
                    rule_shadow,
                );
                line(
                    &painter,
                    egui::pos2(rect.left(), bottom_y),
                    egui::pos2(rect.right(), bottom_y),
                    2.0,
                    rule_color,
                );
                line(
                    &painter,
                    egui::pos2(rect.left(), bottom_y + 1.0),
                    egui::pos2(rect.right(), bottom_y + 1.0),
                    2.0,
                    rule_shadow,
                );

                for fraction in verticals {
                    let x = rect.left() + screen_w * fraction;
                    line(
                        &painter,
                        egui::pos2(x, rect.top()),
                        egui::pos2(x, rect.bottom()),
                        3.0,
                        rule_color,
                    );
                }

                let pulse_width = (screen_w * 0.17).max(96.0);
                let pulse_height = (screen_h * 0.033).max(12.0);
                let time = ui.ctx().input(|i| i.time) as f32;
                let pulse_phase = (time / 10.0).fract();
                let forward = ((time / 10.0).floor() as i32) % 2 == 0;
                let pulse_x = if forward {
                    rect.left() - pulse_width + (screen_w + pulse_width) * pulse_phase
                } else {
                    rect.right() - (screen_w + pulse_width) * pulse_phase
                };
                let pulse_y = if forward {
                    rect.top() - pulse_height * 0.5
                } else {
                    rect.bottom() - pulse_height * 0.5
                };
                let pulse_rect = egui::Rect::from_min_size(
                    egui::pos2(pulse_x, pulse_y),
                    egui::vec2(pulse_width, pulse_height),
                );
                painter.rect_filled(
                    pulse_rect,
                    2.0,
                    egui::Color32::from_rgba_premultiplied(218, 197, 135, 72),
                );

                let right_panel = egui::Rect::from_min_max(
                    egui::pos2(rect.left() + screen_w * 0.665, rect.top() + screen_h * 0.18),
                    egui::pos2(rect.left() + screen_w * 0.945, rect.top() + screen_h * 0.40),
                );
                painter.rect_filled(right_panel, 0.0, panel_fill);
                painter.rect_stroke(
                    right_panel,
                    0.0,
                    egui::Stroke::new(1.0, panel_stroke),
                    egui::StrokeKind::Inside,
                );

                let title_pos = egui::pos2(
                    rect.left() + screen_w * 0.075,
                    rect.top() + screen_h * 0.12,
                );
                painter.text(
                    title_pos,
                    egui::Align2::LEFT_TOP,
                    "COMMAND & CONQUER\nGENERALS ZERO HOUR",
                    egui::FontId::proportional((screen_h * 0.05).clamp(18.0, 34.0)),
                    egui::Color32::from_rgb(233, 228, 210),
                );

                let subtitle_pos = egui::pos2(
                    rect.left() + screen_w * 0.075,
                    rect.top() + screen_h * 0.27,
                );
                painter.text(
                    subtitle_pos,
                    egui::Align2::LEFT_TOP,
                    format!("Shell map: {}", current_map_name),
                    egui::FontId::proportional((screen_h * 0.022).clamp(12.0, 16.0)),
                    egui::Color32::from_rgb(190, 180, 160),
                );

                let button_width = (screen_w * 0.23).clamp(180.0, 240.0);
                let button_height = (screen_h * 0.06).clamp(28.0, 38.0);
                let button_x = rect.left() + screen_w * 0.675;
                let mut button_y = rect.top() + screen_h * 0.205;
                let spacing = button_height + screen_h * 0.014;

                let shell_button = |ui: &mut egui::Ui,
                                    rect: egui::Rect,
                                    label: &str,
                                    accent: bool| {
                    let response = ui.put(
                        rect,
                        egui::Button::new(
                            egui::RichText::new(label)
                                .size((screen_h * 0.027).clamp(14.0, 18.0))
                                .color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgba_premultiplied(14, 18, 42, 170))
                        .stroke(egui::Stroke::new(
                            1.0,
                            if accent { glow_stroke } else { panel_stroke },
                        )),
                    );
                    response
                };

                let single_player_rect = egui::Rect::from_min_size(
                    egui::pos2(button_x, button_y),
                    egui::vec2(button_width, button_height),
                );
                let single_player_response =
                    shell_button(ui, single_player_rect, "Single Player", true);
                if single_player_response.hovered() {
                    hovered_shell_hook = Some("ShellMainMenuSkirmishHighlighted");
                }
                if single_player_response.clicked() {
                    start_skirmish = true;
                    selected_shell_hook = Some("ShellMainMenuSkirmishPushed");
                }
                button_y += spacing;

                let load_map_rect = egui::Rect::from_min_size(
                    egui::pos2(button_x, button_y),
                    egui::vec2(button_width, button_height),
                );
                if shell_button(ui, load_map_rect, "Load Last Map", false).clicked() {
                    load_last_map = true;
                }
                button_y += spacing;

                let exit_rect = egui::Rect::from_min_size(
                    egui::pos2(button_x, button_y),
                    egui::vec2(button_width, button_height),
                );
                let exit_response = shell_button(ui, exit_rect, "Exit", false);
                if exit_response.hovered() {
                    hovered_shell_hook = Some("ShellMainMenuExitHighlighted");
                }
                if exit_response.clicked() {
                    exit_requested = true;
                    selected_shell_hook = Some("ShellMainMenuExitPushed");
                }
            });

        let next_hovered_hook = hovered_shell_hook;
        if self.active_menu_shell_hook != next_hovered_hook {
            if let Some(previous) = self.active_menu_shell_hook.take() {
                let unhighlight_hook = match previous {
                    "ShellMainMenuSkirmishHighlighted" => Some("ShellMainMenuSkirmishUnhighlighted"),
                    "ShellMainMenuExitHighlighted" => Some("ShellMainMenuExitUnhighlighted"),
                    "ShellMainMenuOptionsHighlighted" => Some("ShellMainMenuOptionsUnhighlighted"),
                    "ShellMainMenuNetworkHighlighted" => Some("ShellMainMenuNetworkUnhighlighted"),
                    "ShellMainMenuOnlineHighlighted" => Some("ShellMainMenuOnlineUnhighlighted"),
                    _ => None,
                };
                if let Some(hook) = unhighlight_hook {
                    signal_shell_hook(hook);
                }
            }
            if let Some(hook) = next_hovered_hook {
                signal_shell_hook(hook);
            }
            self.active_menu_shell_hook = next_hovered_hook;
        }

        if let Some(hook) = selected_shell_hook {
            signal_shell_hook(hook);
        }

        if start_skirmish {
            self.start_game_from_ui(
                GameMode::Skirmish,
                "USA".to_string(),
                DEFAULT_SKIRMISH_MAP.to_string(),
            );
            self.request_state_change(GameState::Playing);
        }

        if load_last_map {
            self.start_game_from_ui(
                GameMode::Skirmish,
                "USA".to_string(),
                current_map_name,
            );
            self.request_state_change(GameState::Playing);
        }

        if exit_requested {
            self.request_state_change(GameState::Exiting);
        }
    }

    #[cfg(feature = "game_client")]
    fn render_legacy_shell_overlay(&mut self) -> bool {
        static LOGGED_WINDOW_TEXTS: AtomicBool = AtomicBool::new(false);
        static LOGGED_WINDOW_DRAWS: AtomicBool = AtomicBool::new(false);
        let shell_active = {
            let shell = get_shell();
            shell.is_shell_active() && shell.get_screen_count() > 0
        };
        if !shell_active {
            static LOGGED_NO_SHELL: AtomicBool = AtomicBool::new(false);
            if !LOGGED_NO_SHELL.swap(true, Ordering::Relaxed) {
                eprintln!("DEBUG_STARTUP_LEGACY_SHELL: inactive");
            }
            return false;
        }

        let Some(renderer_arc) = with_ui_renderer(|renderer| renderer.clone()) else {
            static LOGGED_NO_RENDERER: AtomicBool = AtomicBool::new(false);
            if !LOGGED_NO_RENDERER.swap(true, Ordering::Relaxed) {
                eprintln!("DEBUG_STARTUP_LEGACY_SHELL: ui_renderer_missing");
            }
            return false;
        };

        let window_size = self.window.inner_size();
        let ui_time = self.frame_counter as f32 / 30.0;

        {
            let Ok(mut renderer) = renderer_arc.write() else {
                return false;
            };
            renderer.begin_frame();
            renderer.set_time(ui_time);
            renderer.set_screen_size(window_size.width.max(1), window_size.height.max(1));
        }

        with_window_manager(|manager| manager.draw_all());
        if !LOGGED_WINDOW_TEXTS.swap(true, Ordering::Relaxed) {
            with_window_manager_ref(|manager| {
                for (name, text, text_label, hidden, parent_name) in
                    manager.debug_collect_window_texts_by_prefix("MainMenu.wnd:")
                {
                    eprintln!(
                        "DEBUG_SHELL_WINDOW_TEXT: name={name} hidden={hidden} parent={parent_name:?} text={text:?} text_label={text_label:?}"
                    );
                }
            });
        }
        if !LOGGED_WINDOW_DRAWS.swap(true, Ordering::Relaxed) {
            with_window_manager_ref(|manager| {
                for (name, hidden, pos, size, parent_name, image_name) in
                    manager.debug_collect_window_draws_by_prefix("MainMenu.wnd:")
                {
                    if hidden {
                        continue;
                    }
                    let area = (size.0.max(0) as i64) * (size.1.max(0) as i64);
                    if area < 20_000 {
                        continue;
                    }
                    eprintln!(
                        "DEBUG_SHELL_WINDOW_DRAW: name={name} pos={:?} size={:?} parent={parent_name:?} image={image_name:?}",
                        pos, size
                    );
                }
            });
        }

        self.render_pipeline
            .enqueue_post_frame_callback(move |frame| {
                let mut renderer = renderer_arc.write().map_err(|_| {
                    RendererError::InvalidOperation("legacy UI renderer poisoned".into())
                })?;
                let color_view = frame.color_view_arc();
                let encoder = frame.encoder();
                let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("legacy shell ui overlay"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: color_view.as_ref(),
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                let mut render_pass: wgpu::RenderPass<'static> =
                    unsafe { std::mem::transmute(render_pass) };
                let result = renderer.render(&mut render_pass);
                renderer.end_frame();
                result.map_err(|err| {
                    RendererError::RenderError(format!(
                        "legacy shell ui render failed: {err}"
                    ))
                })
            });
        self.legacy_shell_enqueued_frame
            .get_or_insert(self.current_startup_logic_frame());
        static LOGGED_LEGACY_OK: AtomicBool = AtomicBool::new(false);
        if !LOGGED_LEGACY_OK.swap(true, Ordering::Relaxed) {
            eprintln!("DEBUG_STARTUP_LEGACY_SHELL: enqueued");
        }
        true
    }

    #[cfg(not(feature = "game_client"))]
    fn render_legacy_shell_overlay(&mut self) -> bool {
        false
    }

    fn render_loading_overlay(&mut self) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
            .show(&self.egui_context, |ui| {
                let rect = ui.max_rect();
                let painter = ui.painter_at(rect);
                let panel_rect = egui::Rect::from_min_max(
                    egui::pos2(rect.left() + rect.width() * 0.665, rect.top() + rect.height() * 0.18),
                    egui::pos2(rect.left() + rect.width() * 0.945, rect.top() + rect.height() * 0.31),
                );
                painter.rect_filled(
                    panel_rect,
                    0.0,
                    egui::Color32::from_rgba_premultiplied(0, 0, 0, 126),
                );
                painter.rect_stroke(
                    panel_rect,
                    0.0,
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(47, 55, 168)),
                    egui::StrokeKind::Inside,
                );

                ui.scope_builder(egui::UiBuilder::new().max_rect(panel_rect.shrink(16.0)), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(6.0);
                        ui.heading("Loading");
                        ui.add_space(10.0);
                        ui.spinner();
                        ui.add_space(10.0);
                        ui.label("Preparing game systems and assets...");
                    });
                });
            });
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

        let rect = egui::Rect::from_min_max(
            egui::Pos2::new(min_x.clamp(0.0, 1.0), min_y.clamp(0.0, 1.0)),
            egui::Pos2::new(max_x.clamp(0.0, 1.0), max_y.clamp(0.0, 1.0)),
        );

        ui_state.minimap_viewport = rect;
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
                }
                UIEvent::ExitGame => {
                    info!("UI requested exit");
                    self.request_state_change(GameState::Exiting);
                }
                UIEvent::ChangeScreen(screen) => {
                    self.ui_manager.transition_to_screen(screen);
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
                    &self.egui_renderer,
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
            &self.egui_renderer,
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
        (self.camera_target, self.camera_position, self.camera_zoom) = Self::bootstrap_camera_for_loaded_map(
            &self.game_logic,
            self.current_player_id,
            startup_camera_defaults,
        );
        self.sync_orbit_from_camera_transform();
        self.ui_manager.transition_to_screen(Screen::GameHUD);
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
        egui_renderer: &Arc<Mutex<egui_wgpu::Renderer>>,
        game_logic: &mut GameLogic,
    ) -> anyhow::Result<()> {
        let mut world_bounds = game_logic.world_bounds();
        render_pipeline.initialize_minimap_renderer(
            graphics_system.device_arc(),
            graphics_system.queue_arc(),
            world_bounds,
        )?;
        let mut renderer_guard = egui_renderer
            .lock()
            .map_err(|_| anyhow::anyhow!("egui renderer mutex poisoned"))?;
        render_pipeline.ensure_minimap_texture_registered(&mut renderer_guard)?;
        render_pipeline
            .load_heightmap_from_hint(
                &graphics_system.device_arc(),
                &graphics_system.queue_arc(),
                Some(world_bounds),
            )
            .map_err(|e| anyhow::anyhow!("heightmap bridge: {e}"))?;

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

    fn apply_script_frame_limit(&mut self) {
        let Some(max_fps) = self.script_fps_limit else {
            self.script_fps_limit_last_tick = None;
            return;
        };

        if max_fps == 0 {
            self.script_fps_limit_last_tick = None;
            return;
        }

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
        static DEBUG_CENTER_CAMERA_LOGS: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        if DEBUG_CENTER_CAMERA_LOGS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 24 {
            eprintln!(
                "DEBUG_SHELL_CAMERA_APPLY: frame={} requested={world_pos:?} clamped={clamped:?} old_target={:?} new_ground_height={ground_height:.3}",
                self.frame_counter,
                self.camera_target,
            );
        }
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
        self.egui_hud.reset_match_state();
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
            LEGACY_VIEW_FOV_RADIANS,
            aspect,
            LEGACY_VIEW_NEAR_CLIP,
            LEGACY_VIEW_FAR_CLIP,
        );
    }

    fn exit_to_main_menu_from_victory(&mut self) {
        self.reset_match_state();
        self.ui_manager.transition_to_screen(Screen::MainMenu);
    }

    fn handle_key_press(&mut self, key: &Key) {
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
        let selected_units = self.selected_objects.clone();

        // Check if we have a pending command from UI buttons
        if self.egui_hud.has_pending_command() {
            // Check if clicking on an object
            let target_object = self.find_object_at_position(mouse_pos, &self.game_logic, true);

            if let Some(target_id) = target_object {
                // Complete pending command with object target
                self.egui_hud
                    .complete_pending_command_with_object(target_id, selected_units);
            } else {
                // Complete pending command with position target
                self.egui_hud
                    .complete_pending_command_with_position(mouse_pos, selected_units);
            }

            self.play_sound_effect(SoundType::Command);
            return;
        }

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

/// Run the actual C&C game
pub async fn run_cnc_game(
    event_loop: EventLoop<()>,
    window_attributes: WindowAttributes,
    cmd_args: Arc<CommandLineArgs>,
) -> Result<()> {
    info!("🎮 Starting Command & Conquer Generals Zero Hour - Real Game");

    let mut pending_window_attributes = Some(window_attributes);
    let mut window: Option<Arc<Window>> = None;
    let mut engine: Option<CnCGameEngine> = None;

    #[cfg(feature = "integration-diagnostics")]
    let mut integration_bridge: Option<IntegrationTelemetryBridge> = None;
    #[cfg(feature = "integration-diagnostics")]
    let runtime_handle = tokio::runtime::Handle::current();

    #[allow(deprecated)]
    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);

        let mut drive_frame = |engine: &mut CnCGameEngine, current_window: &Arc<Window>| {
            let frame_timing = match ww3d_engine::update() {
                Ok(_) => match ww3d_engine::timing() {
                    Ok(timing) => {
                        let sync_ms = (timing.total_seconds() * 1000.0)
                            .clamp(0.0, u32::MAX as f32) as u32;
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
        };

        elwt.set_control_flow(ControlFlow::Poll);

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
                if cmd_args.windowed {
                    "Windowed"
                } else {
                    "Fullscreen"
                }
            );

            created_window.set_visible(true);
            created_window.request_redraw();

            #[cfg(target_os = "windows")]
            {
                use raw_window_handle::HasWindowHandle;
                if let Ok(handle) = created_window.window_handle() {
                    if let raw_window_handle::RawWindowHandle::Win32(win) = handle.as_raw() {
                        unsafe {
                            crate::win_main::APPLICATION_WINDOW =
                                win.hwnd.get() as *mut std::ffi::c_void;
                        }
                        debug!("Win32 window handle stored");
                    }
                }
            }

            match pollster::block_on(CnCGameEngine::new(
                created_window.clone(),
                cmd_args.clone(),
            )) {
                Ok(new_engine) => {
                    info!("C&C Game engine initialized successfully!");
                    window = Some(created_window);
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
                Err(err) => {
                    error!("Failed to initialize C&C game engine: {err}");
                    elwt.exit();
                }
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
            info!("Engine shutting down");
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
                let egui_response = engine
                    .egui_state
                    .on_window_event(current_window.as_ref(), event);

                if !egui_response.consumed && !engine.input(event) {
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
                        WindowEvent::RedrawRequested => {
                            drive_frame(engine, current_window);

                            // Match the C++ main loop's self-driven cadence more closely. The
                            // Rust path should not rely on the platform delivering another
                            // redraw request before shell/game progression can continue.
                            current_window.request_redraw();
                        }
                        _ => {}
                    }
                }
            }
            Event::AboutToWait => {
                if matches!(engine.get_state(), GameState::Menu | GameState::Loading) {
                    drive_frame(engine, current_window);
                }
                current_window.request_redraw();
                static FRAME_COUNT: std::sync::atomic::AtomicU32 =
                    std::sync::atomic::AtomicU32::new(0);
                let frame = FRAME_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if frame % 60 == 0 {
                    println!("🔄 Event loop active (frame {})", frame);
                }
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
                // Direct material reference on mesh (legacy fallback)
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
                    Err(e) => {
                        Err(anyhow::anyhow!(
                            "Failed to load W3D model '{}': {}",
                            model_name,
                            e
                        ))
                    }
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
        let startup_age = self.current_startup_logic_frame().saturating_sub(menu_enter_frame);
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
                    warn!("Shell incremental prewarm failed for model '{}': {}", model_name, err);
                }
            }
        }
    }
}
