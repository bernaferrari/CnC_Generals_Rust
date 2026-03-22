use crate::config::GlobalData;
use crate::input_system::InputSystem;
use anyhow::{anyhow, Result};
use game_engine::common::message_stream::{
    get_message_stream, GameMessageType as MessageStreamGameMessageType,
};
use game_engine::common::system::{
    big_file_system::BigArchiveBackend, file_system::get_file_system,
    local_file_system::LocalFileSystem,
    subsystem_interface::SubsystemInterface as CommonSubsystemInterface,
};
use log::{debug, error, info, warn};
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;
use ww3d_engine::FrameTiming;

/// Subsystem interface trait - matches C++ SubsystemInterface
pub trait SubsystemInterface: Send + Sync + Any {
    /// Initialize the subsystem
    fn init(&mut self) -> Result<()>;

    /// Reset the subsystem (for new games/maps)
    fn reset(&mut self) -> Result<()>;

    /// Update the subsystem (called each frame)
    fn update(&mut self, dt: f32) -> Result<()>;

    /// Update hook that includes the full frame timing (defaults to `update`)
    fn update_with_timing(&mut self, timing: &FrameTiming) -> Result<()> {
        self.update(timing.delta_seconds())
    }

    /// Shutdown the subsystem
    fn shutdown(&mut self) -> Result<()>;

    /// Get subsystem name for debugging
    fn name(&self) -> &'static str;

    /// Post-process loading (called after all subsystems are loaded)
    fn post_process_load(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Initialize shell/menu-facing INI scheme managers at the UI handoff point.
///
/// This matches the C++ flow more closely than doing it during `GlobalData`
/// loading: the shell scheme files are read when the shell/UI layer comes up,
/// not while the core data subsystem is still being initialized.
pub fn initialize_shell_ui_schemes() {
    game_engine::common::ini::ini_control_bar_scheme::initialize_control_bar_scheme_manager();
    game_engine::common::ini::ini_shell_menu_scheme::init_shell_menu_scheme_manager();
}

/// File System subsystem - manages BIG files and local files
pub struct FileSystemSubsystem {
    archive_system: Option<crate::assets::archive::ArchiveFileSystem>,
    local_file_system: Option<crate::assets::LocalFileSystem>,
    initialized: bool,
}

impl FileSystemSubsystem {
    pub fn new() -> Self {
        Self {
            archive_system: None,
            local_file_system: None,
            initialized: false,
        }
    }

    pub fn get_archive_system(&self) -> Option<&crate::assets::archive::ArchiveFileSystem> {
        self.archive_system.as_ref()
    }
}

impl SubsystemInterface for FileSystemSubsystem {
    fn name(&self) -> &'static str {
        "FileSystem"
    }

    fn init(&mut self) -> Result<()> {
        info!("Initializing FileSystem subsystem");

        // Initialize the shared Common file system used by INI/map/texture loaders.
        let mut search_paths = vec![
            PathBuf::from("."),
            PathBuf::from("Data"),
            PathBuf::from("Art"),
            PathBuf::from("Code/Main/assets"),
            PathBuf::from("GeneralsRust/Code/Main/assets"),
            PathBuf::from("windows_game"),
            PathBuf::from("windows_game/Command & Conquer Generals Zero Hour"),
            PathBuf::from("windows_game/Command & Conquer Generals Zero Hour/Data"),
            PathBuf::from("windows_game/Command & Conquer Generals"),
            PathBuf::from("windows_game/Command & Conquer Generals/Data"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
        ];

        if let Ok(cwd) = std::env::current_dir() {
            search_paths.push(cwd.clone());
            search_paths.push(cwd.join("Data"));
            search_paths.push(cwd.join("Art"));
            search_paths.push(cwd.join("Code/Main/assets"));
            search_paths.push(cwd.join("GeneralsRust/Code/Main/assets"));
            search_paths.push(cwd.join("windows_game"));
            search_paths.push(cwd.join("windows_game/Command & Conquer Generals Zero Hour"));
            search_paths.push(cwd.join("windows_game/Command & Conquer Generals Zero Hour/Data"));
            search_paths.push(cwd.join("windows_game/Command & Conquer Generals"));
            search_paths.push(cwd.join("windows_game/Command & Conquer Generals/Data"));
        }

        let mut deduped = Vec::new();
        let mut seen = HashSet::new();
        for path in search_paths {
            let key = path
                .to_string_lossy()
                .replace('\\', "/")
                .to_ascii_lowercase();
            if seen.insert(key) {
                deduped.push(path);
            }
        }

        {
            let file_system = get_file_system();
            let mut fs_guard = file_system
                .lock()
                .map_err(|_| anyhow!("Failed to lock Common FileSystem"))?;

            {
                let local_backend: &mut LocalFileSystem =
                    fs_guard.ensure_backend(LocalFileSystem::new);
                for path in &deduped {
                    local_backend.add_search_path(path);
                }
            }

            {
                let big_backend: &mut BigArchiveBackend =
                    fs_guard.ensure_backend(BigArchiveBackend::new);
                for path in &deduped {
                    big_backend.add_search_path(path);
                }
            }

            fs_guard.clear_cache();
            let _ = CommonSubsystemInterface::init(&mut *fs_guard);

            if fs_guard.does_file_exist("INIZH.big") {
                info!("Common FileSystem: INIZH.big detected");
            } else {
                warn!("Common FileSystem: INIZH.big not detected in configured search paths");
            }
        }

        // Create local file system first (matches C++ order)
        let local_fs = crate::assets::LocalFileSystem::new();
        self.local_file_system = Some(local_fs);

        // Create and initialize archive system
        let archive_sys = crate::assets::archive::ArchiveFileSystem::new();
        // Initialize asynchronously - in production this would be handled differently
        // For now, mark as initialized and handle async loading separately
        self.archive_system = Some(archive_sys);

        self.initialized = true;
        info!("FileSystem subsystem initialized");
        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        info!("Resetting FileSystem subsystem");
        // FileSystem typically doesn't need resetting
        Ok(())
    }

    fn update(&mut self, _dt: f32) -> Result<()> {
        // FileSystem is typically passive, no updates needed
        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down FileSystem subsystem");
        self.archive_system = None;
        self.local_file_system = None;
        self.initialized = false;
        Ok(())
    }
}

/// Global Data subsystem - manages game configuration and data
pub struct GlobalDataSubsystem {
    ini_crc: u32,
}

impl GlobalDataSubsystem {
    pub fn new() -> Self {
        Self { ini_crc: 0 }
    }
}

impl SubsystemInterface for GlobalDataSubsystem {
    fn name(&self) -> &'static str {
        "GlobalData"
    }

    fn init(&mut self) -> Result<()> {
        info!("Initializing GlobalData subsystem");

        // Load game configuration data
        let mut global_data = GlobalData::new();

        // Load default and override INI files (matches C++ pattern)
        global_data.load_ini("Data/INI/Default/GameData.ini")?;
        global_data.load_ini("Data/INI/GameData.ini")?;

        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            // Load debug configuration in debug/internal builds
            global_data.load_ini("Data/INI/GameDataDebug.ini").ok(); // Allow failure
        }

        self.ini_crc = global_data.calculate_crc();
        global_data.ini_crc = self.ini_crc;
        global_data.sync_runtime_view();

        info!(
            "GlobalData subsystem initialized (INI CRC: {:08X})",
            self.ini_crc
        );
        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        info!("Resetting GlobalData subsystem");
        // Global data typically doesn't need resetting
        Ok(())
    }

    fn update(&mut self, _dt: f32) -> Result<()> {
        // Global data is passive
        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down GlobalData subsystem");
        Ok(())
    }
}

/// Audio Manager subsystem - handles all audio
pub struct AudioManagerSubsystem {
    audio_manager: Option<crate::assets::audio::AudioManager>,
    _music_on: bool,
    _sounds_on: bool,
    _speech_on: bool,
    queued_events: Vec<crate::game_logic::AudioEventRequest>,
    sound_effects_table: Option<crate::assets::SoundEffectsTable>,
}

impl AudioManagerSubsystem {
    pub fn new() -> Self {
        Self {
            audio_manager: None,
            _music_on: true,
            _sounds_on: true,
            _speech_on: true,
            queued_events: Vec::new(),
            sound_effects_table: None,
        }
    }

    pub fn queue_event(&mut self, event: crate::game_logic::AudioEventRequest) {
        self.queued_events.push(event);
    }

    fn drain_events(&mut self) -> Vec<crate::game_logic::AudioEventRequest> {
        self.queued_events.drain(..).collect()
    }
}

impl SubsystemInterface for AudioManagerSubsystem {
    fn name(&self) -> &'static str {
        "AudioManager"
    }

    fn init(&mut self) -> Result<()> {
        info!("Initializing AudioManager subsystem");
        self.sound_effects_table = crate::assets::SoundEffectsTable::load_default();
        if let Some(table) = self.sound_effects_table.as_ref() {
            if table.is_empty() {
                self.sound_effects_table = None;
            }
        }

        match crate::assets::audio::AudioManager::new() {
            Ok(audio_manager) => {
                self.audio_manager = Some(audio_manager);
                info!("AudioManager subsystem initialized successfully");
                Ok(())
            }
            Err(e) => {
                warn!(
                    "Failed to initialize audio: {}. Game will continue without audio.",
                    e
                );
                // Don't fail initialization if audio fails - matches C++ behavior
                Ok(())
            }
        }
    }

    fn reset(&mut self) -> Result<()> {
        info!("Resetting AudioManager subsystem");
        // Stop all current sounds/music but keep the audio system active
        if let Some(audio_manager) = &mut self.audio_manager {
            audio_manager.stop_all_sounds();
        }
        Ok(())
    }

    fn update(&mut self, _dt: f32) -> Result<()> {
        // Apply high-level toggles/events that don't require archive lookups yet.
        // Detailed EVA/SFX playback will be handled once the sound event tables are wired.
        for event in self.drain_events() {
            match event.event_type.as_str() {
                "MusicDisable" => {
                    self._music_on = false;
                    if let Some(audio_manager) = &mut self.audio_manager {
                        audio_manager.pause_audio(crate::assets::AudioAffect::Music);
                    }
                }
                "MusicEnable" => {
                    self._music_on = true;
                    if let Some(audio_manager) = &mut self.audio_manager {
                        audio_manager.resume_audio(crate::assets::AudioAffect::Music);
                    }
                }
                _ => {
                    if !self._sounds_on {
                        continue;
                    }

                    let Some(table) = self.sound_effects_table.as_ref() else {
                        continue;
                    };

                    let Some(sound_path) = table.resolve_sound_path(event.event_type.as_str())
                    else {
                        continue;
                    };

                    if let Ok(handle) = tokio::runtime::Handle::try_current() {
                        tokio::task::block_in_place(|| {
                            let _ = handle.block_on(crate::assets::manager::play_cnc_sound_effect(
                                &sound_path,
                            ));
                        });
                    }
                }
            }
        }

        if let Some(audio_manager) = &mut self.audio_manager {
            audio_manager.update();
        }
        Ok(())
    }

    fn update_with_timing(&mut self, timing: &FrameTiming) -> Result<()> {
        if let Some(audio_manager) = &mut self.audio_manager {
            audio_manager.update_with_time(timing.delta_seconds(), timing.total_seconds());
        }
        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down AudioManager subsystem");
        self.audio_manager = None;
        Ok(())
    }
}

/// Radar subsystem - minimap and radar display (matches C++ TheRadar)
pub struct RadarSubsystem {
    initialized: bool,
}

impl RadarSubsystem {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Default for RadarSubsystem {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for RadarSubsystem {
    fn name(&self) -> &'static str {
        "Radar"
    }

    fn init(&mut self) -> Result<()> {
        info!("Initializing Radar subsystem");
        self.initialized = true;
        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        Ok(())
    }

    fn update(&mut self, _dt: f32) -> Result<()> {
        // Radar updates are handled by gamelogic radar notifier
        Ok(())
    }

    fn update_with_timing(&mut self, timing: &FrameTiming) -> Result<()> {
        self.update(timing.delta_seconds())
    }

    fn shutdown(&mut self) -> Result<()> {
        self.initialized = false;
        Ok(())
    }
}

/// GameClient subsystem - client-side rendering and drawables (matches C++ TheGameClient)
pub struct GameClientSubsystem {
    initialized: bool,
    frame: u32,
}

impl GameClientSubsystem {
    pub fn new() -> Self {
        Self {
            initialized: false,
            frame: 0,
        }
    }

    fn update_frame_tick(&self) -> Result<()> {
        let stream = get_message_stream();
        let mut stream_guard = stream
            .write()
            .map_err(|e| anyhow!("Failed to lock MessageStream: {}", e))?;
        let frame_msg =
            stream_guard.append_message(MessageStreamGameMessageType::FrameTick(self.frame));
        frame_msg.append_timestamp_argument(self.frame);
        Ok(())
    }

    #[cfg(feature = "game_client")]
    fn update_legacy_client_singletons(&self) {
        use game_client::core::script_action_handler::apply_pending_script_display_state;
        use game_client::eva::update_eva_system;
        use game_client::gui::{
            get_display_string_manager, get_shell, window_video_manager::with_window_video_manager,
            with_window_manager,
        };
        use game_client::system::SubsystemInterface as GameClientSubsystemInterface;
        use game_client::video_player::{get_video_player, VideoPlayerInterface as _};

        update_eva_system();
        with_window_video_manager(|manager| manager.update());
        with_window_manager(|manager| manager.update());

        if let Some(video_player) = get_video_player() {
            if let Ok(mut guard) = video_player.lock() {
                if let Some(player) = guard.as_mut() {
                    player.update();
                }
            }
        }

        if let Err(err) = get_display_string_manager().update() {
            warn!("Display string manager update failed: {}", err);
        }

        apply_pending_script_display_state();

        let mut shell = get_shell();
        if let Err(err) = GameClientSubsystemInterface::update(&mut *shell) {
            warn!("Shell update failed: {}", err);
        }
    }

    #[cfg(not(feature = "game_client"))]
    fn update_legacy_client_singletons(&self) {}

    fn update_internal(&mut self, advance_frame: bool) -> Result<()> {
        if advance_frame {
            self.frame = self.frame.wrapping_add(1);
        }

        self.update_frame_tick()?;
        self.update_legacy_client_singletons();
        Ok(())
    }
}

impl Default for GameClientSubsystem {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for GameClientSubsystem {
    fn name(&self) -> &'static str {
        "GameClient"
    }

    fn init(&mut self) -> Result<()> {
        info!("Initializing GameClient subsystem");
        self.initialized = true;
        self.frame = 0;
        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        self.frame = 0;
        Ok(())
    }

    fn update(&mut self, _dt: f32) -> Result<()> {
        self.update_internal(true)
    }

    fn update_with_timing(&mut self, timing: &FrameTiming) -> Result<()> {
        self.frame = timing.frame_number as u32;
        self.update_internal(false)
    }

    fn shutdown(&mut self) -> Result<()> {
        self.initialized = false;
        Ok(())
    }
}

/// Message Stream subsystem - game communication system
/// This wraps the actual game MessageStream singleton from Common
pub struct MessageStreamSubsystem {
    // Reference to the global message stream is obtained via get_message_stream()
    initialized: bool,
}

impl MessageStreamSubsystem {
    pub fn new() -> Self {
        Self { initialized: false }
    }

    /// Propagate messages through all translators
    /// This is the critical function that matches C++ MessageStream::propagateMessages()
    pub fn propagate_messages(&mut self) -> Result<()> {
        use game_engine::common::message_stream::get_message_stream;

        let stream = get_message_stream();
        let mut stream_guard = stream
            .write()
            .map_err(|e| anyhow!("Failed to lock MessageStream: {}", e))?;

        // Call propagate_messages which processes all messages through translators
        // and transfers them to TheCommandList (matching C++ behavior)
        let _completed = stream_guard
            .propagate_messages()
            .map_err(|e| anyhow!("Message propagation failed: {:?}", e))?;

        Ok(())
    }
}

impl SubsystemInterface for MessageStreamSubsystem {
    fn name(&self) -> &'static str {
        "MessageStream"
    }

    fn init(&mut self) -> Result<()> {
        info!("Initializing MessageStream subsystem");
        use game_engine::common::message_stream::get_message_stream;

        let stream = get_message_stream();
        let mut stream_guard = stream
            .write()
            .map_err(|e| anyhow!("Failed to lock MessageStream: {}", e))?;

        // Initialize the underlying message stream
        // Note: game_engine's MessageStream has its own init() method via SubsystemInterface
        // but we can't call it directly due to trait mismatch. Just clear messages instead.
        stream_guard.clear_messages();

        self.initialized = true;
        info!("MessageStream subsystem initialized successfully");
        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        info!("Resetting MessageStream subsystem");
        use game_engine::common::message_stream::get_message_stream;

        let stream = get_message_stream();
        let mut stream_guard = stream
            .write()
            .map_err(|e| anyhow!("Failed to lock MessageStream: {}", e))?;

        // Reset the underlying message stream
        stream_guard.clear_messages();
        // Clear translators too if needed

        Ok(())
    }

    fn update(&mut self, _dt: f32) -> Result<()> {
        // This is the critical update - matches C++ TheMessageStream->propagateMessages()
        self.propagate_messages()
    }

    fn update_with_timing(&mut self, _timing: &FrameTiming) -> Result<()> {
        self.update(0.0)
    }

    fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down MessageStream subsystem");
        use game_engine::common::message_stream::get_message_stream;

        let stream = get_message_stream();
        let mut stream_guard = stream
            .write()
            .map_err(|e| anyhow!("Failed to lock MessageStream: {}", e))?;
        stream_guard.clear_messages();

        self.initialized = false;
        Ok(())
    }
}

/// Network subsystem - network communication (matches C++ TheNetwork)
pub struct NetworkSubsystem {
    initialized: bool,
    active_session: bool,
    frame_data_ready: bool,
}

impl NetworkSubsystem {
    pub fn new() -> Self {
        Self {
            initialized: false,
            active_session: false,
            frame_data_ready: true,
        }
    }

    /// C++ parity helper: mirrors whether `TheNetwork` exists for active gameplay sync.
    pub fn has_active_session(&self) -> bool {
        self.initialized && self.active_session
    }

    /// C++ parity helper: mirrors `TheNetwork->isFrameDataReady()`.
    pub fn is_frame_data_ready(&self) -> bool {
        self.frame_data_ready
    }

    /// Updates active-session and frame-ready state from the networking layer.
    pub fn set_session_state(&mut self, active_session: bool, frame_data_ready: bool) {
        self.active_session = active_session;
        self.frame_data_ready = if active_session {
            frame_data_ready
        } else {
            true
        };
    }

    pub fn set_active_session(&mut self, active_session: bool) {
        self.active_session = active_session;
        if !active_session {
            self.frame_data_ready = true;
        }
    }

    pub fn set_frame_data_ready(&mut self, frame_data_ready: bool) {
        self.frame_data_ready = frame_data_ready;
    }
}

impl Default for NetworkSubsystem {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for NetworkSubsystem {
    fn name(&self) -> &'static str {
        "Network"
    }

    fn init(&mut self) -> Result<()> {
        info!("Initializing Network subsystem");
        self.initialized = true;
        self.active_session = false;
        self.frame_data_ready = true;
        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        self.active_session = false;
        self.frame_data_ready = true;
        Ok(())
    }

    fn update(&mut self, _dt: f32) -> Result<()> {
        #[cfg(feature = "network")]
        {
            // Network update would be called here
        }
        if !self.active_session {
            // No active network session mirrors C++ `TheNetwork == NULL`.
            self.frame_data_ready = true;
        }
        Ok(())
    }

    fn update_with_timing(&mut self, timing: &FrameTiming) -> Result<()> {
        self.update(timing.delta_seconds())
    }

    fn shutdown(&mut self) -> Result<()> {
        self.initialized = false;
        self.active_session = false;
        self.frame_data_ready = true;
        Ok(())
    }
}

/// CD Manager subsystem - CD/DVD drive management (legacy, for C++ parity)
pub struct CDManagerSubsystem {
    initialized: bool,
}

impl CDManagerSubsystem {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Default for CDManagerSubsystem {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for CDManagerSubsystem {
    fn name(&self) -> &'static str {
        "CDManager"
    }

    fn init(&mut self) -> Result<()> {
        info!("Initializing CDManager subsystem");
        self.initialized = true;
        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        Ok(())
    }

    fn update(&mut self, _dt: f32) -> Result<()> {
        // CD management is legacy - no-op in modern implementation
        Ok(())
    }

    fn update_with_timing(&mut self, timing: &FrameTiming) -> Result<()> {
        self.update(timing.delta_seconds())
    }

    fn shutdown(&mut self) -> Result<()> {
        self.initialized = false;
        Ok(())
    }
}

/// GameLogic subsystem - main game simulation (matches C++ TheGameLogic)
pub struct GameLogicSubsystem {
    initialized: bool,
    frame: u32,
}

impl GameLogicSubsystem {
    pub fn new() -> Self {
        Self {
            initialized: false,
            frame: 0,
        }
    }
}

impl Default for GameLogicSubsystem {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for GameLogicSubsystem {
    fn name(&self) -> &'static str {
        "GameLogic"
    }

    fn init(&mut self) -> Result<()> {
        info!("Initializing GameLogic subsystem");
        self.initialized = true;
        self.frame = 0;
        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        self.frame = 0;
        Ok(())
    }

    fn update(&mut self, _dt: f32) -> Result<()> {
        self.frame = self.frame.wrapping_add(1);
        // Actual GameLogic update is done via gamelogic::system::update_game_logic()
        // This is called separately after message propagation
        Ok(())
    }

    fn update_with_timing(&mut self, timing: &FrameTiming) -> Result<()> {
        self.frame = timing.frame_number as u32;
        self.update(timing.delta_seconds())
    }

    fn shutdown(&mut self) -> Result<()> {
        self.initialized = false;
        Ok(())
    }
}

/// Game message structure (deprecated - use game_engine::common::message_stream::GameMessage)
#[derive(Debug, Clone)]
#[deprecated(note = "Use game_engine::common::message_stream::game_message::GameMessage instead")]
pub struct GameMessage {
    pub message_type: GameMessageType,
    pub arguments: Vec<GameMessageArgument>,
}

#[derive(Debug, Clone)]
pub enum GameMessageType {
    NewGame,
    LoadGame,
    SaveGame,
    CommandMove,
    CommandAttack,
    CommandBuild,
    GamePaused,
    GameResumed,
    UnitCreated,
    UnitDestroyed,
    MetaInstantQuit,
    MetaOptions,
    Custom(String),
}

#[derive(Debug, Clone)]
pub enum GameMessageArgument {
    Integer(i32),
    Float(f32),
    String(String),
    Position(f32, f32, f32),
}

/// Input System subsystem - keyboard and mouse handling
pub struct InputSystemSubsystem {
    input_system: Option<InputSystem>,
}

impl InputSystemSubsystem {
    pub fn new() -> Self {
        Self { input_system: None }
    }
}

impl SubsystemInterface for InputSystemSubsystem {
    fn name(&self) -> &'static str {
        "InputSystem"
    }

    fn init(&mut self) -> Result<()> {
        info!("Initializing InputSystem subsystem");
        self.input_system = Some(InputSystem::new());
        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        info!("Resetting InputSystem subsystem");
        if let Some(input_system) = &mut self.input_system {
            input_system.reset();
        }
        Ok(())
    }

    fn update(&mut self, dt: f32) -> Result<()> {
        if let Some(input_system) = &mut self.input_system {
            input_system.update(dt);
        }
        Ok(())
    }

    fn update_with_timing(&mut self, timing: &FrameTiming) -> Result<()> {
        if let Some(input_system) = &mut self.input_system {
            input_system.update_with_timing(timing);
        }
        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down InputSystem subsystem");
        self.input_system = None;
        Ok(())
    }
}

trait SubsystemStorage: Send {
    fn name(&self) -> &'static str;
    fn type_id(&self) -> TypeId;
    fn interface(&self) -> &dyn SubsystemInterface;
    fn interface_mut(&mut self) -> &mut dyn SubsystemInterface;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn init(&mut self) -> Result<()>;
    fn reset(&mut self) -> Result<()>;
    fn update(&mut self, dt: f32) -> Result<()>;
    fn update_with_timing(&mut self, timing: &FrameTiming) -> Result<()> {
        self.update(timing.delta_seconds())
    }
    fn shutdown(&mut self) -> Result<()>;
    fn post_process_load(&mut self) -> Result<()>;
}

struct TypedSubsystemStorage<T: SubsystemInterface + 'static> {
    subsystem: T,
}

impl<T: SubsystemInterface + 'static> TypedSubsystemStorage<T> {
    fn new(subsystem: T) -> Self {
        Self { subsystem }
    }
}

impl<T: SubsystemInterface + 'static> SubsystemStorage for TypedSubsystemStorage<T> {
    fn name(&self) -> &'static str {
        self.subsystem.name()
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn interface(&self) -> &dyn SubsystemInterface {
        &self.subsystem
    }

    fn interface_mut(&mut self) -> &mut dyn SubsystemInterface {
        &mut self.subsystem
    }

    fn as_any(&self) -> &dyn Any {
        &self.subsystem
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        &mut self.subsystem
    }

    fn init(&mut self) -> Result<()> {
        self.subsystem.init()
    }

    fn reset(&mut self) -> Result<()> {
        self.subsystem.reset()
    }

    fn update(&mut self, dt: f32) -> Result<()> {
        self.subsystem.update(dt)
    }

    fn update_with_timing(&mut self, timing: &FrameTiming) -> Result<()> {
        self.subsystem.update_with_timing(timing)
    }

    fn shutdown(&mut self) -> Result<()> {
        self.subsystem.shutdown()
    }

    fn post_process_load(&mut self) -> Result<()> {
        self.subsystem.post_process_load()
    }
}

#[derive(Copy, Clone, Debug)]
pub struct SubsystemHandle<T> {
    index: usize,
    type_id: TypeId,
    _marker: PhantomData<T>,
}

impl<T: 'static> SubsystemHandle<T> {
    fn new(index: usize) -> Self {
        Self {
            index,
            type_id: TypeId::of::<T>(),
            _marker: PhantomData,
        }
    }

    pub fn get<'a>(&self, manager: &'a SubsystemManager) -> Option<&'a T> {
        manager
            .subsystems
            .get(self.index)
            .and_then(|slot| slot.as_any().downcast_ref::<T>())
    }

    pub fn get_mut<'a>(&self, manager: &'a mut SubsystemManager) -> Option<&'a mut T> {
        if self.index >= manager.subsystems.len() {
            return None;
        }
        let slot_ptr: *mut dyn SubsystemStorage = manager.subsystems[self.index].as_mut();
        unsafe {
            let slot = &mut *slot_ptr;
            slot.as_any_mut().downcast_mut::<T>()
        }
    }
}

pub struct SubsystemManager {
    subsystems: Vec<Box<dyn SubsystemStorage>>,
    indices_by_type: HashMap<TypeId, usize>,
    indices_by_name: HashMap<&'static str, usize>,
    initialization_order: Vec<&'static str>,
    initialized: bool,
    start_time: Option<SystemTime>,
}

impl SubsystemManager {
    pub fn new() -> Self {
        // Define startup initialization order matching the C++ GameEngine::init() sequence.
        // The runtime update order still follows the subsystem registration order below,
        // which mirrors the C++ GameEngine::update() cadence for the active gameplay systems.
        let initialization_order = vec![
            "FileSystem",    // File system must be first
            "GlobalData",    // Load INI configuration
            "CDManager",     // Legacy CD/DVD subsystem
            "AudioManager",  // Audio subsystem
            "MessageStream", // Message propagation
            "GameClient",    // Game client (drawables, effects)
            "InputSystem",   // Input handling
            "GameLogic",     // Game logic
            "Radar",         // Radar/minimap is initialized after gameplay systems
            "Network",       // Network layer is kept last for startup readiness gating
        ];

        Self {
            subsystems: Vec::new(),
            indices_by_type: HashMap::new(),
            indices_by_name: HashMap::new(),
            initialization_order,
            initialized: false,
            start_time: None,
        }
    }

    /// Add a subsystem to be managed, returning a handle for typed access.
    pub fn add_subsystem<T: SubsystemInterface + 'static>(
        &mut self,
        subsystem: T,
    ) -> SubsystemHandle<T> {
        let slot = Box::new(TypedSubsystemStorage::new(subsystem));
        let name = slot.name();
        // Index by the concrete subsystem type to guarantee handle_for/get/get_mut parity.
        let type_id = TypeId::of::<T>();
        self.subsystems.push(slot);
        let index = self.subsystems.len() - 1;
        self.indices_by_name.insert(name, index);
        self.indices_by_type.insert(type_id, index);
        SubsystemHandle::new(index)
    }

    pub fn get_interface_by_name(&self, name: &'static str) -> Option<&dyn SubsystemInterface> {
        self.indices_by_name
            .get(name)
            .and_then(|&idx| self.subsystems.get(idx).map(|slot| slot.interface()))
    }

    pub fn get_interface_by_name_mut(
        &mut self,
        name: &'static str,
    ) -> Option<&mut dyn SubsystemInterface> {
        if let Some(&idx) = self.indices_by_name.get(name) {
            return self
                .subsystems
                .get_mut(idx)
                .map(|slot| slot.interface_mut());
        }
        None
    }

    /// Initialize all subsystems in the correct order
    pub fn initialize_all(&mut self) -> Result<()> {
        info!("Starting subsystem initialization sequence");
        self.start_time = Some(SystemTime::now());
        self.initialized = false;

        let mut initialized = HashSet::new();

        // Initialize in the predefined order
        for &target_name in &self.initialization_order {
            let start_time = SystemTime::now();
            match self.indices_by_name.get(target_name).copied() {
                Some(index) => {
                    let slot = self
                        .subsystems
                        .get_mut(index)
                        .expect("subsystem index must be valid");

                    info!("Initializing subsystem: {}", target_name);

                    if let Err(e) = slot.init() {
                        error!("Failed to initialize subsystem {}: {}", target_name, e);
                        return Err(anyhow!(
                            "Subsystem {} initialization failed: {}",
                            target_name,
                            e
                        ));
                    }

                    let duration = start_time.elapsed().unwrap_or_default();
                    info!(
                        "✅ {} initialized in {:.2}ms",
                        target_name,
                        duration.as_millis()
                    );
                    initialized.insert(index);
                }
                None => {
                    let err = anyhow!("Subsystem {} not found during initialization", target_name);
                    error!("{}", err);
                    return Err(err);
                }
            }
        }

        // Initialize any remaining subsystems not in the order list
        for (index, slot) in self.subsystems.iter_mut().enumerate() {
            if initialized.contains(&index) {
                continue;
            }
            let name = slot.name();
            info!("Initializing additional subsystem: {}", name);
            if let Err(e) = slot.init() {
                error!("Failed to initialize additional subsystem {}: {}", name, e);
                return Err(anyhow!(
                    "Additional subsystem {} initialization failed: {}",
                    name,
                    e
                ));
            }
        }

        // Post-process loading phase
        info!("Running post-process loading for all subsystems");
        for slot in &mut self.subsystems {
            let name = slot.name();
            if let Err(e) = slot.post_process_load() {
                error!("Post-process loading failed for {}: {}", name, e);
                return Err(anyhow!(
                    "Subsystem {} post-process loading failed: {}",
                    name,
                    e
                ));
            }
        }

        self.initialized = true;
        let total_time = self
            .start_time
            .and_then(|start| start.elapsed().ok())
            .unwrap_or_default();
        info!(
            "✅ All subsystems initialized successfully in {:.2}ms",
            total_time.as_millis()
        );

        Ok(())
    }

    /// Reset all subsystems (for new games)
    pub fn reset_all(&mut self) -> Result<()> {
        info!("Resetting all subsystems");

        for slot in &mut self.subsystems {
            let name = slot.name();
            if let Err(e) = slot.reset() {
                error!("Failed to reset subsystem {}: {}", name, e);
                // Continue with other subsystems even if one fails
            }
        }

        info!("All subsystems reset");
        Ok(())
    }

    /// Update all subsystems
    pub fn update_all(&mut self, dt: f32) -> Result<()> {
        for slot in &mut self.subsystems {
            let name = slot.name();
            if let Err(e) = slot.update(dt) {
                error!("Error updating subsystem {}: {}", name, e);
                // Continue updating other subsystems
            }
        }
        Ok(())
    }

    /// Update all subsystems with full frame timing information.
    pub fn update_all_with_timing(&mut self, timing: &FrameTiming) -> Result<()> {
        for slot in &mut self.subsystems {
            let name = slot.name();
            if let Err(e) = slot.update_with_timing(timing) {
                error!("Error updating subsystem {}: {}", name, e);
            }
        }
        Ok(())
    }

    /// Obtain a typed handle to a subsystem, if registered.
    pub fn handle_for<T: SubsystemInterface + 'static>(&self) -> Option<SubsystemHandle<T>> {
        self.indices_by_type
            .get(&TypeId::of::<T>())
            .copied()
            .map(SubsystemHandle::new)
    }

    /// Borrow a subsystem immutably by type.
    pub fn get<T: SubsystemInterface + 'static>(&self) -> Option<&T> {
        self.indices_by_type
            .get(&TypeId::of::<T>())
            .and_then(|&index| self.subsystems.get(index))
            .and_then(|slot| slot.as_any().downcast_ref::<T>())
    }

    /// Borrow a subsystem mutably by type.
    pub fn get_mut<T: SubsystemInterface + 'static>(&mut self) -> Option<&mut T> {
        self.indices_by_type
            .get(&TypeId::of::<T>())
            .copied()
            .and_then(move |index| self.subsystems.get_mut(index))
            .and_then(|slot| slot.as_any_mut().downcast_mut::<T>())
    }

    /// Shutdown all subsystems
    pub fn shutdown_all(&mut self) -> Result<()> {
        info!("Shutting down all subsystems");

        // Shutdown in reverse order
        for slot in self.subsystems.iter_mut().rev() {
            let name = slot.name();
            info!("Shutting down subsystem: {}", name);
            if let Err(e) = slot.shutdown() {
                error!("Error shutting down subsystem {}: {}", name, e);
            }
        }

        self.subsystems.clear();
        self.indices_by_name.clear();
        self.indices_by_type.clear();
        self.start_time = None;
        self.initialized = false;
        info!("All subsystems shut down");
        Ok(())
    }

    /// Check if all subsystems are initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get initialization statistics
    pub fn get_stats(&self) -> SubsystemStats {
        SubsystemStats {
            total_subsystems: self.subsystems.len(),
            initialized: self.initialized,
            initialization_time: self.start_time.and_then(|t| t.elapsed().ok()),
        }
    }

    /// Notify all subsystems of focus change (for graphics device reset equivalent)
    pub fn notify_focus_change(&mut self, active: bool) -> Result<()> {
        info!(
            "📡 Notifying all subsystems of focus change: {}",
            if active { "active" } else { "inactive" }
        );

        if !active {
            if let Some(input_system) = self.get_mut::<InputSystemSubsystem>() {
                if let Err(err) = input_system.reset() {
                    warn!("Failed to reset input system during focus loss: {}", err);
                }
            }
        }

        Ok(())
    }

    /// Notify audio subsystem of focus change (matches TheAudio->loseFocus/regainFocus)
    pub fn notify_audio_focus_change(&mut self, active: bool) -> Result<()> {
        info!(
            "🔊 Notifying audio subsystem of focus change: {}",
            if active { "gained" } else { "lost" }
        );

        if self.get::<AudioManagerSubsystem>().is_some() {
            // In a complete implementation, AudioManager would implement focus handling
            // This could pause/resume audio based on focus state
            info!("Audio focus change handled by AudioManager subsystem");
        } else {
            debug!("Audio subsystem not available during focus change notification");
        }

        Ok(())
    }
}

/// Subsystem statistics
#[derive(Debug)]
pub struct SubsystemStats {
    pub total_subsystems: usize,
    pub initialized: bool,
    pub initialization_time: Option<std::time::Duration>,
}

/// Global subsystem manager instance stored in a thread-safe wrapper.
static SUBSYSTEM_MANAGER: OnceLock<Arc<Mutex<SubsystemManager>>> = OnceLock::new();

/// Lightweight handle used to acquire locked access to the subsystem manager.
#[derive(Clone)]
pub struct SubsystemManagerHandle {
    inner: Arc<Mutex<SubsystemManager>>,
}

impl SubsystemManagerHandle {
    fn new(inner: Arc<Mutex<SubsystemManager>>) -> Self {
        Self { inner }
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, SubsystemManager> {
        self.inner.lock().expect("SubsystemManager mutex poisoned")
    }
}

/// Execute a closure with an immutable borrow of a registered subsystem.
pub fn with_subsystem<T, R>(f: impl FnOnce(&T) -> R) -> Option<R>
where
    T: SubsystemInterface + 'static,
{
    let handle = SUBSYSTEM_MANAGER.get()?.clone();
    let manager = handle.lock().ok()?;
    let subsystem_handle = manager.handle_for::<T>()?;
    let subsystem = subsystem_handle.get(&manager)?;
    Some(f(subsystem))
}

/// Execute a closure with a mutable borrow of a registered subsystem.
pub fn with_subsystem_mut<T, R>(f: impl FnOnce(&mut T) -> R) -> Option<R>
where
    T: SubsystemInterface + 'static,
{
    let handle = SUBSYSTEM_MANAGER.get()?.clone();
    let mut manager = handle.lock().ok()?;
    let subsystem_handle = manager.handle_for::<T>()?;
    let subsystem = subsystem_handle.get_mut(&mut manager)?;
    Some(f(subsystem))
}

/// Initialize the global subsystem manager.
/// Also available as `initialize_subsystem_manager` for C++ naming compatibility.
pub fn init_subsystem_manager() -> Result<()> {
    if SUBSYSTEM_MANAGER.get().is_none() {
        let mut manager = SubsystemManager::new();

        // Add subsystems in C++ GameEngine::update() order
        let _ = manager.add_subsystem(FileSystemSubsystem::new());
        let _ = manager.add_subsystem(GlobalDataSubsystem::new());
        let _ = manager.add_subsystem(RadarSubsystem::new());
        let _ = manager.add_subsystem(AudioManagerSubsystem::new());
        let _ = manager.add_subsystem(GameClientSubsystem::new());
        let _ = manager.add_subsystem(MessageStreamSubsystem::new());
        let _ = manager.add_subsystem(NetworkSubsystem::new());
        let _ = manager.add_subsystem(CDManagerSubsystem::new());
        let _ = manager.add_subsystem(InputSystemSubsystem::new());
        let _ = manager.add_subsystem(GameLogicSubsystem::new());

        manager.initialize_all()?;

        let arc = Arc::new(Mutex::new(manager));
        SUBSYSTEM_MANAGER
            .set(arc.clone())
            .map_err(|_| anyhow!("Subsystem manager already initialized"))?;
    }

    let arc = SUBSYSTEM_MANAGER
        .get()
        .expect("Subsystem manager not initialized")
        .clone();

    let mut manager = arc
        .lock()
        .expect("SubsystemManager mutex poisoned during init");

    if manager.subsystems.is_empty() {
        // Add subsystems in C++ GameEngine::update() order
        let _ = manager.add_subsystem(FileSystemSubsystem::new());
        let _ = manager.add_subsystem(GlobalDataSubsystem::new());
        let _ = manager.add_subsystem(RadarSubsystem::new());
        let _ = manager.add_subsystem(AudioManagerSubsystem::new());
        let _ = manager.add_subsystem(GameClientSubsystem::new());
        let _ = manager.add_subsystem(MessageStreamSubsystem::new());
        let _ = manager.add_subsystem(NetworkSubsystem::new());
        let _ = manager.add_subsystem(CDManagerSubsystem::new());
        let _ = manager.add_subsystem(InputSystemSubsystem::new());
        let _ = manager.add_subsystem(GameLogicSubsystem::new());
    }

    if !manager.is_initialized() {
        manager.initialize_all()?;
    }

    Ok(())
}

/// Alias for `init_subsystem_manager` - C++ naming compatibility.
pub fn initialize_subsystem_manager() -> Result<()> {
    init_subsystem_manager()
}

/// Obtain a handle to the global subsystem manager.
pub fn get_subsystem_manager() -> Option<SubsystemManagerHandle> {
    SUBSYSTEM_MANAGER
        .get()
        .cloned()
        .map(SubsystemManagerHandle::new)
}

/// Shutdown the global subsystem manager.
pub fn shutdown_subsystem_manager() -> Result<()> {
    if let Some(arc) = SUBSYSTEM_MANAGER.get() {
        let mut manager = arc
            .lock()
            .expect("SubsystemManager mutex poisoned during shutdown");
        manager.shutdown_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::ini::{
        ini_control_bar_scheme::get_control_bar_scheme_manager,
        ini_shell_menu_scheme::get_shell_menu_scheme_manager,
    };

    struct TestSubsystem {
        initialized: bool,
    }

    impl TestSubsystem {
        fn new() -> Self {
            Self { initialized: false }
        }
    }

    impl SubsystemInterface for TestSubsystem {
        fn name(&self) -> &'static str {
            "Test"
        }

        fn init(&mut self) -> Result<()> {
            self.initialized = true;
            Ok(())
        }

        fn reset(&mut self) -> Result<()> {
            Ok(())
        }
        fn update(&mut self, _dt: f32) -> Result<()> {
            Ok(())
        }
        fn shutdown(&mut self) -> Result<()> {
            self.initialized = false;
            Ok(())
        }
    }

    #[test]
    fn test_subsystem_manager() {
        let mut manager = SubsystemManager::new();
        let _ = manager.add_subsystem(TestSubsystem::new());

        assert!(!manager.is_initialized());
        assert!(manager.initialize_all().is_ok());
        assert!(manager.is_initialized());

        let stats = manager.get_stats();
        assert_eq!(stats.total_subsystems, 1);
        assert!(stats.initialized);

        assert!(manager.shutdown_all().is_ok());
        assert!(!manager.is_initialized());
    }

    #[test]
    fn test_initialize_shell_ui_schemes_is_idempotent() {
        initialize_shell_ui_schemes();
        initialize_shell_ui_schemes();

        assert!(get_control_bar_scheme_manager().is_some());
        assert!(get_shell_menu_scheme_manager().read().is_ok());
    }
}
