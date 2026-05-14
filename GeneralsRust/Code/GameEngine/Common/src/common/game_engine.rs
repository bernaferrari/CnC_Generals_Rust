////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: game_engine.rs ///////////////////////////////////////////////////
// Main game engine implementation based on C++ GameEngine
// Author: Converted from Michael S. Booth's C++ implementation, April 2001
/////////////////////////////////////////////////////////////////////////

use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use log::{debug, error, info, warn};
use parking_lot::Mutex;
use std::sync::OnceLock;

use crate::common::audio::game_audio::{
    initialize_global_audio_manager, AudioAffect, AudioManager,
};
use crate::common::ini::{get_global_data, INILoadType, INI};
use crate::common::message_stream::{get_message_stream, GameMessageType};
use crate::common::random_value::init_random_with_seed;
use crate::common::recorder::init_recorder;
use crate::common::system::radar::get_radar_system;
use crate::common::system::{
    big_file_system::BigArchiveBackend,
    cd_manager::{get_cd_manager, init_cd_manager},
    file::FileAccess,
    file_system::get_file_system,
    local_file_system::LocalFileSystem,
    subsystem_interface::{
        SubsystemError, SubsystemInterface, SubsystemManager, SubsystemResult, SubsystemState,
    },
};
use crate::common::{
    command_line::CommandLineParser,
    global_data,
    name_key_generator::NameKeyGenerator,
    recorder::{with_recorder, with_recorder_mut},
    rts::science::ScienceSubsystem,
};
use ww3d_animation::{initialize_animated_sound_mgr, initialize_animated_sound_mgr_from_bytes};

// Forward declarations - these will be implemented as we convert more systems
pub trait GameLogicInterface: Send + Sync {
    fn init(&mut self) -> SubsystemResult<()>;
    fn update(&mut self, delta_time: Duration) -> SubsystemResult<()>;
    fn reset(&mut self) -> SubsystemResult<()>;
    fn shutdown(&mut self) -> SubsystemResult<()>;
    fn get_state(&self) -> SubsystemState;
    /// C++ parity adapter for TheGameLogic->isInMultiplayerGame().
    fn is_in_multiplayer_game(&self) -> bool {
        false
    }
    /// C++ parity adapter for TheTacticalView->getTimeMultiplier().
    fn visual_time_multiplier(&self) -> f32 {
        1.0
    }
    /// C++ parity adapter for TheScriptEngine->isTimeFast().
    fn is_script_time_fast(&self) -> bool {
        false
    }
}

pub trait GameClientInterface: Send + Sync {
    fn init(&mut self) -> SubsystemResult<()>;
    fn update(&mut self, delta_time: Duration) -> SubsystemResult<()>;
    fn render(&mut self) -> SubsystemResult<()>;
    fn reset(&mut self) -> SubsystemResult<()>;
    fn shutdown(&mut self) -> SubsystemResult<()>;
    fn get_state(&self) -> SubsystemState;
    fn is_active(&self) -> bool;
    fn set_active(&mut self, active: bool);
}

type GameClientFactory =
    dyn Fn() -> SubsystemResult<Box<dyn GameClientInterface>> + Send + Sync + 'static;

static GAME_CLIENT_FACTORY: OnceLock<Mutex<Option<Arc<GameClientFactory>>>> = OnceLock::new();

fn game_client_factory_slot() -> &'static Mutex<Option<Arc<GameClientFactory>>> {
    GAME_CLIENT_FACTORY.get_or_init(|| Mutex::new(None))
}

/// Register a runtime factory that builds the concrete `GameClient` bridge.
pub fn register_game_client_factory(
    factory: impl Fn() -> SubsystemResult<Box<dyn GameClientInterface>> + Send + Sync + 'static,
) {
    let mut slot = game_client_factory_slot().lock();
    *slot = Some(Arc::new(factory));
}

/// Clear the runtime game-client factory.
pub fn clear_game_client_factory() {
    let mut slot = game_client_factory_slot().lock();
    *slot = None;
}

fn create_registered_game_client() -> Option<SubsystemResult<Box<dyn GameClientInterface>>> {
    let factory = game_client_factory_slot().lock().clone()?;
    Some(factory())
}

pub trait AudioManagerInterface: Send + Sync {
    fn init(&mut self) -> SubsystemResult<()>;
    fn update(&mut self, delta_time: Duration) -> SubsystemResult<()>;
    fn shutdown(&mut self) -> SubsystemResult<()>;
    fn set_master_volume(&mut self, volume: f32);
    fn get_master_volume(&self) -> f32;
}

pub trait NetworkInterface: Send + Sync {
    fn init(&mut self) -> SubsystemResult<()>;
    fn update(&mut self, delta_time: Duration) -> SubsystemResult<()>;
    fn shutdown(&mut self) -> SubsystemResult<()>;
    fn is_multiplayer_session(&self) -> bool;
    fn is_frame_data_ready(&self) -> bool;
}

/// Adapter that wraps the async `game_network` crate behind the legacy synchronous interface.

/// Game state management for main game flow.
///
/// Canonical enum shared across the engine.  The Main crate re-exports
/// this definition instead of maintaining its own copy.
///
/// | Variant      | C++ equivalent              |
/// |--------------|-----------------------------|
/// | Initializing | early startup, before menu  |
/// | Menu         | shell / main menu           |
/// | Loading      | loading screen / map load   |
/// | InGame       | active gameplay             |
/// | Paused       | game paused, pause overlay  |
/// | Victory      | victory / win screen        |
/// | Defeat       | defeat / loss screen        |
/// | Exiting      | shutdown in progress        |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Initializing,
    Menu,
    Loading,
    InGame,
    Paused,
    Victory,
    Defeat,
    Exiting,
}

/// Game Engine Configuration
#[derive(Debug, Clone)]
pub struct GameEngineConfig {
    pub max_fps: u32,
    pub enable_debugging: bool,
    pub enable_networking: bool,
    pub enable_audio: bool,
    pub windowed: bool,
    pub resolution: (u32, u32),
    pub data_paths: Vec<String>,
    pub test_mode: bool, // Enable test mode for CI/automated testing
    pub max_runtime: Option<Duration>, // Maximum runtime before auto-exit (for testing)
}

impl Default for GameEngineConfig {
    fn default() -> Self {
        Self {
            max_fps: 45, // DEFAULT_MAX_FPS from C++
            enable_debugging: cfg!(debug_assertions),
            enable_networking: true,
            enable_audio: true,
            windowed: true,
            resolution: (1024, 768),
            data_paths: vec!["Data".to_string(), "Mods".to_string()],
            test_mode: false,
            max_runtime: None,
        }
    }
}

/// Performance and timing tracking
#[derive(Debug, Default)]
pub struct PerformanceMetrics {
    pub frame_count: u64,
    pub total_time: Duration,
    pub last_frame_time: Duration,
    pub average_fps: f64,
    pub min_frame_time: Duration,
    pub max_frame_time: Duration,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self {
            min_frame_time: Duration::from_millis(1000), // Start with 1 second as min
            ..Default::default()
        }
    }

    pub fn update_frame_timing(&mut self, frame_time: Duration) {
        self.frame_count += 1;
        self.total_time += frame_time;
        self.last_frame_time = frame_time;

        if frame_time < self.min_frame_time {
            self.min_frame_time = frame_time;
        }
        if frame_time > self.max_frame_time {
            self.max_frame_time = frame_time;
        }

        self.average_fps = self.frame_count as f64 / self.total_time.as_secs_f64();
    }
}

const GAME_SINGLE_PLAYER: i32 = 0;
const DIFFICULTY_NORMAL: i32 = 1;

fn sync_after_intro_when_intro_disabled() {
    let Some(global_data) = get_global_data() else {
        return;
    };
    let mut global = global_data.write();
    if !global.play_intro {
        global.after_intro = true;
    }
}

fn handle_initial_file_startup() {
    let Some(global_data) = get_global_data() else {
        return;
    };

    let initial_file = {
        let global = global_data.read();
        if global.initial_file.is_empty() {
            return;
        }
        global.initial_file.clone()
    };

    let initial_file_lower = initial_file.to_ascii_lowercase();
    if initial_file_lower.ends_with(".map") {
        {
            let mut global = global_data.write();
            global.shell_map_on = false;
            global.play_intro = false;
            global.pending_file = initial_file.clone();
        }

        let stream_arc = get_message_stream();
        if let Ok(mut stream) = stream_arc.write() {
            let msg = stream.append_message(GameMessageType::NewGame);
            msg.append_integer_argument(GAME_SINGLE_PLAYER);
            msg.append_integer_argument(DIFFICULTY_NORMAL);
            msg.append_integer_argument(0);
        }

        init_random_with_seed(0);
    } else if initial_file_lower.ends_with(".rep") {
        init_recorder();
        let _ = with_recorder_mut(|recorder| recorder.playback_file(initial_file.clone()));
    }
}

/// Main Game Engine implementation matching C++ GameEngine class
pub struct GameEngine {
    // Core state matching C++ implementation
    config: GameEngineConfig,
    quitting: bool,
    is_active: bool,
    initialized: bool,
    current_state: GameState,

    // Subsystem management
    subsystem_manager: SubsystemManager,

    // Asset management (will be integrated later)
    // asset_manager: Option<Arc<AssetManager>>,

    // Timing and performance
    performance_metrics: PerformanceMetrics,
    last_update: Instant,
    start_time: Instant,
    frame_limiter: Option<Instant>,

    // Command line arguments
    command_args: Vec<String>,

    // High-level subsystem interfaces
    audio_manager: Option<Box<dyn AudioManagerInterface>>,
    network_interface: Option<Box<dyn NetworkInterface>>,
    game_logic: Option<Box<dyn GameLogicInterface>>,
    game_client: Option<Box<dyn GameClientInterface>>,
}

impl Default for GameEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GameEngine {
    /// Create a new game engine instance
    pub fn new() -> Self {
        info!("Creating new GameEngine instance");

        Self {
            config: GameEngineConfig::default(),
            quitting: false,
            is_active: false,
            initialized: false,
            current_state: GameState::Initializing,
            subsystem_manager: SubsystemManager::new(),
            performance_metrics: PerformanceMetrics::new(),
            last_update: Instant::now(),
            start_time: Instant::now(),
            frame_limiter: None,
            command_args: Vec::new(),
            audio_manager: None,
            network_interface: None,
            game_logic: None,
            game_client: None,
        }
    }

    /// Initialize the game engine (matching C++ init method)
    pub async fn init(&mut self, args: Vec<String>) -> SubsystemResult<()> {
        info!("================================================================================");
        info!("Initializing Command & Conquer Generals Zero Hour - Rust Edition");
        info!("Version: 2025.1.0 (Rust Conversion)");
        info!("Build: Debug/Development");
        info!("================================================================================");

        self.command_args = args;

        // Align with legacy startup: seed the global name-key generator before subsystems request keys.
        NameKeyGenerator::init();
        self.bootstrap_global_data_from_ini();
        self.parse_command_line()?;

        // Initialize asset system
        info!("Initializing asset system");
        self.init_asset_system().await?;

        // Initialize subsystems in order (matching C++ initialization order)
        self.init_file_system().await?;
        self.init_audio_system().await?;
        init_cd_manager();
        self.init_network_system().await?;
        self.init_game_client().await?;
        self.init_game_logic().await?;

        // Initialize subsystem manager (GameLogic/GameClient will be registered by callers)
        self.subsystem_manager.init_all_async().await?;
        if !self.subsystem_manager.initialization_plan().is_empty() {
            info!(
                "Subsystem initialization order: {}",
                self.subsystem_manager.initialization_plan().join(" -> ")
            );
        }

        handle_initial_file_startup();
        sync_after_intro_when_intro_disabled();

        // C++ parity: Three mask-initialization functions called before resetAll.
        // They initialize global static bitmasks used by numerous systems.
        crate::common::system::kind_of::init_kind_of_masks();
        crate::common::system::disabled_types::init_disabled_masks();
        crate::common::damage_fx::init_damage_type_flags();

        // C++ parity: TheSubsystemList->resetAll() is invoked at the end of init.
        self.subsystem_manager.reset_all()?;

        self.initialized = true;
        self.last_update = Instant::now();

        // Transition to main menu state
        self.set_game_state(GameState::Menu);

        info!("GameEngine initialization completed successfully");
        Ok(())
    }

    /// Main execution loop (matching C++ execute method)
    pub async fn execute(&mut self) -> SubsystemResult<()> {
        if !self.initialized {
            return Err(SubsystemError::NotInitialized);
        }

        info!("Starting main game loop");
        self.frame_limiter = Some(Instant::now());
        let benchmark_start = Instant::now();
        // Main game loop matching C++ while(!m_quitting) structure
        while !self.quitting {
            let current_time = Instant::now();
            let delta_time = current_time.duration_since(self.last_update);

            // Update timing metrics
            self.last_update = current_time;

            // Service Windows OS (on Windows)
            self.service_os();

            if self.check_benchmark_timeout(benchmark_start) {
                break;
            }

            // Update all subsystems
            if let Err(e) = self.update_frame(delta_time).await {
                error!("Frame update failed: {}", e);
                return Err(SubsystemError::UpdateFailed(format!(
                    "Uncaught exception in GameEngine::update: {e}"
                )));
            }

            // Update game state logic
            self.update_game_state();

            // Check for exit conditions
            if self.should_quit() {
                info!("Exit condition detected, beginning shutdown");
                break;
            }

            if self.should_apply_execute_loop_throttle() {
                // C++ parity: debug/internal builds yield a tiny timeslice before fps limiting.
                #[cfg(any(debug_assertions, feature = "internal"))]
                std::thread::sleep(Duration::from_millis(1));

                let prev_frame_time = self.frame_limiter.unwrap_or_else(Instant::now);
                if let Some(updated_prev_time) = self.apply_frame_limit(prev_frame_time).await {
                    self.frame_limiter = Some(updated_prev_time);
                }
            }

            // Debug output every 1000 frames (like C++ version)
            if self.performance_metrics.frame_count % 1000 == 0 {
                debug!(
                    "Frame {}, Average FPS: {:.2}, Last frame: {:.2}ms",
                    self.performance_metrics.frame_count,
                    self.performance_metrics.average_fps,
                    self.performance_metrics.last_frame_time.as_millis()
                );
            }
        }

        info!("Main game loop exited, shutting down");
        self.shutdown().await?;
        Ok(())
    }

    fn check_benchmark_timeout(&mut self, benchmark_start: Instant) -> bool {
        // C++ parity: benchmark timer shutdown logic is debug/internal only.
        if !cfg!(any(debug_assertions, feature = "internal")) {
            return false;
        }

        let Some(global_data) = get_global_data() else {
            return false;
        };
        let global_data = global_data.read();
        let timer = global_data.benchmark_timer;
        if timer <= 0 {
            return false;
        }
        if benchmark_start.elapsed() < Duration::from_secs(timer as u64) {
            return false;
        }
        if self.current_state != GameState::InGame {
            return false;
        }

        with_recorder_mut(|recorder| {
            if recorder.is_recording() {
                recorder.stop_recording();
            }
        });

        if let Some(game_logic) = &mut self.game_logic {
            if let Err(err) = game_logic.reset() {
                warn!("Benchmark shutdown: failed to clear game logic: {}", err);
            }
        }

        info!("Benchmark timer reached ({}s) - exiting", timer);
        self.set_quitting(true);
        true
    }

    fn should_apply_frame_limit(
        use_fps_limit: bool,
        max_fps: i32,
        tivo_fast_mode: bool,
        replay_playback: bool,
    ) -> bool {
        if !use_fps_limit || max_fps <= 0 {
            return false;
        }
        if tivo_fast_mode && replay_playback {
            return false;
        }
        true
    }

    fn should_apply_execute_loop_throttle(&self) -> bool {
        let visual_time_multiplier = self
            .game_logic
            .as_ref()
            .map(|logic| logic.visual_time_multiplier())
            .unwrap_or(1.0);
        let script_time_fast = self
            .game_logic
            .as_ref()
            .map(|logic| logic.is_script_time_fast())
            .unwrap_or(false);

        visual_time_multiplier <= 1.0 && !script_time_fast
    }

    async fn apply_frame_limit(&self, prev_frame_time: Instant) -> Option<Instant> {
        let use_fps_limit = global_data::read_safe()
            .map(|data| data.writable.use_fps_limit)
            .unwrap_or(false);

        // C++ parity: frame limiter consumes GameEngine::m_maxFPS, not a per-frame global read.
        let max_fps = (self.config.max_fps as i32).max(0);

        let tivo_fast_mode = get_global_data()
            .map(|data| data.read().tivo_fast_mode)
            .unwrap_or(false);
        let replay_playback = with_recorder_mut(|recorder| recorder.is_playback()).unwrap_or(false);

        if !Self::should_apply_frame_limit(use_fps_limit, max_fps, tivo_fast_mode, replay_playback)
        {
            return None;
        }

        let limit_ms = (1000.0 / max_fps as f32 - 1.0).max(0.0);
        let mut now = Instant::now();
        if limit_ms > 0.0 {
            let limit = Duration::from_millis(limit_ms as u64);
            while now.duration_since(prev_frame_time) < limit {
                tokio::time::sleep(Duration::from_millis(0)).await;
                now = Instant::now();
            }
        }
        Some(now)
    }

    /// Update a single frame (matching C++ per-frame update)
    async fn update_frame(&mut self, delta_time: Duration) -> SubsystemResult<()> {
        // Update any registered subsystems first (science store, etc.)
        self.subsystem_manager.update_all_async().await?;

        if let Ok(mut radar) = get_radar_system().write() {
            let frame = self.performance_metrics.frame_count.saturating_add(1) as u32;
            radar.update(frame);
        }

        if let Some(audio) = &mut self.audio_manager {
            audio.update(delta_time)?;
        }

        if let Some(game_client) = &mut self.game_client {
            game_client.update(delta_time)?;
            if game_client.is_active() {
                game_client.render()?;
            }
        }

        // C++ parity: TheMessageStream->propagateMessages() executes between client update
        // and network/CD/game-logic progression.
        let stream_arc = get_message_stream();
        if let Ok(mut stream) = stream_arc.write() {
            if let Err(err) = stream.propagate_messages() {
                warn!("Message propagation failed: {}", err);
            }
        }

        let should_update_game_logic = if let Some(network) = &mut self.network_interface {
            network.update(delta_time)?;
            if network.is_multiplayer_session() {
                network.is_frame_data_ready()
            } else {
                self.current_state != GameState::Paused
            }
        } else {
            self.current_state != GameState::Paused
        };

        if let Some(mut cd_manager) = get_cd_manager() {
            cd_manager.update();
        }

        if should_update_game_logic {
            if let Some(game_logic) = &mut self.game_logic {
                game_logic.update(delta_time)?;
            }
        }

        self.performance_metrics.update_frame_timing(delta_time);
        Ok(())
    }

    /// Reset the engine to initial state (matching C++ reset method)
    pub async fn reset(&mut self) -> SubsystemResult<()> {
        info!("Resetting GameEngine to initial state");

        // C++ parity: reset all initialized subsystems before rebuilding gameplay/session state.
        if self.initialized {
            self.subsystem_manager.reset_all()?;
        }

        // Reset all subsystems
        if let Some(game_logic) = &mut self.game_logic {
            game_logic.reset()?;
        }

        if let Some(game_client) = &mut self.game_client {
            game_client.reset()?;
        }

        // C++ parity: multiplayer reset tears down network session object.
        let delete_network = self
            .game_logic
            .as_ref()
            .map(|logic| logic.is_in_multiplayer_game())
            .unwrap_or(false);
        if delete_network {
            if let Some(mut network) = self.network_interface.take() {
                if let Err(err) = network.shutdown() {
                    warn!(
                        "Reset: network shutdown failed during multiplayer teardown: {}",
                        err
                    );
                }
            }
        }

        // Reset performance metrics
        self.performance_metrics = PerformanceMetrics::new();
        self.last_update = Instant::now();
        self.frame_limiter = None;

        info!("GameEngine reset completed");
        Ok(())
    }

    /// Shutdown the engine (matching C++ destructor behavior)
    pub async fn shutdown(&mut self) -> SubsystemResult<()> {
        info!("Shutting down GameEngine");

        // Shutdown in reverse order of initialization
        if let Some(game_client) = &mut self.game_client {
            game_client.shutdown()?;
        }

        if let Some(game_logic) = &mut self.game_logic {
            game_logic.shutdown()?;
        }

        if let Some(network) = &mut self.network_interface {
            network.shutdown()?;
        }

        if let Some(audio) = &mut self.audio_manager {
            audio.shutdown()?;
        }

        // Shutdown subsystem manager
        self.subsystem_manager.shutdown_all()?;

        self.initialized = false;
        info!("GameEngine shutdown completed");
        Ok(())
    }

    // Getters/Setters matching C++ interface

    pub fn set_quitting(&mut self, quitting: bool) {
        self.quitting = quitting;
        if quitting {
            info!("GameEngine quitting flag set");
        }
    }

    pub fn get_quitting(&self) -> bool {
        self.quitting
    }

    pub fn data_paths(&self) -> &[String] {
        &self.config.data_paths
    }

    pub fn is_multiplayer_session(&self) -> bool {
        with_recorder(|recorder| recorder.is_multiplayer()).unwrap_or(false)
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn set_is_active(&mut self, is_active: bool) {
        self.is_active = is_active;
        if let Some(client) = &mut self.game_client {
            client.set_active(is_active);
        }
    }

    pub fn set_frames_per_second_limit(&mut self, fps: u32) {
        self.config.max_fps = fps;
        info!("Frame rate limit set to: {} FPS", fps);
    }

    pub fn get_frames_per_second_limit(&self) -> u32 {
        self.config.max_fps
    }

    pub fn get_performance_metrics(&self) -> &PerformanceMetrics {
        &self.performance_metrics
    }

    // Game state management

    pub fn get_game_state(&self) -> GameState {
        self.current_state.clone()
    }

    pub fn set_game_state(&mut self, new_state: GameState) {
        let old_state = self.current_state.clone();
        self.current_state = new_state.clone();

        info!("Game state transition: {:?} -> {:?}", old_state, new_state);

        // Handle state transition logic
        match new_state {
            GameState::Menu => {}
            GameState::Loading => {}
            GameState::InGame => {
                info!("Starting gameplay session");
            }
            GameState::Paused => {}
            GameState::Victory | GameState::Defeat => {}
            GameState::Initializing => {}
            GameState::Exiting => {
                info!("Game exiting - initiating shutdown");
                self.quitting = true;
            }
        }
    }

    fn update_game_state(&mut self) {
        match self.current_state {
            GameState::Initializing => {}
            GameState::Menu => {}
            GameState::Loading => {}
            GameState::InGame => {}
            GameState::Paused => {}
            GameState::Victory | GameState::Defeat => {}
            GameState::Exiting => {}
        }
    }

    // Private helper methods

    fn parse_command_line(&mut self) -> SubsystemResult<()> {
        debug!("Parsing command line arguments: {:?}", self.command_args);

        let mut parser = CommandLineParser::from_runtime_global_data();
        parser.parse_command_line(self.command_args.clone());

        let writable = parser.get_global_data().clone();

        self.config.windowed = writable.windowed;
        // C++ parity: always push GlobalData::m_framesPerSecondLimit into engine m_maxFPS.
        // Clamp negative values to 0 to keep Rust representation safe.
        self.set_frames_per_second_limit(writable.frames_per_second_limit.max(0) as u32);

        if writable.x_resolution > 0 && writable.y_resolution > 0 {
            self.config.resolution = (writable.x_resolution as u32, writable.y_resolution as u32);
            info!(
                "Resolution set to: {}x{}",
                self.config.resolution.0, self.config.resolution.1
            );
        }

        // Audio is enabled only if all relevant toggles are on.
        self.config.enable_audio =
            writable.audio_on && writable.music_on && writable.sounds_on && writable.speech_on;

        let mut add_data_path = |path: &str| {
            if path.is_empty() {
                return;
            }
            if !self
                .config
                .data_paths
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(path))
            {
                self.config.data_paths.push(path.to_string());
            }
        };

        add_data_path(&writable.mod_dir);
        if !writable.mod_big.is_empty() {
            if let Some(parent) = std::path::Path::new(&writable.mod_big).parent() {
                if let Some(parent_str) = parent.to_str() {
                    add_data_path(parent_str);
                }
            }
        }

        // Handle engine-specific flags that are not part of the legacy command line layer yet.
        let mut i = 0;
        while i < self.command_args.len() {
            match self.command_args[i].as_str() {
                "-nonetwork" => {
                    self.config.enable_networking = false;
                    info!("Networking disabled");
                }
                "-test" | "--test" => {
                    self.config.test_mode = true;
                    info!("Test mode enabled");
                }
                "-max-runtime" => {
                    if i + 1 < self.command_args.len() {
                        if let Ok(seconds) = self.command_args[i + 1].parse::<u64>() {
                            self.config.max_runtime = Some(Duration::from_secs(seconds));
                            info!("Maximum runtime set to: {} seconds", seconds);
                        }
                        i += 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }

        Ok(())
    }

    fn bootstrap_global_data_from_ini(&self) {
        // C++ parity: load GameData INI before applying command-line overrides.
        let mut ini = INI::new();
        let mut loaded_any = false;

        for (index, source) in ["Data/INI/Default/GameData.ini", "Data/INI/GameData.ini"]
            .iter()
            .enumerate()
        {
            let load_type = if index == 0 {
                INILoadType::Overwrite
            } else {
                INILoadType::MultiFile
            };
            match ini.load(source, load_type) {
                Ok(()) => loaded_any = true,
                Err(err) => warn!("Bootstrap GameData source '{}' not loaded: {}", source, err),
            }
        }

        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            if let Err(err) = ini.load("Data/INI/GameDataDebug.ini", INILoadType::MultiFile) {
                debug!("Optional GameDataDebug.ini not loaded: {}", err);
            }
        }

        if loaded_any {
            debug!("Bootstrap GameData INI loaded before command-line override pass");
        } else {
            warn!("No GameData INI sources loaded during bootstrap; using runtime defaults");
        }
    }

    async fn init_asset_system(&mut self) -> SubsystemResult<()> {
        info!("Loading game assets from .big archives");

        // Initialize default asset paths based on the current working directory
        let writable_snapshot = {
            let data = global_data::read();
            data.writable.clone()
        };

        let mut asset_paths: Vec<PathBuf> = vec![
            PathBuf::from("Data"),
            PathBuf::from("Mods"),
            PathBuf::from("Assets"),
            PathBuf::from("GeneralsRust/Code/Main/assets"),
        ];

        for path in &self.config.data_paths {
            asset_paths.push(PathBuf::from(path));
        }

        if !writable_snapshot.mod_dir.is_empty() {
            asset_paths.push(PathBuf::from(&writable_snapshot.mod_dir));
        }

        if !writable_snapshot.mod_big.is_empty() {
            if let Some(parent) = PathBuf::from(&writable_snapshot.mod_big).parent() {
                asset_paths.push(parent.to_path_buf());
            }
        }

        // Deduplicate while preserving insertion order
        let mut seen = HashSet::new();
        asset_paths.retain(|path| {
            let key = path.to_string_lossy().replace('\\', "/").to_lowercase();
            seen.insert(key)
        });

        debug!("Asset search paths: {:?}", asset_paths);

        {
            let file_system = get_file_system();
            let mut fs = file_system.lock().expect("FileSystem mutex poisoned");

            {
                let local_backend: &mut LocalFileSystem = fs.ensure_backend(LocalFileSystem::new);
                for path in &asset_paths {
                    local_backend.add_search_path(path);
                }
            }

            {
                let big_backend: &mut BigArchiveBackend = fs.ensure_backend(BigArchiveBackend::new);
                for path in &asset_paths {
                    big_backend.add_search_path(path);
                }
            }

            fs.clear_cache();

            if fs.state() != SubsystemState::Running {
                fs.init().map_err(|err| {
                    SubsystemError::InitializationFailed(format!(
                        "Failed to init FileSystem: {}",
                        err
                    ))
                })?;
            }
        }

        let file_system = get_file_system();
        let fs_guard = file_system.lock().expect("FileSystem mutex poisoned");

        // Mirrors C++ archive expectations for Zero Hour.
        let important_big_files = [
            "INIZH.big",
            "W3DZH.big",
            "TexturesZH.big",
            "AudioZH.big",
            "EnglishZH.big",
        ];

        let mut big_files_found = 0;
        for archive in &important_big_files {
            if fs_guard.does_file_exist(archive) {
                big_files_found += 1;
            } else {
                debug!(
                    "Required archive '{}' not found in current search paths",
                    archive
                );
            }
        }

        if big_files_found == important_big_files.len() {
            info!("Asset system initialised - all core archives located");
        } else if big_files_found > 0 {
            warn!(
                "Asset system initialised - located {}/{} core archives",
                big_files_found,
                important_big_files.len()
            );
        } else {
            warn!("Asset system initialised without core archives; running in minimal mode");
        }

        if !writable_snapshot.mod_big.is_empty() {
            if fs_guard.does_file_exist(&writable_snapshot.mod_big) {
                info!("Active mod archive detected: {}", writable_snapshot.mod_big);
            } else {
                warn!(
                    "Mod archive '{}' was requested but not found",
                    writable_snapshot.mod_big
                );
            }
        }

        self.subsystem_manager
            .register_subsystem(ScienceSubsystem::descriptor())?;

        Ok(())
    }

    async fn init_file_system(&mut self) -> SubsystemResult<()> {
        info!("Initializing file system");

        // Ensure the local file system backend is registered; BIG archives are loaded in game engine init.
        let fs = get_file_system();
        let _guard = fs.lock().map_err(|_| {
            SubsystemError::InitializationFailed("FileSystem mutex poisoned".into())
        })?;

        Ok(())
    }

    async fn init_audio_system(&mut self) -> SubsystemResult<()> {
        if !self.config.enable_audio {
            info!("Audio system disabled");
            return Ok(());
        }

        info!("Initializing audio system");

        match self.resolve_anim_sound_ini() {
            Some(ResolvedAnimSoundIni::Path(ini_path)) => {
                match initialize_animated_sound_mgr(Some(ini_path.as_path())) {
                    Ok(_) => info!("Animated sound metadata loaded from {}", ini_path.display()),
                    Err(err) => warn!(
                        "Failed to parse animated sound metadata at {}: {err:?}",
                        ini_path.display()
                    ),
                }
            }
            Some(ResolvedAnimSoundIni::Bytes { source_name, bytes }) => {
                match initialize_animated_sound_mgr_from_bytes(&bytes, &source_name) {
                    Ok(_) => info!(
                        "Animated sound metadata loaded from mounted {}",
                        source_name
                    ),
                    Err(err) => warn!(
                        "Failed to parse animated sound metadata from {}: {err:?}",
                        source_name
                    ),
                }
            }
            None => {
                if let Err(err) = initialize_animated_sound_mgr::<&str>(None) {
                    warn!(
                        "Failed to initialize animated sound metadata from default search: {err:?}"
                    );
                }
            }
        }

        let legacy_manager = initialize_global_audio_manager();
        let mut audio_handle = LegacyAudioManagerHandle::new(legacy_manager);
        audio_handle.init()?;

        self.audio_manager = Some(Box::new(audio_handle));
        info!("Audio system initialized");
        Ok(())
    }

    async fn init_network_system(&mut self) -> SubsystemResult<()> {
        if !self.config.enable_networking {
            info!("Network system disabled");
            return Ok(());
        }

        // C++ parity: TheNetwork remains NULL after startup init and is created on MP session start.
        self.network_interface = None;
        info!("Network runtime available but no active multiplayer session at startup");
        Ok(())
    }

    fn resolve_anim_sound_ini(&self) -> Option<ResolvedAnimSoundIni> {
        const ANIM_SOUND_FILE: &str = "w3danimsound.ini";

        for base in &self.config.data_paths {
            for candidate in [
                Path::new(base).join(ANIM_SOUND_FILE),
                Path::new(base).join("INI").join(ANIM_SOUND_FILE),
                Path::new(base).join("Default").join(ANIM_SOUND_FILE),
                Path::new(base)
                    .join("INI")
                    .join("Default")
                    .join(ANIM_SOUND_FILE),
            ] {
                if candidate.exists() {
                    return Some(ResolvedAnimSoundIni::Path(candidate));
                }
            }
        }

        for candidate in [
            Path::new(ANIM_SOUND_FILE).to_path_buf(),
            Path::new("Data").join("INI").join(ANIM_SOUND_FILE),
            Path::new("Data")
                .join("INI")
                .join("Default")
                .join(ANIM_SOUND_FILE),
        ] {
            if candidate.exists() {
                return Some(ResolvedAnimSoundIni::Path(candidate));
            }
        }

        if let Some((source_name, bytes)) = read_virtual_asset_bytes(&[
            ANIM_SOUND_FILE,
            "Data/INI/w3danimsound.ini",
            "Data/INI/Default/w3danimsound.ini",
        ]) {
            return Some(ResolvedAnimSoundIni::Bytes { source_name, bytes });
        }

        let big_candidates = [
            Path::new("INIZH.big"),
            Path::new("INI.big"),
            Path::new("windows_game/Command & Conquer Generals Zero Hour/INIZH.big"),
            Path::new("windows_game/Command & Conquer Generals Zero Hour/INI.big"),
            Path::new("windows_game/Command & Conquer Generals/INI.big"),
            Path::new("windows_game/Command & Conquer Generals/Data/INI.big"),
            Path::new("../windows_game/Command & Conquer Generals Zero Hour/INIZH.big"),
            Path::new("../windows_game/Command & Conquer Generals Zero Hour/INI.big"),
            Path::new("../windows_game/Command & Conquer Generals/INI.big"),
            Path::new("../windows_game/Command & Conquer Generals/Data/INI.big"),
        ];

        let entry_names = [
            "data/ini/w3danimsound.ini",
            "data\\ini\\w3danimsound.ini",
            "data/ini/default/w3danimsound.ini",
            "data\\ini\\default\\w3danimsound.ini",
            "data/ini/w3danimSound.ini",
            "data\\ini\\W3DAnimSound.ini",
        ];

        for candidate in big_candidates {
            let mut candidate_paths = Vec::new();
            candidate_paths.push(candidate.to_path_buf());
            if let Ok(cwd) = std::env::current_dir() {
                for ancestor in cwd.ancestors() {
                    candidate_paths.push(ancestor.join(candidate));
                }
            }

            for candidate_path in candidate_paths {
                if !candidate_path.exists() {
                    continue;
                }
                match extract_big_entry_with_name(&candidate_path, &entry_names) {
                    Ok(Some((entry_name, bytes))) => {
                        return Some(ResolvedAnimSoundIni::Bytes {
                            source_name: format!("{}::{}", candidate_path.display(), entry_name),
                            bytes,
                        });
                    }
                    Ok(None) => {
                        debug!(
                            "w3danimsound.ini not found inside archive {}",
                            candidate_path.display()
                        );
                    }
                    Err(err) => {
                        warn!(
                            "Failed to inspect {} for w3danimsound.ini: {err:?}",
                            candidate_path.display()
                        );
                    }
                }
            }
        }

        None
    }
}

enum ResolvedAnimSoundIni {
    Path(PathBuf),
    Bytes { source_name: String, bytes: Vec<u8> },
}

fn read_virtual_asset_bytes(virtual_names: &[&str]) -> Option<(String, Vec<u8>)> {
    let file_system = get_file_system();
    let mut fs_guard = file_system.lock().ok()?;

    for virtual_name in virtual_names {
        let Some(mut file) =
            fs_guard.open_file(virtual_name, FileAccess::READ.combine(FileAccess::BINARY))
        else {
            continue;
        };
        let Ok(bytes) = file.read_entire_and_close() else {
            continue;
        };
        return Some(((*virtual_name).to_string(), bytes));
    }

    None
}

struct LegacyAudioManagerHandle {
    manager: Arc<std::sync::Mutex<AudioManager>>,
}

impl LegacyAudioManagerHandle {
    fn new(manager: Arc<std::sync::Mutex<AudioManager>>) -> Self {
        Self { manager }
    }
}

impl AudioManagerInterface for LegacyAudioManagerHandle {
    fn init(&mut self) -> SubsystemResult<()> {
        if let Ok(mut mgr) = self.manager.lock() {
            AudioManager::init(&mut *mgr);
        }
        Ok(())
    }

    fn update(&mut self, _delta_time: Duration) -> SubsystemResult<()> {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.update();
        }
        Ok(())
    }

    fn shutdown(&mut self) -> SubsystemResult<()> {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.reset();
        }
        Ok(())
    }

    fn set_master_volume(&mut self, volume: f32) {
        if let Ok(mut mgr) = self.manager.lock() {
            mgr.set_volume(volume, AudioAffect::All);
        }
    }

    fn get_master_volume(&self) -> f32 {
        self.manager
            .lock()
            .map(|mgr| mgr.get_volume(AudioAffect::Sound))
            .unwrap_or(1.0)
    }
}

fn extract_big_entry_with_name(
    candidate: &Path,
    entry_names: &[&str],
) -> std::io::Result<Option<(String, Vec<u8>)>> {
    let mut file = match File::open(candidate) {
        Ok(f) => f,
        Err(err) => return Err(err),
    };

    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;
    if magic != *b"BIGF" && magic != *b"BIG4" {
        return Ok(None);
    }

    let mut buf = [0u8; 4];
    file.read_exact(&mut buf)?; // archive size (unused)
    file.read_exact(&mut buf)?; // entry count (BE)
    let entry_count = u32::from_be_bytes(buf);
    file.read_exact(&mut buf)?; // reserved

    let normalized_targets: Vec<String> = entry_names
        .iter()
        .map(|name| name.replace('\\', "/").to_lowercase())
        .collect();

    for _ in 0..entry_count {
        let mut tmp = [0u8; 4];
        file.read_exact(&mut tmp)?;
        let offset = u32::from_be_bytes(tmp) as u64;
        file.read_exact(&mut tmp)?;
        let size = u32::from_be_bytes(tmp) as usize;

        let mut name_bytes = Vec::with_capacity(64);
        loop {
            let mut b = [0u8; 1];
            file.read_exact(&mut b)?;
            if b[0] == 0 {
                break;
            }
            name_bytes.push(b[0]);
        }

        let normalized_name = String::from_utf8_lossy(&name_bytes)
            .replace('\\', "/")
            .to_lowercase();

        if normalized_targets
            .iter()
            .any(|target| *target == normalized_name)
        {
            if size == 0 {
                return Ok(None);
            }

            let current_pos = file.stream_position()?;
            file.seek(SeekFrom::Start(offset))?;
            let mut data = vec![0u8; size];
            file.read_exact(&mut data)?;
            file.seek(SeekFrom::Start(current_pos))?;
            return Ok(Some((normalized_name, data)));
        }
    }

    Ok(None)
}

impl GameEngine {
    async fn init_game_logic(&mut self) -> SubsystemResult<()> {
        info!("Initializing game logic system");

        // Game logic is expected to be provided by the host; keep running even if absent.
        if self.game_logic.is_none() {
            info!("No GameLogic registered; skipping logic initialization");
        } else if let Some(logic) = &mut self.game_logic {
            logic.init()?;
            info!("Game logic system initialized");
        }
        Ok(())
    }

    async fn init_game_client(&mut self) -> SubsystemResult<()> {
        info!("Initializing game client system");

        if let Some(factory_result) = create_registered_game_client() {
            match factory_result {
                Ok(mut game_client) => {
                    info!("Using registered game client bootstrap");
                    if let Err(err) = game_client.init() {
                        return Err(SubsystemError::InitializationFailed(format!(
                            "registered game client bootstrap failed during init: {}",
                            err
                        )));
                    }
                    self.game_client = Some(game_client);
                    info!("Game client system initialized successfully");
                    return Ok(());
                }
                Err(err) => {
                    return Err(SubsystemError::InitializationFailed(format!(
                        "registered game client bootstrap failed to create client: {}",
                        err
                    )));
                }
            }
        }

        return Err(SubsystemError::InitializationFailed(
            "No game client factory registered. Call register_game_client_factory() before init."
                .into(),
        ));
    }

    fn service_os(&mut self) {
        // Platform-specific OS message handling would go here
        // On Windows, this would process Windows messages
        // For now, this is a no-op
    }

    fn should_quit(&self) -> bool {
        // Check various quit conditions
        if self.quitting {
            return true;
        }

        // Check for maximum runtime (for testing)
        if let Some(max_runtime) = self.config.max_runtime {
            if self.start_time.elapsed() >= max_runtime {
                info!("Maximum runtime ({:?}) reached - auto-exiting", max_runtime);
                return true;
            }
        }

        if let Some(game_logic) = &self.game_logic {
            if matches!(
                game_logic.get_state(),
                SubsystemState::ShuttingDown | SubsystemState::Shutdown | SubsystemState::Error
            ) {
                info!("GameLogic requested shutdown");
                return true;
            }
        }

        if let Some(game_client) = &self.game_client {
            if matches!(
                game_client.get_state(),
                SubsystemState::ShuttingDown | SubsystemState::Shutdown | SubsystemState::Error
            ) {
                info!("GameClient requested shutdown");
                return true;
            }
        }

        if self.subsystem_manager.has_error() {
            info!("Subsystem manager reports an error state");
            return true;
        }

        false
    }

    fn service_os(&mut self) { (matching C++ TheGameEngine)
static GAME_ENGINE_INSTANCE: OnceLock<Mutex<Option<Arc<Mutex<GameEngine>>>>> = OnceLock::new();

fn game_engine_slot() -> &'static Mutex<Option<Arc<Mutex<GameEngine>>>> {
    GAME_ENGINE_INSTANCE.get_or_init(|| Mutex::new(None))
}

/// Get the global game engine instance.
pub fn get_game_engine() -> Option<Arc<Mutex<GameEngine>>> {
    game_engine_slot().lock().clone()
}

/// Install the global game engine instance.
pub fn set_game_engine(engine: GameEngine) -> Arc<Mutex<GameEngine>> {
    let engine = Arc::new(Mutex::new(engine));
    *game_engine_slot().lock() = Some(Arc::clone(&engine));
    engine
}

/// Clear the global game engine instance.
pub fn clear_game_engine() {
    *game_engine_slot().lock() = None;
}

/// Initialize (or retrieve) the global game engine instance.
pub fn init_game_engine() -> Arc<Mutex<GameEngine>> {
    if let Some(engine) = get_game_engine() {
        engine
    } else {
        set_game_engine(GameEngine::new())
    }
}

/// Factory function matching C++ CreateGameEngine()
pub fn create_game_engine() -> GameEngine {
    GameEngine::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::ini_game_data::init_global_data;
    use crate::common::message_stream::get_message_stream;
    use crate::common::recorder::init_recorder;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    struct CountingGameLogic {
        updates: Arc<AtomicUsize>,
    }

    impl CountingGameLogic {
        fn new(updates: Arc<AtomicUsize>) -> Self {
            Self { updates }
        }
    }

    impl GameLogicInterface for CountingGameLogic {
        fn init(&mut self) -> SubsystemResult<()> {
            Ok(())
        }

        fn update(&mut self, _delta_time: Duration) -> SubsystemResult<()> {
            self.updates.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn reset(&mut self) -> SubsystemResult<()> {
            Ok(())
        }

        fn shutdown(&mut self) -> SubsystemResult<()> {
            Ok(())
        }

        fn get_state(&self) -> SubsystemState {
            SubsystemState::Running
        }
    }

    struct StubNetwork {
        frame_ready: bool,
    }

    impl StubNetwork {
        fn new(frame_ready: bool) -> Self {
            Self { frame_ready }
        }
    }

    impl NetworkInterface for StubNetwork {
        fn init(&mut self) -> SubsystemResult<()> {
            Ok(())
        }

        fn update(&mut self, _delta_time: Duration) -> SubsystemResult<()> {
            Ok(())
        }

        fn shutdown(&mut self) -> SubsystemResult<()> {
            Ok(())
        }

        fn is_multiplayer_session(&self) -> bool {
            true
        }

        fn is_frame_data_ready(&self) -> bool {
            self.frame_ready
        }
    }

    struct RegisteredGameClient {
        init_calls: Arc<AtomicUsize>,
        init_error: Option<String>,
        active: bool,
        state: SubsystemState,
    }

    impl RegisteredGameClient {
        fn new(init_calls: Arc<AtomicUsize>) -> Self {
            Self {
                init_calls,
                init_error: None,
                active: true,
                state: SubsystemState::Uninitialized,
            }
        }

        fn failing_init(init_calls: Arc<AtomicUsize>, reason: &str) -> Self {
            Self {
                init_calls,
                init_error: Some(reason.to_string()),
                active: true,
                state: SubsystemState::Uninitialized,
            }
        }
    }

    impl GameClientInterface for RegisteredGameClient {
        fn init(&mut self) -> SubsystemResult<()> {
            self.init_calls.fetch_add(1, Ordering::Relaxed);
            if let Some(reason) = self.init_error.take() {
                self.state = SubsystemState::Error;
                return Err(SubsystemError::InitializationFailed(reason));
            }
            self.state = SubsystemState::Running;
            Ok(())
        }

        fn update(&mut self, _delta_time: Duration) -> SubsystemResult<()> {
            Ok(())
        }

        fn render(&mut self) -> SubsystemResult<()> {
            Ok(())
        }

        fn reset(&mut self) -> SubsystemResult<()> {
            self.state = SubsystemState::Running;
            Ok(())
        }

        fn shutdown(&mut self) -> SubsystemResult<()> {
            self.state = SubsystemState::Shutdown;
            Ok(())
        }

        fn get_state(&self) -> SubsystemState {
            self.state
        }

        fn is_active(&self) -> bool {
            self.active
        }

        fn set_active(&mut self, active: bool) {
            self.active = active;
        }
    }

    #[test]
    fn test_registered_game_client_factory_is_used_when_available() {
        let _guard = TEST_LOCK.lock();
        clear_game_client_factory();

        let init_calls = Arc::new(AtomicUsize::new(0));
        let factory_calls = Arc::clone(&init_calls);
        register_game_client_factory(move || {
            Ok(Box::new(RegisteredGameClient::new(Arc::clone(
                &factory_calls,
            ))))
        });

        let mut engine = GameEngine::new();
        let result = tokio_test::block_on(engine.init_game_client());
        assert!(
            result.is_ok(),
            "registered client init failed: {:?}",
            result
        );

        let client = engine
            .game_client
            .as_ref()
            .expect("registered client should have been installed");
        assert_eq!(init_calls.load(Ordering::Relaxed), 1);
        assert_eq!(client.get_state(), SubsystemState::Running);

        clear_game_client_factory();
    }

    #[test]
    fn test_registered_game_client_factory_create_failure_does_not_fallback_to_stub_client() {
        let _guard = TEST_LOCK.lock();
        clear_game_client_factory();

        register_game_client_factory(|| {
            Err(SubsystemError::InitializationFailed(
                "real client unavailable".to_string(),
            ))
        });

        let mut engine = GameEngine::new();
        let result = tokio_test::block_on(engine.init_game_client());

        assert!(matches!(
            result,
            Err(SubsystemError::InitializationFailed(ref message))
                if message.contains("registered game client bootstrap failed to create client")
        ));
        assert!(
            engine.game_client.is_none(),
            "registered bootstrap failures must not be hidden by fallback clients"
        );

        clear_game_client_factory();
    }

    #[test]
    fn test_registered_game_client_init_failure_does_not_fallback_to_stub_client() {
        let _guard = TEST_LOCK.lock();
        clear_game_client_factory();

        let init_calls = Arc::new(AtomicUsize::new(0));
        let factory_calls = Arc::clone(&init_calls);
        register_game_client_factory(move || {
            Ok(Box::new(RegisteredGameClient::failing_init(
                Arc::clone(&factory_calls),
                "wgpu init failed",
            )))
        });

        let mut engine = GameEngine::new();
        let result = tokio_test::block_on(engine.init_game_client());

        assert_eq!(init_calls.load(Ordering::Relaxed), 1);
        assert!(matches!(
            result,
            Err(SubsystemError::InitializationFailed(ref message))
                if message.contains("registered game client bootstrap failed during init")
        ));
        assert!(
            engine.game_client.is_none(),
            "registered bootstrap init failures must not be hidden by fallback clients"
        );

        clear_game_client_factory();
    }

    #[tokio::test]
    async fn test_game_engine_creation() {
        let mut engine = GameEngine::new();
        assert!(!engine.initialized);
        assert!(!engine.quitting);
        assert!(!engine.is_active);
    }

    #[tokio::test]
    async fn test_game_engine_init() {
        let mut engine = GameEngine::new();
        let args = vec!["game.exe".to_string(), "-windowed".to_string()];

        let result = engine.init(args).await;
        assert!(result.is_ok(), "Engine initialization failed: {:?}", result);
        assert!(engine.initialized);
        assert!(engine.config.windowed);
        assert!(engine.network_interface.is_none());
    }

    #[test]
    fn test_command_line_parsing() {
        let mut engine = GameEngine::new();
        engine.command_args = vec![
            "game.exe".to_string(),
            "-fps".to_string(),
            "60".to_string(),
            "-windowed".to_string(),
            "-resolution".to_string(),
            "1920".to_string(),
            "1080".to_string(),
        ];

        let result = engine.parse_command_line();
        assert!(result.is_ok());
        assert_eq!(engine.config.max_fps, 60);
        assert!(engine.config.windowed);
        assert_eq!(engine.config.resolution, (1920, 1080));
    }

    #[test]
    fn test_performance_metrics() {
        let mut metrics = PerformanceMetrics::new();
        let frame_time = Duration::from_millis(16); // ~60 FPS

        metrics.update_frame_timing(frame_time);

        assert_eq!(metrics.frame_count, 1);
        assert_eq!(metrics.last_frame_time, frame_time);
    }

    #[test]
    fn test_sync_after_intro_when_intro_disabled_promotes_runtime_flag() {
        init_global_data();
        let global = get_global_data().expect("global data should be initialized");
        {
            let mut data = global.write();
            data.play_intro = false;
            data.after_intro = false;
        }

        sync_after_intro_when_intro_disabled();

        let data = global.read();
        assert!(!data.play_intro);
        assert!(data.after_intro);
    }

    #[test]
    fn test_handle_initial_file_startup_for_map_disables_intro_and_queues_new_game() {
        init_global_data();
        let global = get_global_data().expect("global data should be initialized");
        {
            let mut data = global.write();
            data.initial_file = "Maps\\TestMap\\TestMap.map".to_string();
            data.pending_file.clear();
            data.shell_map_on = true;
            data.play_intro = true;
        }

        let stream_arc = get_message_stream();
        stream_arc
            .write()
            .expect("message stream lock should succeed")
            .clear_messages();

        handle_initial_file_startup();

        let data = global.read();
        assert_eq!(data.pending_file, "Maps\\TestMap\\TestMap.map");
        assert!(!data.shell_map_on);
        assert!(!data.play_intro);
        drop(data);

        let stream = stream_arc
            .read()
            .expect("message stream lock should succeed");
        assert!(stream.contains_message_of_type(&GameMessageType::NewGame));
    }

    #[tokio::test]
    async fn test_update_frame_skips_game_logic_when_paused_and_offline() {
        let updates = Arc::new(AtomicUsize::new(0));
        let mut engine = GameEngine::new();
        engine.current_state = GameState::Paused;
        engine.game_logic = Some(Box::new(CountingGameLogic::new(updates.clone())));
        engine.network_interface = None;

        engine
            .update_frame(Duration::from_millis(16))
            .await
            .expect("update frame should succeed");

        assert_eq!(updates.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_update_frame_updates_game_logic_when_network_frame_ready_even_paused() {
        let updates = Arc::new(AtomicUsize::new(0));
        let mut engine = GameEngine::new();
        engine.current_state = GameState::Paused;
        engine.game_logic = Some(Box::new(CountingGameLogic::new(updates.clone())));
        engine.network_interface = Some(Box::new(StubNetwork::new(true)));

        engine
            .update_frame(Duration::from_millis(16))
            .await
            .expect("update frame should succeed");

        assert_eq!(updates.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_should_apply_frame_limit_honors_cpp_tivo_replay_gate() {
        assert!(!GameEngine::should_apply_frame_limit(true, 30, true, true));
        assert!(GameEngine::should_apply_frame_limit(true, 30, true, false));
        assert!(!GameEngine::should_apply_frame_limit(
            false, 30, false, false
        ));
        assert!(!GameEngine::should_apply_frame_limit(true, 0, false, false));
    }

    #[test]
    fn test_is_multiplayer_session_uses_recorder_state_like_cpp() {
        let _ = init_recorder();
        with_recorder_mut(|recorder| recorder.set_game_mode_provider(Some(Arc::new(|| 2))));

        let engine = GameEngine::new();
        assert!(engine.is_multiplayer_session());

        with_recorder_mut(|recorder| recorder.set_game_mode_provider(Some(Arc::new(|| 0))));
        assert!(!engine.is_multiplayer_session());
    }
}
