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
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use crate::assets::{AssetConfig, AssetHandle, AssetManager, AssetPriority};
use crate::audio::GameAudio;
use crate::core::script_action_handler::{
    apply_pending_script_display_state, get_script_fps_limit, get_script_visual_speed_multiplier,
    register_script_display_bridge, reset_script_action_runtime_state,
    GameClientScriptActionHandler,
};
use crate::core::subsystems::{
    create_keyboard, create_mouse, AudioSubsystem, DisplayStringManagerSubsystem,
    FontLibrarySubsystem, HeaderTemplateManagerSubsystem, HotKeyManagerSubsystem,
    InGameUISubsystem, InGameUiHandle, KeyboardHandle, MouseHandle, TerrainVisualStub,
    VideoPlayerSubsystem, WindowManagerSubsystem,
};
use crate::core::Region3D;
use crate::display::display::Display as GraphicsDisplay;
use crate::display::image::{get_mapped_image_collection, sync_mapped_images_from_common};
use crate::display::view::with_tactical_view_ref;
use crate::display::DisplayInterface;
use crate::drawable::*;
use crate::effects::weather_complete::{get_weather_system_mut, initialize_weather_system};
use crate::effects::{DecalManager, DecalSettings, EffectsConfig};
use crate::fx_list::{init_fx_list_store, register_decal_manager, register_fx_audio};
use crate::game_text::GameText;
use crate::gui::{get_shell, set_ui_renderer, with_window_manager, UIRenderer, WindowStatus};
use crate::helpers::{register_in_game_ui_backend, register_mouse_backend};
use crate::input::*;
use crate::message_stream::command_list::get_command_list;
use crate::message_stream::command_router::route_commands_to_gamelogic;
use crate::message_stream::game_message::GameMessageType;
use crate::message_stream::message_stream::THE_MESSAGE_STREAM;
use crate::message_stream::player_state::set_local_player_id;
use crate::message_stream::translators::TranslatorFactory;
use crate::message_stream::{GameMessage, GameMessageDisposition, GameMessageTranslator};
use crate::network::{is_network_command_message, NetworkBridgeHandle};
use crate::platform::PlatformContext;
use crate::system::beacon_display;
use crate::system::{
    BeaconNotification, Coord3D, GameMessageResult, SubsystemInterface, TimeOfDay,
};
use crate::video_player::{
    get_video_player, init_video_player, shutdown_video_player,
    VideoPlayerInterface as GlobalVideoPlayerInterface,
};
use game_engine::common::game_lod::prefers_low_res_movies;
use game_engine::common::ini::get_global_data;
use game_engine::common::recorder::{init_recorder, with_recorder_mut};
use game_engine::common::system::{geometry::Matrix3D, Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::{get_thing_factory, ThingTemplate};
use game_engine::common::user_preferences::UserPreferences;
use game_engine::System::register_drawable_id_counter_hooks;
use nalgebra::Point3;

// GameLogic integration for object iteration
// Note: gamelogic is the crate name (from Cargo.toml)
use game_engine::common::frame_clock::FrameTiming;
use gamelogic::common::types::{ObjectID, Real, INVALID_ID};
use gamelogic::helpers::{
    register_animation_metadata_hook, register_scorch_hook, register_terrain_tree_hook,
    TerrainTreeEvent, TheGameLogic, TheScriptEngine,
};
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::object::Object as GameLogicObject;
use ww3d_core::w3d_io::{W3DChunk, W3DReader};

/// Result type for GameClient operations
pub type GameClientResult<T> = Result<T, GameClientError>;

/// Unique identifier for drawable objects
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DrawableId(pub u32);

impl DrawableId {
    pub const INVALID: Self = DrawableId(0);

    pub fn is_valid(self) -> bool {
        self.0 != 0
    }
}

static GLOBAL_NEXT_DRAWABLE_ID: AtomicU32 = AtomicU32::new(1);

/// Error types for GameClient operations
#[derive(Debug, thiserror::Error)]
pub enum GameClientError {
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Subsystem error: {0}")]
    SubsystemError(String),

    #[error("Drawable not found: {0:?}")]
    DrawableNotFound(DrawableId),

    #[error("Resource loading failed: {0}")]
    ResourceLoadingFailed(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Memory allocation failed")]
    OutOfMemory,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Generic error: {0}")]
    GenericError(#[from] Box<dyn std::error::Error + Send + Sync>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartupMovieAction {
    PlayLogo(&'static str),
    PlaySizzle(&'static str),
    FinalizeStartup,
}

fn startup_movie_action(
    play_intro: bool,
    after_intro: bool,
    play_sizzle: bool,
    startup_sizzle_pending: bool,
    low_res_movies: bool,
) -> Option<StartupMovieAction> {
    if play_intro {
        return Some(StartupMovieAction::PlayLogo(if low_res_movies {
            "EALogoMovie640"
        } else {
            "EALogoMovie"
        }));
    }

    if !after_intro {
        return None;
    }

    if startup_sizzle_pending && play_sizzle {
        return Some(StartupMovieAction::PlaySizzle(if low_res_movies {
            "Sizzle640"
        } else {
            "Sizzle"
        }));
    }

    Some(StartupMovieAction::FinalizeStartup)
}

impl From<Box<dyn std::error::Error>> for GameClientError {
    fn from(error: Box<dyn std::error::Error>) -> Self {
        // Convert the error to a string and create a Send + Sync box
        let error_string = error.to_string();
        let sendable_error: Box<dyn std::error::Error + Send + Sync> =
            Box::new(std::io::Error::new(std::io::ErrorKind::Other, error_string));
        GameClientError::GenericError(sendable_error)
    }
}

struct AnimationDurationResolver {
    asset_manager: Arc<AssetManager>,
    cache_ms: Mutex<HashMap<String, Option<Real>>>,
}

impl AnimationDurationResolver {
    fn new(asset_manager: Arc<AssetManager>) -> Self {
        Self {
            asset_manager,
            cache_ms: Mutex::new(HashMap::new()),
        }
    }

    fn get_duration_ms(&self, animation_name: &str) -> Option<Real> {
        let normalized = normalize_animation_name(animation_name);
        if normalized.is_empty() {
            return None;
        }

        if let Ok(cache) = self.cache_ms.lock() {
            if let Some(cached) = cache.get(&normalized) {
                return *cached;
            }
        }

        let resolved = self.resolve_uncached(&normalized);
        if let Ok(mut cache) = self.cache_ms.lock() {
            cache.insert(normalized, resolved);
        }
        resolved
    }

    fn resolve_uncached(&self, animation_name: &str) -> Option<Real> {
        for candidate in animation_file_candidates(animation_name) {
            let Ok(data) = pollster::block_on(self.asset_manager.load_raw_data_exact(&candidate))
            else {
                continue;
            };
            if let Some(duration_ms) = extract_animation_duration_ms(&data, animation_name) {
                return Some(duration_ms);
            }
        }
        self.resolve_via_global_scan(animation_name)
    }

    fn resolve_via_global_scan(&self, animation_name: &str) -> Option<Real> {
        let paths = self.asset_manager.list_asset_paths_with_extension("w3d");
        for path in paths {
            let Ok(data) = pollster::block_on(self.asset_manager.load_raw_data_exact(&path)) else {
                continue;
            };
            let durations = extract_all_animation_durations_ms(&data);
            if durations.is_empty() {
                continue;
            }

            let mut matched: Option<Real> = None;
            if let Ok(mut cache) = self.cache_ms.lock() {
                for (name, duration_ms) in durations {
                    let key = normalize_animation_name(&name);
                    if key.is_empty() || duration_ms <= 0.0 {
                        continue;
                    }
                    cache.entry(key.clone()).or_insert(Some(duration_ms));
                    if animation_name_matches(animation_name, &key) {
                        matched = Some(duration_ms);
                    }
                }

                if let Some(duration_ms) = matched {
                    cache.insert(animation_name.to_string(), Some(duration_ms));
                    return Some(duration_ms);
                }
            } else {
                for (name, duration_ms) in durations {
                    if duration_ms > 0.0 && animation_name_matches(animation_name, &name) {
                        return Some(duration_ms);
                    }
                }
            }
        }
        None
    }
}

fn normalize_animation_name(value: &str) -> String {
    value.trim().replace('\\', "/").to_ascii_lowercase()
}

fn animation_file_candidates(animation_name: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();
    let normalized = normalize_animation_name(animation_name);

    let mut push_candidate = |raw: String| {
        let candidate = raw.trim().replace('\\', "/");
        if candidate.is_empty() {
            return;
        }
        if seen.insert(candidate.clone()) {
            candidates.push(PathBuf::from(candidate));
        }
    };

    let with_ext = |name: &str| -> String {
        if name.ends_with(".w3d") {
            name.to_string()
        } else {
            format!("{name}.w3d")
        }
    };

    push_candidate(with_ext(&normalized));
    if normalized.ends_with(".w3d") {
        push_candidate(normalized.trim_end_matches(".w3d").to_string());
    }

    if let Some((prefix, _)) = normalized.split_once('.') {
        push_candidate(with_ext(prefix));
    }
    if let Some((prefix, _)) = normalized.rsplit_once('.') {
        push_candidate(with_ext(prefix));
    }

    candidates
}

fn extract_animation_duration_ms(data: &[u8], animation_name: &str) -> Option<Real> {
    let mut reader = W3DReader::new(Cursor::new(data));
    let chunks = reader.read_all_chunks().ok()?;
    extract_animation_duration_ms_from_chunks(&chunks, animation_name)
}

fn extract_all_animation_durations_ms(data: &[u8]) -> Vec<(String, Real)> {
    let mut reader = W3DReader::new(Cursor::new(data));
    let Ok(chunks) = reader.read_all_chunks() else {
        return Vec::new();
    };

    let mut found = Vec::new();
    extract_all_animation_durations_ms_from_chunks(&chunks, &mut found);
    found
}

fn extract_animation_duration_ms_from_chunks(
    chunks: &[W3DChunk],
    animation_name: &str,
) -> Option<Real> {
    for chunk in chunks {
        match chunk {
            W3DChunk::Animation(animation) => {
                if animation_name_matches(animation_name, &animation.header.name_str()) {
                    if let Some(duration_ms) = calculate_duration_ms(
                        animation.header.num_frames,
                        animation.header.frame_rate,
                    ) {
                        return Some(duration_ms);
                    }
                }
            }
            W3DChunk::AnimationHeader(header) => {
                if animation_name_matches(animation_name, &header.name_str()) {
                    if let Some(duration_ms) =
                        calculate_duration_ms(header.num_frames, header.frame_rate)
                    {
                        return Some(duration_ms);
                    }
                }
            }
            W3DChunk::CompressedAnimation(sub_chunks) => {
                if let Some(duration_ms) =
                    extract_animation_duration_ms_from_chunks(sub_chunks, animation_name)
                {
                    return Some(duration_ms);
                }
            }
            W3DChunk::CompressedAnimationHeader(header) => {
                let header_name = chunk_name_str(&header.name);
                if animation_name_matches(animation_name, &header_name) {
                    if let Some(duration_ms) =
                        calculate_duration_ms(header.num_frames, header.frame_rate)
                    {
                        return Some(duration_ms);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_all_animation_durations_ms_from_chunks(
    chunks: &[W3DChunk],
    out: &mut Vec<(String, Real)>,
) {
    for chunk in chunks {
        match chunk {
            W3DChunk::Animation(animation) => {
                if let Some(duration_ms) =
                    calculate_duration_ms(animation.header.num_frames, animation.header.frame_rate)
                {
                    out.push((animation.header.name_str(), duration_ms));
                }
            }
            W3DChunk::AnimationHeader(header) => {
                if let Some(duration_ms) =
                    calculate_duration_ms(header.num_frames, header.frame_rate)
                {
                    out.push((header.name_str(), duration_ms));
                }
            }
            W3DChunk::CompressedAnimation(sub_chunks) => {
                extract_all_animation_durations_ms_from_chunks(sub_chunks, out);
            }
            W3DChunk::CompressedAnimationHeader(header) => {
                if let Some(duration_ms) =
                    calculate_duration_ms(header.num_frames, header.frame_rate)
                {
                    out.push((chunk_name_str(&header.name), duration_ms));
                }
            }
            _ => {}
        }
    }
}

fn chunk_name_str(bytes: &[u8; 16]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches('\0')
        .to_ascii_lowercase()
}

fn calculate_duration_ms(num_frames: u32, frame_rate: u32) -> Option<Real> {
    if num_frames == 0 || frame_rate == 0 {
        return None;
    }
    Some((num_frames as Real * 1000.0) / frame_rate as Real)
}

fn animation_name_matches(requested: &str, candidate: &str) -> bool {
    let requested = normalize_animation_name(requested);
    let candidate = normalize_animation_name(candidate);
    if requested.is_empty() || candidate.is_empty() {
        return false;
    }
    if requested == candidate {
        return true;
    }

    let requested_trimmed = requested.strip_suffix(".w3d").unwrap_or(&requested);
    let candidate_trimmed = candidate.strip_suffix(".w3d").unwrap_or(&candidate);
    if requested_trimmed == candidate_trimmed {
        return true;
    }

    if let Some((_, requested_tail)) = requested_trimmed.rsplit_once('.') {
        if requested_tail == candidate_trimmed {
            return true;
        }
    }
    if let Some((_, candidate_tail)) = candidate_trimmed.rsplit_once('.') {
        if requested_trimmed == candidate_tail {
            return true;
        }
    }

    false
}

/// Drawable table of contents entry for save/load operations
#[derive(Debug, Clone)]
struct DrawableTOCEntry {
    name: String,
    id: u16,
}

/// Tracks the currently loaded map asset
#[derive(Debug, Clone)]
struct LoadedMap {
    name: String,
    handle: AssetHandle,
}

/// Message translator ID type
pub type TranslatorId = u32;
pub const TRANSLATOR_ID_INVALID: TranslatorId = 0;

/// The main GameClient struct - central hub for all client operations
pub struct GameClient {
    // Core state
    frame: u32,
    next_drawable_id: DrawableId,
    local_player_id: i32,

    // Drawable management
    drawable_map: std::collections::HashMap<DrawableId, Box<dyn Drawable>>,
    drawable_object_map: std::collections::HashMap<ObjectID, DrawableId>,
    drawable_toc: Vec<DrawableTOCEntry>,
    text_bearing_drawables: Vec<DrawableId>,
    loaded_map: Option<LoadedMap>,

    // Message system
    translators: [TranslatorId; super::MAX_CLIENT_TRANSLATORS],
    num_translators: usize,
    command_translator: Option<Arc<dyn CommandTranslator>>,
    message_dispatcher: Arc<GameClientMessageDispatcher>,
    network_bridge: Option<NetworkBridgeHandle>,

    // Subsystems
    subsystem_manager: SubsystemManager,

    // Performance tracking
    rendered_object_count: u32,
    last_update_time: Instant,

    // Timing
    target_frame_duration: Duration,

    // Runtime flags
    startup_sizzle_pending: bool,
    initialized: bool,
}

/// Manages subsystem lifecycle and dependencies
pub struct SubsystemManager {
    display: Option<Arc<Mutex<GraphicsDisplay>>>,
    audio: Option<Arc<Mutex<AudioSubsystem>>>,
    input_keyboard: Option<KeyboardHandle>,
    input_mouse: Option<MouseHandle>,
    terrain_visual: Option<Arc<Mutex<TerrainVisualStub>>>,
    window_manager: Option<Arc<Mutex<WindowManagerSubsystem>>>,
    font_library: Option<Arc<Mutex<FontLibrarySubsystem>>>,
    header_templates: Option<Arc<Mutex<HeaderTemplateManagerSubsystem>>>,
    display_strings: Option<Arc<Mutex<DisplayStringManagerSubsystem>>>,
    hot_key_manager: Option<Arc<Mutex<HotKeyManagerSubsystem>>>,
    in_game_ui: Option<Arc<Mutex<InGameUISubsystem>>>,
    video_player: Option<Arc<Mutex<VideoPlayerSubsystem>>>,
    decal_manager: Option<Arc<Mutex<DecalManager>>>,
    asset_manager: Option<Arc<AssetManager>>,
    platform_context: Option<PlatformContext>,
}

/// Message dispatcher for filtering and routing game messages
pub struct GameClientMessageDispatcher {
    message_filters: Vec<Box<dyn MessageFilter + Send + Sync>>,
}

struct DispatcherTranslator {
    dispatcher: Arc<GameClientMessageDispatcher>,
}

impl DispatcherTranslator {
    fn new(dispatcher: Arc<GameClientMessageDispatcher>) -> Self {
        Self { dispatcher }
    }
}

impl GameMessageTranslator for DispatcherTranslator {
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        self.dispatcher.translate_game_message(msg)
    }
}

/// Trait for message filtering components
pub trait MessageFilter {
    fn should_keep_message(&self, msg: &GameMessage) -> bool;
    fn transform_message(&self, msg: &mut GameMessage) -> GameMessageResult<()>;
}

/// In-Game UI interface for managing game interface elements
pub trait InGameUI: SubsystemInterface + Send + Sync {
    /// Stop tracking a drawable object in the UI
    fn disregard_drawable(&self, drawable: &dyn Drawable)
        -> Result<(), Box<dyn std::error::Error>>;

    /// React to beacon changes so the HUD/radar can display the correct data.
    fn handle_beacon_notification(
        &mut self,
        _notification: &BeaconNotification,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

/// Video player interface for playing cutscenes and videos
pub trait VideoPlayerInterface: SubsystemInterface + Send + Sync {
    // Video player methods would be defined here
    // For now it's just a marker trait
}

/// Command translator interface for context-sensitive commands
pub trait CommandTranslator: Send + Sync {
    fn evaluate_context_command(
        &self,
        drawable: &dyn Drawable,
        position: &Coord3D,
        cmd_type: CommandEvaluateType,
    ) -> GameMessageResult<GameMessageType>;
}

/// Command evaluation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandEvaluateType {
    Primary,
    Secondary,
    Context,
}

impl GameClient {
    /// Creates a new GameClient instance
    pub fn new() -> GameClientResult<Self> {
        let mut client = Self {
            frame: 0,
            next_drawable_id: DrawableId(1),
            local_player_id: 0,
            drawable_map: std::collections::HashMap::with_capacity(super::DRAWABLE_HASH_SIZE),
            drawable_object_map: std::collections::HashMap::new(),
            drawable_toc: Vec::new(),
            text_bearing_drawables: Vec::new(),
            loaded_map: None,
            translators: [TRANSLATOR_ID_INVALID; super::MAX_CLIENT_TRANSLATORS],
            num_translators: 0,
            command_translator: None,
            message_dispatcher: Arc::new(GameClientMessageDispatcher::new()),
            network_bridge: None,
            subsystem_manager: SubsystemManager::new(),
            rendered_object_count: 0,
            last_update_time: Instant::now(),
            target_frame_duration: Duration::from_millis(33),
            startup_sizzle_pending: false,
            initialized: false,
        };

        client.set_local_player_id(0);

        Ok(client)
    }

    /// Initializes all subsystems and resources
    pub fn init(&mut self) -> GameClientResult<()> {
        if self.initialized {
            return Err(GameClientError::InvalidOperation(
                "GameClient already initialized".to_string(),
            ));
        }

        reset_script_action_runtime_state();
        init_video_player();

        // Set expected frame rate
        self.set_frame_rate(Duration::from_millis(33))?; // ~30 FPS

        // Initialize subsystems in dependency order
        self.init_core_subsystems()?;
        self.init_asset_systems()?;
        self.init_input_subsystems()?;
        self.init_display_subsystems()?;
        self.init_audio_subsystems()?;
        self.init_game_subsystems()?;
        self.init_message_translators()?;
        self.init_network_bridge();
        self.init_recorder_bridge();
        self.init_savegame_counter_bridge();

        self.initialized = true;

        log::info!("GameClient initialized successfully");
        Ok(())
    }

    /// Updates the game client - main game loop entry point
    pub fn update(&mut self) -> GameClientResult<()> {
        if !self.initialized {
            return Err(GameClientError::InvalidOperation(
                "GameClient not initialized".to_string(),
            ));
        }

        let current_time = Instant::now();
        let delta_time = current_time.duration_since(self.last_update_time);
        self.last_update_time = current_time;

        self.frame = self.frame.wrapping_add(1);

        // Create frame tick message
        self.create_frame_tick_message()?;
        self.update_startup_movies()?;
        if self.startup_movies_active() {
            self.update_startup_movie_display()?;
            self.rendered_object_count = 0;
            self.finish_frame_timing(current_time);
            return Ok(());
        }
        self.ensure_shell_visible()?;

        // Update subsystems
        let mut visual_delta = if self.should_freeze_visual_time() {
            0.0
        } else {
            delta_time.as_secs_f32()
        };
        let visual_speed = get_script_visual_speed_multiplier();
        visual_delta = if visual_speed <= 0 {
            0.0
        } else {
            visual_delta * visual_speed as f32
        };
        self.update_input()?;
        self.update_audio()?;
        self.update_drawables(visual_delta)?;
        self.update_effects(visual_delta)?;
        apply_pending_script_display_state();
        self.update_display()?;
        self.update_ui()?;
        self.process_beacon_notifications()?;
        self.pump_message_stream()?;

        // Reset performance counters
        self.rendered_object_count = 0;

        self.finish_frame_timing(current_time);
        Ok(())
    }

    fn finish_frame_timing(&self, frame_start: Instant) {
        let script_fps_limit = get_script_fps_limit();
        let target_frame_duration = if script_fps_limit > 0 {
            Duration::from_secs_f64(1.0 / script_fps_limit as f64)
        } else {
            self.target_frame_duration
        };
        let frame_elapsed = frame_start.elapsed();
        if frame_elapsed < target_frame_duration {
            thread::sleep(target_frame_duration - frame_elapsed);
        }
    }

    /// Resets the game client for a new game
    pub fn reset(&mut self) -> GameClientResult<()> {
        reset_script_action_runtime_state();
        Self::reset_global_video_player_streams();
        self.startup_sizzle_pending = false;

        // C++ parity: show a blank transition window while subsystems reset.
        let reset_background = with_window_manager(|manager| {
            manager
                .create_layout_with_windows("Menus/BlankWindow.wnd")
                .ok()
                .map(|(layout, _)| {
                    layout.borrow_mut().hide(false);
                    layout.borrow_mut().bring_forward();
                    if let Some(window) = layout.borrow().get_first_window() {
                        window.borrow_mut().clear_status(WindowStatus::IMAGE);
                    }
                    layout
                })
        });

        // Clear drawable map
        self.drawable_map.clear();
        self.drawable_object_map.clear();

        // Clear other drawable data
        self.text_bearing_drawables.clear();

        if let Some(loaded) = self.loaded_map.take() {
            if let Some(ref asset_manager) = self.subsystem_manager.asset_manager {
                asset_manager.release_asset(loaded.handle);
            }
        }

        // Reset subsystems
        self.subsystem_manager.reset_all()?;

        if let Some(layout) = reset_background {
            with_window_manager(|manager| manager.destroy_layout(&layout));
        }

        // Clear TOC
        self.drawable_toc.clear();

        log::info!("GameClient reset completed");
        Ok(())
    }

    fn should_save_drawable(drawable: &dyn Drawable) -> bool {
        if drawable.get_status().has(DrawableStatus::NO_SAVE) {
            if drawable.get_object_id().is_none() {
                return false;
            }
            log::warn!("Drawable marked NO_SAVE but bound to an object; keeping for parity.");
        }

        true
    }

    fn resolve_drawable_template_name(drawable: &dyn Drawable) -> Option<String> {
        if let Some(name) = drawable.get_template_name() {
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }

        let object_id = drawable.get_object_id()?;
        let object_arc = OBJECT_REGISTRY.get_object(object_id)?;
        let object_guard = object_arc.read().ok()?;
        let name = object_guard.get_template().get_name().to_string();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }

    fn collect_saveable_drawables_sorted(&self) -> Result<Vec<(DrawableId, String)>, String> {
        let mut entries = Vec::new();
        for (&id, drawable) in &self.drawable_map {
            if !Self::should_save_drawable(drawable.as_ref()) {
                continue;
            }

            let template_name = Self::resolve_drawable_template_name(drawable.as_ref())
                .ok_or_else(|| format!("Drawable '{}' missing template name for save", id.0))?;
            entries.push((id, template_name));
        }

        // HashMap traversal order is nondeterministic; save/load parity expects stable ordering.
        entries.sort_by_key(|(id, _)| id.0);
        Ok(entries)
    }

    fn add_toc_entry(&mut self, name: String, id: u16) {
        self.drawable_toc.push(DrawableTOCEntry { name, id });
    }

    fn find_toc_entry_by_name(&self, name: &str) -> Option<&DrawableTOCEntry> {
        self.drawable_toc.iter().find(|entry| entry.name == name)
    }

    fn find_toc_entry_by_id(&self, id: u16) -> Option<&DrawableTOCEntry> {
        self.drawable_toc.iter().find(|entry| entry.id == id)
    }

    fn xfer_drawable_toc(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        self.drawable_toc.clear();

        let mut toc_count: u32 = 0;
        if xfer.is_writing() {
            let save_entries = self.collect_saveable_drawables_sorted()?;
            let mut toc_names: Vec<String> = Vec::new();
            for (_, template_name) in save_entries {
                if toc_names.iter().any(|name| name == &template_name) {
                    continue;
                }
                toc_names.push(template_name);
            }

            for name in toc_names {
                toc_count = toc_count.saturating_add(1);
                self.add_toc_entry(name, toc_count as u16);
            }

            xfer.xfer_unsigned_int(&mut toc_count)
                .map_err(|e| e.to_string())?;

            for entry in &mut self.drawable_toc {
                let mut name = entry.name.clone();
                xfer.xfer_ascii_string(&mut name)
                    .map_err(|e| e.to_string())?;
                entry.name = name;

                let mut id = entry.id;
                xfer.xfer_unsigned_short(&mut id)
                    .map_err(|e| e.to_string())?;
                entry.id = id;
            }
        } else {
            xfer.xfer_unsigned_int(&mut toc_count)
                .map_err(|e| e.to_string())?;

            for _ in 0..toc_count {
                let mut name = String::new();
                xfer.xfer_ascii_string(&mut name)
                    .map_err(|e| e.to_string())?;
                let mut id: u16 = 0;
                xfer.xfer_unsigned_short(&mut id)
                    .map_err(|e| e.to_string())?;
                self.add_toc_entry(name, id);
            }
        }

        Ok(())
    }

    fn xfer_drawable_snapshot(
        drawable: &mut dyn Drawable,
        xfer: &mut dyn Xfer,
    ) -> Result<(), String> {
        drawable.xfer_snapshot(xfer)
    }

    fn drawable_matches_saved_template(
        drawable: &dyn Drawable,
        saved_template: &Arc<ThingTemplate>,
        factory: &game_engine::common::thing::ThingFactory,
    ) -> bool {
        let Some(existing_name) = Self::resolve_drawable_template_name(drawable) else {
            return false;
        };
        let Some(existing_template) = factory.find_template(&existing_name, false) else {
            return false;
        };

        let existing_final = ThingTemplate::get_final_override(&existing_template);
        let saved_final = ThingTemplate::get_final_override(saved_template);
        Arc::ptr_eq(&existing_final, &saved_final)
            || existing_final.get_name() == saved_final.get_name()
    }

    /// Retrieve the platform context (window + graphics + audio) for external event-loop driving.
    pub fn take_platform_context(&mut self) -> Option<PlatformContext> {
        self.subsystem_manager.platform_context.take()
    }

    /// Registers a drawable and assigns it a unique ID
    pub fn register_drawable(
        &mut self,
        drawable: Box<dyn Drawable>,
    ) -> GameClientResult<DrawableId> {
        self.register_drawable_with_template(drawable, None)
    }

    pub fn register_drawable_with_template(
        &mut self,
        mut drawable: Box<dyn Drawable>,
        template_name: Option<String>,
    ) -> GameClientResult<DrawableId> {
        if let Some(name) = template_name {
            drawable.set_template_name(Some(name));
        } else if drawable.get_template_name().is_none() {
            if let Some(object_id) = drawable.get_object_id() {
                if let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) {
                    if let Ok(object_guard) = object_arc.read() {
                        let fallback_name = object_guard.get_template().get_name().to_string();
                        if !fallback_name.is_empty() {
                            drawable.set_template_name(Some(fallback_name));
                        }
                    }
                }
            }
        }

        let id = self.alloc_drawable_id();
        drawable.set_id(id);

        let object_id = drawable.get_object_id();
        if let Some(object_id) = object_id {
            if let Some(previous_drawable_id) = self.drawable_object_map.get(&object_id).copied() {
                if previous_drawable_id != id {
                    if let Some(previous_drawable) =
                        self.drawable_map.get_mut(&previous_drawable_id)
                    {
                        previous_drawable.set_object_id(None);
                    }
                    self.drawable_object_map.remove(&object_id);
                }
            }
        }
        self.drawable_map.insert(id, drawable);
        if let Some(object_id) = object_id {
            self.drawable_object_map.insert(object_id, id);
        }

        log::debug!("Registered drawable with ID {:?}", id);
        Ok(id)
    }

    pub fn create_drawable_from_template(
        &mut self,
        template: &ThingTemplate,
    ) -> GameClientResult<DrawableId> {
        let drawable: Box<dyn Drawable> = Box::new(BasicDrawable::new(DrawableId::INVALID));
        self.register_drawable_with_template(drawable, Some(template.get_name().to_string()))
    }

    /// Finds a drawable by its ID
    pub fn find_drawable_by_id(&self, id: DrawableId) -> Option<&dyn Drawable> {
        self.drawable_map.get(&id).map(|d| d.as_ref())
    }

    /// Finds a mutable drawable by its ID
    pub fn find_drawable_by_id_mut(&mut self, id: DrawableId) -> Option<&mut Box<dyn Drawable>> {
        self.drawable_map.get_mut(&id)
    }

    /// Destroys a drawable and removes it from all systems
    pub fn destroy_drawable(&mut self, id: DrawableId) -> GameClientResult<()> {
        if let Some(drawable) = self.drawable_map.get(&id) {
            // Notify UI systems
            if let Some(ref ui) = self.subsystem_manager.in_game_ui {
                ui.lock()
                    .map_err(|_| {
                        GameClientError::SubsystemError("In-game UI lock poisoned".to_string())
                    })?
                    .disregard_drawable(drawable.as_ref())?;
            }
        }

        // Remove from the map (this drops the drawable)
        if let Some(drawable) = self.drawable_map.remove(&id) {
            if let Some(object_id) = drawable.get_object_id() {
                if self.drawable_object_map.get(&object_id).copied() == Some(id) {
                    self.drawable_object_map.remove(&object_id);
                }
            }
        }

        // Remove from text bearing list
        self.text_bearing_drawables
            .retain(|&stored_id| stored_id != id);

        Ok(())
    }

    pub fn bind_drawable_to_object(
        &mut self,
        drawable_id: DrawableId,
        object_id: ObjectID,
    ) -> GameClientResult<()> {
        let old_object_id = self
            .drawable_map
            .get(&drawable_id)
            .and_then(|drawable| drawable.get_object_id());

        if let Some(old_object_id) = old_object_id {
            if old_object_id != object_id
                && self.drawable_object_map.get(&old_object_id).copied() == Some(drawable_id)
            {
                self.drawable_object_map.remove(&old_object_id);
            }
        }

        if let Some(previous_drawable_id) = self.drawable_object_map.get(&object_id).copied() {
            if previous_drawable_id != drawable_id {
                if let Some(previous_drawable) = self.drawable_map.get_mut(&previous_drawable_id) {
                    previous_drawable.set_object_id(None);
                }
            }
        }

        if let Some(drawable) = self.drawable_map.get_mut(&drawable_id) {
            drawable.set_object_id(Some(object_id));
            self.drawable_object_map.insert(object_id, drawable_id);
            Ok(())
        } else {
            Err(GameClientError::DrawableNotFound(drawable_id))
        }
    }

    pub fn get_drawable_for_object(&self, object_id: ObjectID) -> Option<DrawableId> {
        self.drawable_object_map.get(&object_id).copied()
    }

    /// Iterates over drawables in a given region
    pub fn iterate_drawables_in_region<F>(
        &self,
        region: Option<&Region3D>,
        mut callback: F,
    ) -> GameClientResult<()>
    where
        F: FnMut(&dyn Drawable),
    {
        for drawable in self.drawable_map.values() {
            let position = drawable.get_position();

            let in_region = match region {
                None => true,
                Some(r) => {
                    position.x >= r.lo.x
                        && position.x <= r.hi.x
                        && position.y >= r.lo.y
                        && position.y <= r.hi.y
                        && position.z >= r.lo.z
                        && position.z <= r.hi.z
                }
            };

            if in_region {
                callback(drawable.as_ref());
            }
        }

        Ok(())
    }

    /// Sets the current frame number
    pub fn set_frame(&mut self, frame: u32) {
        self.frame = frame;
    }

    /// Sets the local player identifier used for command routing.
    pub fn set_local_player_id(&mut self, player_id: i32) {
        self.local_player_id = player_id;
        set_local_player_id(player_id);
    }

    /// Gets the current frame number
    pub fn get_frame(&self) -> u32 {
        self.frame
    }

    /// Gets all drawable IDs
    pub fn get_drawable_ids(&self) -> Vec<DrawableId> {
        self.drawable_map.keys().copied().collect()
    }

    /// Evaluates context commands for a drawable
    pub fn evaluate_context_command(
        &self,
        drawable: &dyn Drawable,
        position: &Coord3D,
        cmd_type: CommandEvaluateType,
    ) -> GameClientResult<GameMessageType> {
        match &self.command_translator {
            Some(translator) => {
                Ok(translator.evaluate_context_command(drawable, position, cmd_type)?)
            }
            None => Ok(GameMessageType::Invalid),
        }
    }

    /// Adds a drawable to the text-bearing list for UI rendering
    pub fn add_text_bearing_drawable(&mut self, drawable_id: DrawableId) {
        self.text_bearing_drawables.push(drawable_id);
    }

    /// Flushes all text-bearing drawables
    pub fn flush_text_bearing_drawables(&mut self) -> GameClientResult<()> {
        for &drawable_id in &self.text_bearing_drawables {
            if let Some(drawable) = self.drawable_map.get(&drawable_id) {
                drawable.draw_ui_text()?;
            }
        }
        self.text_bearing_drawables.clear();
        Ok(())
    }

    /// Sets time of day for all drawables
    pub fn set_time_of_day(&mut self, tod: TimeOfDay) -> GameClientResult<()> {
        self.iterate_drawables_in_region(None, |drawable| {
            let _ = drawable.set_time_of_day(tod);
        })
    }

    /// Loads a map
    pub fn load_map(&mut self, map_name: &str) -> GameClientResult<bool> {
        if map_name.is_empty() {
            return Ok(false);
        }

        if self
            .loaded_map
            .as_ref()
            .map(|map| map.name == map_name)
            .unwrap_or(false)
        {
            log::debug!("Map '{}' already loaded", map_name);
            return Ok(true);
        }

        let asset_manager = self
            .subsystem_manager
            .asset_manager
            .as_ref()
            .ok_or_else(|| {
                GameClientError::InitializationFailed(
                    "Asset manager not initialized before map load".to_string(),
                )
            })?;

        let normalized_name = map_name.replace('\\', "/");
        let candidates = [
            format!("Maps/{0}/{0}.map", normalized_name),
            format!("Maps/{0}.map", normalized_name),
            format!("{0}.map", normalized_name),
        ];

        let mut last_error = None;
        for candidate in candidates.iter() {
            let path = PathBuf::from(candidate);
            match pollster::block_on(
                asset_manager.load_asset(path.clone(), AssetPriority::Critical),
            ) {
                Ok(handle) => {
                    log::info!("Loaded map asset: {}", candidate);

                    if let Some(previous) = self.loaded_map.take() {
                        asset_manager.release_asset(previous.handle);
                    }

                    self.loaded_map = Some(LoadedMap {
                        name: map_name.to_string(),
                        handle,
                    });

                    return Ok(true);
                }
                Err(err) => {
                    last_error = Some(err);
                }
            }
        }

        if let Some(err) = last_error {
            log::error!("Failed to load map '{}': {}", map_name, err);
            return Err(GameClientError::ResourceLoadingFailed(err.to_string()));
        }

        Ok(false)
    }

    /// Unloads a map
    pub fn unload_map(&mut self, map_name: &str) -> GameClientResult<()> {
        log::info!("Unloading map: {}", map_name);

        if let Some(loaded) = self.loaded_map.take() {
            if let Some(ref asset_manager) = self.subsystem_manager.asset_manager {
                asset_manager.release_asset(loaded.handle);
            }
        }

        Ok(())
    }

    /// Preloads assets for performance optimization
    pub fn preload_assets(&mut self, time_of_day: TimeOfDay) -> GameClientResult<()> {
        log::info!("Preloading assets for time of day: {:?}", time_of_day);

        // Preload assets for existing drawables
        self.iterate_drawables_in_region(None, |drawable| {
            let _ = drawable.preload_assets(time_of_day);
        })?;

        // Preload common assets from thing factory
        self.preload_template_assets_from_factory(time_of_day)?;

        // Preload UI assets
        if let Some(ref display) = self.subsystem_manager.display {
            display.lock().unwrap().preload_common_textures()?;
        }

        if let Some(ref asset_manager) = self.subsystem_manager.asset_manager {
            pollster::block_on(asset_manager.preload_configured_assets()).map_err(|e| {
                GameClientError::SubsystemError(format!("Asset preloading failed: {e}"))
            })?;
        }

        log::info!("Asset preloading completed");
        Ok(())
    }

    /// Gets rendered object count for performance monitoring
    pub fn get_rendered_object_count(&self) -> u32 {
        self.rendered_object_count
    }

    /// Increments rendered object count
    pub fn increment_rendered_object_count(&mut self) {
        self.rendered_object_count += 1;
    }

    /// Resets rendered object count
    pub fn reset_rendered_object_count(&mut self) {
        self.rendered_object_count = 0;
    }

    // ==================================================================================
    // GameLogic Object Iteration Methods
    // Reference: GameClient.cpp line 661-698
    // ==================================================================================

    /// Iterate over all GameLogic objects that have drawables bound to them
    ///
    /// This method provides access to GameLogic objects for rendering purposes.
    /// It iterates through all registered objects in the GameLogic layer and invokes
    /// the callback for each object that has an associated drawable.
    ///
    /// # Arguments
    ///
    /// * `callback` - Function called for each object with drawable
    ///
    /// # Returns
    ///
    /// * `Ok(())` - All objects iterated successfully
    /// * `Err(GameClientError)` - If object registry access fails
    ///
    /// # C++ Reference
    ///
    /// Matches C++ GameClient.cpp lines 661-698 - drawable visibility update loop
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_client_rust::core::GameClient;
    /// # let mut client = GameClient::new().unwrap();
    /// client.iterate_objects_with_drawables(|obj_ref| {
    ///     // Process each object that has a drawable
    ///     if let Ok(obj) = obj_ref.read() {
    ///         let pos = obj.get_position();
    ///         println!("Object at ({}, {}, {})", pos.x, pos.y, pos.z);
    ///     }
    /// })?;
    /// # Ok::<(), game_client_rust::core::GameClientError>(())
    /// ```
    pub fn iterate_objects_with_drawables<F>(&self, mut callback: F) -> GameClientResult<()>
    where
        F: FnMut(&Arc<RwLock<GameLogicObject>>),
    {
        // Get all registered GameLogic objects
        let all_objects = OBJECT_REGISTRY.get_all_objects();

        // Iterate through objects and invoke callback for those with drawables
        for object_ref in all_objects {
            let has_drawable = object_ref
                .read()
                .ok()
                .and_then(|obj| obj.get_drawable())
                .is_some();
            if has_drawable {
                callback(&object_ref);
            }
        }

        Ok(())
    }

    /// Find a specific GameLogic object by its ID
    ///
    /// Retrieves a strong reference to a GameLogic object given its ObjectID.
    /// This is useful for looking up specific objects during rendering or
    /// command processing.
    ///
    /// # Arguments
    ///
    /// * `object_id` - The ID of the object to find
    ///
    /// # Returns
    ///
    /// * `Ok(Some(object))` - Object found
    /// * `Ok(None)` - Object not found
    /// * `Err(GameClientError)` - Registry access error
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_client_rust::core::GameClient;
    /// # use game_engine::common::ObjectID;
    /// # let client = GameClient::new().unwrap();
    /// let object_id = 42; // Example ID
    /// if let Some(obj_ref) = client.find_game_object(object_id)? {
    ///     if let Ok(obj) = obj_ref.read() {
    ///         println!("Found object: {:?}", obj.get_id());
    ///     }
    /// }
    /// # Ok::<(), game_client_rust::core::GameClientError>(())
    /// ```
    pub fn find_game_object(
        &self,
        object_id: ObjectID,
    ) -> GameClientResult<Option<Arc<RwLock<GameLogicObject>>>> {
        Ok(OBJECT_REGISTRY.get_object(object_id))
    }

    /// Update drawable visibility based on shroud/fog of war status
    ///
    /// Synchronizes drawable visibility with the GameLogic shroud system.
    /// Objects in fog of war are marked as obscured so they aren't rendered.
    ///
    /// # Arguments
    ///
    /// * `local_player_index` - The local player's index for shroud calculations
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Visibility updated successfully
    /// * `Err(GameClientError)` - If update fails
    ///
    /// # C++ Reference
    ///
    /// Matches C++ GameClient.cpp:661-698 shroud visibility update
    ///
    /// # Note
    ///
    /// Uses GameLogic shroud status to hide or reveal drawables for the local player.
    pub fn update_drawable_visibility(&mut self, local_player_index: i32) -> GameClientResult<()> {
        use gamelogic::common::types::ObjectShroudStatus;

        self.iterate_objects_with_drawables(|obj_ref| {
            let Ok(mut obj) = obj_ref.write() else {
                return;
            };

            if obj.is_destroyed() {
                if let Some(drawable) = obj.get_drawable() {
                    if let Ok(mut drawable_guard) = drawable.write() {
                        drawable_guard.set_visible(false);
                    }
                }
                return;
            }

            // Keep object-level visibility bookkeeping up to date.
            let _ = obj.update_visibility_for_all_players(self.frame);

            let shroud = obj.get_shrouded_status(local_player_index);
            let fully_obscured = matches!(
                shroud,
                ObjectShroudStatus::Fogged
                    | ObjectShroudStatus::Shrouded
                    | ObjectShroudStatus::InvalidButPreviousValid
            );

            if let Some(drawable) = obj.get_drawable() {
                if let Ok(mut drawable_guard) = drawable.write() {
                    drawable_guard.set_visible(!fully_obscured);
                }
            }
        })?;

        Ok(())
    }

    /// Synchronize GameClient drawables with GameLogic objects.
    ///
    /// Updates drawable transforms from their owning GameLogic objects. This mirrors the
    /// C++ render-sync step that keeps drawables aligned with object positions/orientations.
    pub fn sync_with_game_logic(&mut self) -> GameClientResult<()> {
        self.iterate_objects_with_drawables(|obj_ref| {
            let Ok(obj) = obj_ref.read() else {
                return;
            };
            let pos = *obj.get_position();
            let angle = obj.get_orientation();
            if let Some(drawable) = obj.get_drawable() {
                if let Ok(mut drawable_guard) = drawable.write() {
                    let mut transform =
                        glam::Mat4::from_translation(glam::vec3(pos.x, pos.y, pos.z));
                    transform *= glam::Mat4::from_rotation_y(angle);
                    drawable_guard.set_transform(transform);
                }
            }
        })?;

        Ok(())
    }

    /// Main rendering update - called each frame
    ///
    /// Performs all per-frame updates needed for rendering:
    /// - Syncs with GameLogic objects
    /// - Updates drawable positions from object positions
    /// - Updates visibility based on shroud
    /// - Updates animations
    ///
    /// # Arguments
    ///
    /// * `timing` - Frame timing information
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Update successful
    /// * `Err(GameClientError)` - If update fails
    ///
    /// # C++ Reference
    ///
    /// Matches C++ GameClient.cpp Draw functions and update loop
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_client_rust::core::GameClient;
    /// # use game_engine::common::frame_clock::{FrameClock, FrameTiming};
    /// # let mut client = GameClient::new().unwrap();
    /// # let mut clock = FrameClock::new();
    /// let timing = clock.next_frame();
    /// client.update_for_rendering(&timing)?;
    /// # Ok::<(), game_client_rust::core::GameClientError>(())
    /// ```
    pub fn update_for_rendering(&mut self, timing: &FrameTiming) -> GameClientResult<()> {
        // 1. Sync with GameLogic - ensure we know about all objects
        self.sync_with_game_logic()?;

        // 2. Update visibility based on shroud/fog of war
        self.update_drawable_visibility(self.local_player_id)?;

        // 3. Update animations for all drawables
        let visual_delta = if self.should_freeze_visual_time() {
            0.0
        } else {
            timing.delta_seconds()
        };
        self.update_drawable_animations(visual_delta)?;

        Ok(())
    }

    /// Update all drawable animations
    ///
    /// Steps animation state forward for all active drawables.
    ///
    /// # Arguments
    ///
    /// * `delta_time` - Time elapsed since last frame in seconds
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Animations updated
    /// * `Err(GameClientError)` - If update fails
    fn update_drawable_animations(&mut self, delta_time: f32) -> GameClientResult<()> {
        let frame = self.frame;

        for drawable in self.drawable_map.values_mut() {
            // Update drawable animation state
            // The drawable's update method advances animation frames
            drawable.update(delta_time);
        }

        self.iterate_objects_with_drawables(|obj_ref| {
            let Ok(mut obj) = obj_ref.write() else {
                return;
            };
            if let Some(drawable) = obj.get_drawable() {
                if let Ok(mut drawable_guard) = drawable.write() {
                    let _ = drawable_guard.update(delta_time, frame);
                }
            }
        })?;
        Ok(())
    }

    // Private implementation methods

    fn alloc_drawable_id(&mut self) -> DrawableId {
        let global_next = GLOBAL_NEXT_DRAWABLE_ID.load(Ordering::Relaxed).max(1);
        if self.next_drawable_id.0 < global_next {
            self.next_drawable_id = DrawableId(global_next);
        }
        let id = self.next_drawable_id;
        let next = self.next_drawable_id.0.saturating_add(1).max(1);
        self.next_drawable_id = DrawableId(next);
        GLOBAL_NEXT_DRAWABLE_ID.fetch_max(next, Ordering::Relaxed);
        id
    }

    fn get_drawable_id_counter(&self) -> u32 {
        self.next_drawable_id.0.max(1)
    }

    fn set_drawable_id_counter(&mut self, next_drawable_id: u32) {
        let normalized = next_drawable_id.max(1);
        self.next_drawable_id = DrawableId(normalized);
        GLOBAL_NEXT_DRAWABLE_ID.fetch_max(normalized, Ordering::Relaxed);
    }

    fn global_drawable_id_counter() -> u32 {
        GLOBAL_NEXT_DRAWABLE_ID.load(Ordering::Relaxed).max(1)
    }

    fn set_global_drawable_id_counter(next_drawable_id: u32) {
        GLOBAL_NEXT_DRAWABLE_ID.store(next_drawable_id.max(1), Ordering::Relaxed);
    }

    fn init_savegame_counter_bridge(&self) {
        register_drawable_id_counter_hooks(
            Some(Arc::new(Self::global_drawable_id_counter)),
            Some(Arc::new(Self::set_global_drawable_id_counter)),
        );
    }

    // Subsystem initialization methods

    fn init_core_subsystems(&mut self) -> GameClientResult<()> {
        log::debug!("Initializing core subsystems");

        Ok(())
    }

    fn init_asset_systems(&mut self) -> GameClientResult<()> {
        log::debug!("Initializing asset management systems");
        let mut asset_config = AssetConfig::default();

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let data_path = cwd.join("Data");
        asset_config.base_path = if data_path.exists() { data_path } else { cwd };

        if let Ok(entries) = std::fs::read_dir(&asset_config.base_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map_or(false, |ext| ext.eq_ignore_ascii_case("big"))
                {
                    asset_config.archive_paths.push(path);
                }
            }
        }

        asset_config.cache_size_mb = 512;
        asset_config.enable_hot_reload = cfg!(debug_assertions);
        asset_config.enable_validation = cfg!(debug_assertions);

        if self.subsystem_manager.asset_manager.is_none() {
            let asset_manager = AssetManager::new(asset_config).map_err(|e| {
                GameClientError::SubsystemError(format!("Asset manager initialization failed: {e}"))
            })?;

            let asset_manager = Arc::new(asset_manager);
            asset_manager.register_hot_reload_callbacks();
            asset_manager.register_streaming_callbacks();
            self.subsystem_manager.asset_manager = Some(asset_manager);
        }

        log::info!("Asset management systems initialized");
        Ok(())
    }

    fn init_input_subsystems(&mut self) -> GameClientResult<()> {
        log::debug!("Initializing input subsystems");

        // Create keyboard
        let keyboard = create_keyboard();
        keyboard.lock().unwrap().init()?;
        self.subsystem_manager.input_keyboard = Some(keyboard);

        // Create mouse
        let mouse = create_mouse();
        mouse.lock().unwrap().init()?;
        register_mouse_backend(mouse.clone());
        self.subsystem_manager.input_mouse = Some(mouse);

        Ok(())
    }

    fn init_localized_ui_resources(&mut self) -> GameClientResult<()> {
        let loaded_strings = GameText::init_runtime_strings().map_err(|err| {
            GameClientError::SubsystemError(format!("GameText init failed: {err}"))
        })?;
        log::debug!("Loaded {loaded_strings} localized GameText strings");

        // C++ parity: mapped images are available before shell/window creation.
        game_engine::common::ini::ini_mapped_image::ImageCollection::load_global(512);
        let imported = sync_mapped_images_from_common();
        log::debug!("Imported {imported} mapped images into client image collection");
        log_startup_shell_mapped_images();

        Ok(())
    }

    fn init_display_subsystems(&mut self) -> GameClientResult<()> {
        log::debug!("Initializing display subsystems");

        self.init_localized_ui_resources()?;

        if self.subsystem_manager.display.is_none() {
            if self.subsystem_manager.platform_context.is_none() {
                let context =
                    PlatformContext::new("Command & Conquer Generals Zero Hour", 1280, 720)
                        .map_err(|e| {
                            GameClientError::SubsystemError(format!(
                                "Platform context initialisation failed: {e}"
                            ))
                        })?;
                self.subsystem_manager.platform_context = Some(context);
            }

            let graphics_context =
                if let Some(context) = self.subsystem_manager.platform_context.as_ref() {
                    let size = context.window.inner_size();
                    log::info!(
                        "Platform context initialised (window {}x{})",
                        size.width,
                        size.height
                    );
                    context.graphics.clone()
                } else {
                    return Err(GameClientError::InitializationFailed(
                        "Platform context missing during display initialisation".to_string(),
                    ));
                };

            let mut display = GraphicsDisplay::new(graphics_context);
            display.init()?;
            let display = Arc::new(Mutex::new(display));
            register_script_display_bridge(Some(Arc::clone(&display)));
            self.subsystem_manager.display = Some(display);
        } else if let Some(display) = self.subsystem_manager.display.as_ref() {
            register_script_display_bridge(Some(Arc::clone(display)));
        }

        if self.subsystem_manager.font_library.is_none() {
            let mut font_library = FontLibrarySubsystem::new();
            font_library.init()?;
            self.subsystem_manager.font_library = Some(Arc::new(Mutex::new(font_library)));
        }

        if self.subsystem_manager.header_templates.is_none() {
            let mut header_templates = HeaderTemplateManagerSubsystem::new();
            header_templates.init()?;
            self.subsystem_manager.header_templates = Some(Arc::new(Mutex::new(header_templates)));
        }

        if self.subsystem_manager.window_manager.is_none() {
            let mut window_manager = WindowManagerSubsystem::new();
            window_manager.init()?;
            self.subsystem_manager.window_manager = Some(Arc::new(Mutex::new(window_manager)));
        }

        {
            let mut shell = get_shell();
            shell.init().map_err(|err| {
                GameClientError::SubsystemError(format!("Shell init failed: {err}"))
            })?;
        }

        if let Some(context) = self.subsystem_manager.platform_context.as_ref() {
            let config = context.graphics.config();
            let renderer = UIRenderer::new(
                context.graphics.device_arc(),
                context.graphics.queue_arc(),
                config.format,
            )
            .map_err(|err| {
                GameClientError::SubsystemError(format!("UI renderer initialization failed: {err}"))
            })?;
            set_ui_renderer(Arc::new(RwLock::new(renderer)));
        }

        if self.subsystem_manager.display_strings.is_none() {
            let mut display_strings = DisplayStringManagerSubsystem::new();
            display_strings.init()?;
            self.subsystem_manager.display_strings = Some(Arc::new(Mutex::new(display_strings)));
        }

        if self.subsystem_manager.hot_key_manager.is_none() {
            let mut hot_keys = HotKeyManagerSubsystem::new();
            hot_keys.init()?;
            self.subsystem_manager.hot_key_manager = Some(Arc::new(Mutex::new(hot_keys)));
        }

        Ok(())
    }

    fn init_audio_subsystems(&mut self) -> GameClientResult<()> {
        log::debug!("Initializing audio subsystems");

        if self.subsystem_manager.audio.is_none() {
            let mut audio = AudioSubsystem::new()
                .map_err(|e| GameClientError::SubsystemError(format!("Audio init failed: {e}")))?;
            audio.init()?;
            let audio_arc = Arc::new(Mutex::new(audio));
            let hook_audio = Arc::clone(&audio_arc);
            register_fx_audio(Box::new(move |event, position| {
                if let Ok(mut guard) = hook_audio.lock() {
                    let _ = guard.play_event(event, position);
                }
            }));
            self.subsystem_manager.audio = Some(audio_arc);
        }

        if let Some(asset_manager) = self.subsystem_manager.asset_manager.as_ref() {
            crate::assets::register_audio_playback_bridge(Arc::clone(asset_manager));
        }

        Ok(())
    }

    fn init_game_subsystems(&mut self) -> GameClientResult<()> {
        log::debug!("Initializing game subsystems");
        crate::snow::register_weather_definition_parser();
        crate::eva::initialize_eva_system()
            .map_err(|err| GameClientError::SubsystemError(format!("Eva init failed: {err}")))?;
        if self.subsystem_manager.terrain_visual.is_none() {
            let mut terrain_visual = TerrainVisualStub::default();
            terrain_visual.init()?;
            self.subsystem_manager.terrain_visual = Some(Arc::new(Mutex::new(terrain_visual)));
        }

        if let Some(terrain_visual) = self.subsystem_manager.terrain_visual.as_ref() {
            let terrain_visual = Arc::clone(terrain_visual);
            let _ = register_terrain_tree_hook(Arc::new(move |event| {
                if let Ok(mut terrain) = terrain_visual.lock() {
                    match event {
                        TerrainTreeEvent::Add(tree) => terrain.add_tree_registration(tree),
                        TerrainTreeEvent::Remove(drawable_id) => {
                            terrain.remove_tree_registration(drawable_id)
                        }
                    }
                }
            }));
        }

        if let Some(asset_manager) = self.subsystem_manager.asset_manager.as_ref() {
            let resolver = Arc::new(AnimationDurationResolver::new(Arc::clone(asset_manager)));
            let _ = register_animation_metadata_hook(Arc::new(move |animation_name| {
                resolver.get_duration_ms(animation_name)
            }));
        }

        init_fx_list_store()
            .map_err(|e| GameClientError::SubsystemError(format!("FXList init failed: {e}")))?;
        crate::fx_list::register_fx_list_manager_bridge();
        if let Err(e) = crate::effects::particle_manager::initialize_particle_system_manager() {
            return Err(GameClientError::SubsystemError(format!(
                "Particle system manager init failed: {e}"
            )));
        }
        crate::effects::particle_manager::register_particle_system_manager_bridge();
        if let Err(e) = initialize_weather_system() {
            return Err(GameClientError::SubsystemError(format!(
                "Weather system init failed: {e}"
            )));
        }

        if self.subsystem_manager.decal_manager.is_none() {
            let decals = Arc::new(Mutex::new(DecalManager::default()));
            register_decal_manager(Arc::clone(&decals));
            let scorch_decals = Arc::clone(&decals);
            let _ = register_scorch_hook(Arc::new(move |position, size, _type_id| {
                if let Ok(mut guard) = scorch_decals.lock() {
                    let scorch_position = Point3::new(position.x, position.y, position.z);
                    guard.create_decal(DecalSettings::scorch_mark(scorch_position, size.max(0.1)));
                }
            }));
            self.subsystem_manager.decal_manager = Some(decals);
        }

        let mut prefs = UserPreferences::new();
        let _ = prefs.load("Options.ini");
        if let Some(value) = prefs.get_string("DynamicGameLOD") {
            game_engine::common::game_lod::set_dynamic_lod_from_string(value);
        }
        if let Some(value) = prefs.get_string("StaticGameLOD") {
            game_engine::common::game_lod::set_static_lod_from_string(value);
        }
        if let Some(value) = prefs.get_string("IdealStaticGameLOD") {
            game_engine::common::game_lod::set_ideal_static_lod_from_string(value);
        }

        if self.subsystem_manager.in_game_ui.is_none() {
            let mut ui = InGameUISubsystem::default();
            ui.init()?;
            let ui_arc = Arc::new(Mutex::new(ui));
            register_in_game_ui_backend(Arc::new(InGameUiHandle::new(ui_arc.clone())));
            self.subsystem_manager.in_game_ui = Some(ui_arc);
        }

        crate::helpers::register_prepare_new_game_hooks();
        crate::helpers::register_observer_audio_locality_hooks();
        self.install_script_action_handler();

        let _ = crate::snow::initialize_snow_manager();

        if self.subsystem_manager.video_player.is_none() {
            let mut video_player = VideoPlayerSubsystem::default();
            video_player.init()?;
            self.subsystem_manager.video_player = Some(Arc::new(Mutex::new(video_player)));
        }

        Ok(())
    }

    fn install_script_action_handler(&self) {
        if let Ok(mut engine_guard) = gamelogic::get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.set_action_handler(Some(Arc::new(GameClientScriptActionHandler::new())));
            }
        }
    }

    fn reset_global_video_player_streams() {
        if let Some(player) = get_video_player() {
            if let Ok(mut guard) = player.lock() {
                if let Some(player) = guard.as_mut() {
                    player.reset();
                }
            }
        }
    }

    fn init_message_translators(&mut self) -> GameClientResult<()> {
        log::debug!("Initializing message translators");
        let mut stream = THE_MESSAGE_STREAM
            .write()
            .map_err(|_| GameClientError::SubsystemError("Message stream lock poisoned".into()))?;

        self.num_translators = 0;

        for (translator, priority) in TranslatorFactory::create_standard_translator_set() {
            let id = stream.attach_translator(translator, priority);
            if self.num_translators < self.translators.len() {
                self.translators[self.num_translators] = id;
                self.num_translators += 1;
            }
        }

        let dispatcher_translator = Arc::new(RwLock::new(DispatcherTranslator::new(Arc::clone(
            &self.message_dispatcher,
        ))));
        let dispatcher_id = stream.attach_translator(dispatcher_translator, 999_999_999);
        if self.num_translators < self.translators.len() {
            self.translators[self.num_translators] = dispatcher_id;
            self.num_translators += 1;
        }

        Ok(())
    }

    fn init_network_bridge(&mut self) {
        if self.network_bridge.is_some() {
            return;
        }

        match NetworkBridgeHandle::install() {
            Some(handle) => {
                log::info!("Network command bridge installed");
                self.network_bridge = Some(handle);
            }
            None => {
                log::debug!(
                    "Network interface unavailable; network command bridge not installed yet"
                );
            }
        }
    }

    // Update methods

    fn create_frame_tick_message(&self) -> GameClientResult<()> {
        let mut stream = THE_MESSAGE_STREAM
            .write()
            .map_err(|_| GameClientError::SubsystemError("Message stream lock poisoned".into()))?;

        let frame = self.frame;
        let message = stream.append_message(GameMessageType::FrameTick(frame));
        message.append_timestamp_argument(frame);
        Ok(())
    }

    fn pump_message_stream(&self) -> GameClientResult<()> {
        let completed_messages = {
            let mut stream = THE_MESSAGE_STREAM.write().map_err(|_| {
                GameClientError::SubsystemError("Message stream lock poisoned".into())
            })?;
            stream.propagate_messages().map_err(|e| {
                GameClientError::SubsystemError(format!("Message stream update failed: {e}"))
            })?
        };

        if !completed_messages.is_empty() {
            let command_list_arc = get_command_list();
            let mut command_list = command_list_arc.write().map_err(|_| {
                GameClientError::SubsystemError("Command list lock poisoned".into())
            })?;
            command_list.append_message_list(completed_messages);
        }

        with_recorder_mut(|recorder| {
            recorder.set_current_frame(self.frame);
            recorder.update();
        });

        self.flush_command_list_to_logic()
    }

    fn flush_command_list_to_logic(&self) -> GameClientResult<()> {
        let command_list_arc = get_command_list();
        let commands = {
            let mut command_list = command_list_arc.write().map_err(|_| {
                GameClientError::SubsystemError("Command list lock poisoned".into())
            })?;
            command_list.reset_frame_counter();
            command_list.get_all_commands()
        };

        if commands.is_empty() {
            return Ok(());
        }

        route_commands_to_gamelogic(commands, self.frame).map_err(|err| {
            GameClientError::SubsystemError(format!("Failed to route commands: {err}"))
        })?;

        Ok(())
    }

    fn init_recorder_bridge(&self) {
        init_recorder();

        let command_source: Arc<dyn Fn() -> Vec<GameMessage> + Send + Sync> = Arc::new(|| {
            let command_list_arc = get_command_list();
            let read_result = command_list_arc.read();
            match read_result {
                Ok(command_list) => command_list.snapshot_messages(),
                Err(_) => Vec::new(),
            }
        });

        let command_sink: Arc<dyn Fn(GameMessage) + Send + Sync> = Arc::new(|message| {
            let command_list_arc = get_command_list();
            let write_result = command_list_arc.write();
            if let Ok(mut command_list) = write_result {
                command_list.append_message(message);
            }
        });

        let command_cull: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {
            let command_list_arc = get_command_list();
            let write_result = command_list_arc.write();
            if let Ok(mut command_list) = write_result {
                command_list.retain_messages(|msg| {
                    let msg_type = msg.get_type().clone();
                    !(is_network_command_message(msg_type.clone())
                        && !matches!(msg_type, GameMessageType::LogicCRC(_)))
                });
            }
        });

        with_recorder_mut(|recorder| {
            recorder.set_command_source(Some(command_source));
            recorder.set_command_sink(Some(command_sink));
            recorder.set_command_cull(Some(command_cull));
            recorder.set_game_mode_provider(Some(Arc::new(TheGameLogic::get_game_mode)));
        });
    }

    fn update_input(&mut self) -> GameClientResult<()> {
        if let Some(ref keyboard) = self.subsystem_manager.input_keyboard {
            keyboard.lock().unwrap().update();
        }

        if let Some(ref mouse) = self.subsystem_manager.input_mouse {
            mouse.lock().unwrap().update();
        }

        Ok(())
    }

    fn update_audio(&mut self) -> GameClientResult<()> {
        if let Some(ref audio) = self.subsystem_manager.audio {
            audio.lock().unwrap().update()?;
        }
        Ok(())
    }

    fn update_drawables(&mut self, delta_time: f32) -> GameClientResult<()> {
        let frame = self.frame;

        // Update all client-owned drawables
        for drawable in self.drawable_map.values_mut() {
            drawable.update(delta_time);
        }

        // Update GameLogic drawables as well.
        self.iterate_objects_with_drawables(|obj_ref| {
            let Ok(mut obj) = obj_ref.write() else {
                return;
            };
            if let Some(drawable) = obj.get_drawable() {
                if let Ok(mut drawable_guard) = drawable.write() {
                    let _ = drawable_guard.update(delta_time, frame);
                }
            }
        })?;
        Ok(())
    }

    fn update_effects(&mut self, delta_time: f32) -> GameClientResult<()> {
        if let Some(ref decals) = self.subsystem_manager.decal_manager {
            if let Ok(mut guard) = decals.lock() {
                let config = EffectsConfig::default();
                guard.update(delta_time, &config);
            }
        }
        if let Ok(mut weather_guard) = get_weather_system_mut() {
            if let Some(weather) = weather_guard.as_mut() {
                let camera_pos = with_tactical_view_ref(|view| view.get_3d_camera_position());
                weather.update(
                    delta_time,
                    Point3::new(camera_pos.x, camera_pos.y, camera_pos.z),
                );
            }
        }
        Ok(())
    }

    fn should_freeze_visual_time(&self) -> bool {
        let camera_frozen = with_tactical_view_ref(|view| {
            view.is_time_frozen() && !view.is_camera_movement_finished()
        });
        camera_frozen || TheScriptEngine::is_time_frozen_script() || TheGameLogic::is_game_paused()
    }

    fn preload_template_assets_from_factory(
        &mut self,
        time_of_day: TimeOfDay,
    ) -> GameClientResult<()> {
        let preload_everything = get_global_data()
            .map(|global| global.read().preload_everything)
            .unwrap_or(false);

        let Ok(thing_factory_guard) = get_thing_factory() else {
            return Ok(());
        };
        let Some(thing_factory) = thing_factory_guard.as_ref() else {
            return Ok(());
        };

        let mut templates_to_preload: Vec<Arc<ThingTemplate>> = Vec::new();

        let mut current = thing_factory.first_template().cloned();
        while let Some(template) = current {
            if Self::should_preload_template(template.as_ref(), preload_everything) {
                templates_to_preload.push(template.clone());
            }

            let mut override_template = template.get_next_override();
            while let Some(override_entry) = override_template {
                if Self::should_preload_template(override_entry.as_ref(), preload_everything) {
                    templates_to_preload.push(override_entry.clone());
                }
                override_template = override_entry.get_next_override();
            }

            current = template.get_next_template().clone();
        }

        drop(thing_factory_guard);

        for template in templates_to_preload {
            self.preload_template_assets(template.as_ref(), time_of_day);
        }

        Ok(())
    }

    fn should_preload_template(template: &ThingTemplate, preload_everything: bool) -> bool {
        // C++ parity: GameClient.cpp::preloadAssets checks KINDOF_PRELOAD unless preloadEverything is forced.
        const KINDOF_PRELOAD: u32 = 26;
        preload_everything || template.is_kind_of(KINDOF_PRELOAD)
    }

    fn preload_template_assets(&mut self, template: &ThingTemplate, time_of_day: TimeOfDay) {
        // C++ parity: create temp drawable from template, preload, then destroy.
        let temp_id = match self.create_drawable_from_template(template) {
            Ok(id) => id,
            Err(err) => {
                log::warn!(
                    "Failed to create temporary preload drawable for template '{}': {}",
                    template.get_name(),
                    err
                );
                return;
            }
        };

        if let Some(drawable) = self.find_drawable_by_id(temp_id) {
            if let Err(err) = drawable.preload_assets(time_of_day) {
                log::warn!(
                    "Failed to preload assets for template '{}': {}",
                    template.get_name(),
                    err
                );
            }
        }

        if let Err(err) = self.destroy_drawable(temp_id) {
            log::warn!(
                "Failed to destroy temporary preload drawable for template '{}': {}",
                template.get_name(),
                err
            );
        }
    }

    fn update_display(&mut self) -> GameClientResult<()> {
        if let Some(ref display) = self.subsystem_manager.display {
            let mut display = display.lock().unwrap();
            display.update()?;
            display.draw()?;
        }
        Ok(())
    }

    fn update_startup_movie_display(&mut self) -> GameClientResult<()> {
        if let Some(ref display) = self.subsystem_manager.display {
            let mut display = display.lock().unwrap();
            display.draw()?;
            display.update()?;
        }
        Ok(())
    }

    fn startup_movies_active(&self) -> bool {
        get_global_data()
            .map(|data| {
                let data = data.read();
                data.play_intro || data.after_intro
            })
            .unwrap_or(false)
    }

    fn should_activate_shell_after_startup(&self) -> bool {
        let Some(global_data) = get_global_data() else {
            return true;
        };
        let global = global_data.read();
        global.pending_file.is_empty()
    }

    fn activate_shell_after_startup(&self) -> GameClientResult<()> {
        if !self.should_activate_shell_after_startup() {
            return Ok(());
        }

        log::info!("Activating shell after startup movie flow");
        let mut shell = get_shell();
        shell.show_shell(true).map_err(|err| {
            GameClientError::SubsystemError(format!(
                "Failed to activate shell after startup movies: {}",
                err
            ))
        })?;
        if shell.get_screen_count() == 0 {
            shell.push("Menus/MainMenu.wnd", false).map_err(|err| {
                GameClientError::SubsystemError(format!(
                    "Failed to push MainMenu.wnd after startup movies: {}",
                    err
                ))
            })?;
        }
        Ok(())
    }

    fn update_startup_movies(&mut self) -> GameClientResult<()> {
        let Some(global_data) = get_global_data() else {
            return Ok(());
        };
        let Some(display_arc) = self.subsystem_manager.display.as_ref().cloned() else {
            return Ok(());
        };

        let mut display = display_arc
            .lock()
            .map_err(|_| GameClientError::SubsystemError("Display lock poisoned".to_string()))?;
        if display.is_movie_playing() {
            return Ok(());
        }

        let mut global = global_data.write();
        let Some(action) = startup_movie_action(
            global.play_intro,
            global.after_intro,
            global.play_sizzle,
            self.startup_sizzle_pending,
            prefers_low_res_movies(),
        ) else {
            return Ok(());
        };

        match action {
            StartupMovieAction::PlayLogo(movie_name) => {
                display.play_logo_movie(movie_name.to_string(), 5000, 3000);
                global.play_intro = false;
                global.after_intro = true;
                self.startup_sizzle_pending = true;
            }
            StartupMovieAction::PlaySizzle(movie_name) => {
                global.allow_exit_out_of_movies = true;
                if display.play_movie(movie_name.to_string()) {
                    self.startup_sizzle_pending = false;
                    return Ok(());
                }
                self.startup_sizzle_pending = false;
                global.break_the_movie = true;
                global.after_intro = false;
                drop(global);
                self.activate_shell_after_startup()?;
            }
            StartupMovieAction::FinalizeStartup => {
                global.break_the_movie = true;
                global.allow_exit_out_of_movies = true;
                global.after_intro = false;
                drop(global);
                self.activate_shell_after_startup()?;
            }
        }
        Ok(())
    }

    fn ensure_shell_visible(&self) -> GameClientResult<()> {
        if !self.should_activate_shell_after_startup() {
            return Ok(());
        }

        let mut shell = get_shell();
        if shell.get_screen_count() == 0 || !shell.is_shell_active() {
            log::info!(
                "Ensuring shell visibility: screen_count={}, shell_active={}",
                shell.get_screen_count(),
                shell.is_shell_active()
            );
            shell.show_shell(true).map_err(|err| {
                GameClientError::SubsystemError(format!(
                    "Failed to ensure shell visibility: {}",
                    err
                ))
            })?;
        }
        if shell.get_screen_count() == 0 {
            shell.push("Menus/MainMenu.wnd", false).map_err(|err| {
                GameClientError::SubsystemError(format!(
                    "Failed to restore MainMenu.wnd for shell visibility: {}",
                    err
                ))
            })?;
        }
        Ok(())
    }

    fn update_ui(&mut self) -> GameClientResult<()> {
        {
            let mut shell = get_shell();
            shell.update().map_err(|err| {
                GameClientError::SubsystemError(format!("Shell update failed: {err}"))
            })?;
        }

        if let Some(ref ui) = self.subsystem_manager.in_game_ui {
            ui.lock().unwrap().update()?;
        }

        if let Some(ref window_manager) = self.subsystem_manager.window_manager {
            window_manager.lock().unwrap().update()?;
        }

        if let Some(ref video_player) = self.subsystem_manager.video_player {
            video_player.lock().unwrap().update()?;
        }

        crate::eva::update_eva_system();

        Ok(())
    }

    fn process_beacon_notifications(&self) -> GameClientResult<()> {
        let notifications = beacon_display::drain_notifications();
        if notifications.is_empty() {
            return Ok(());
        }

        for notification in notifications {
            if let Some(ref ui) = self.subsystem_manager.in_game_ui {
                let mut ui_guard = ui.lock().map_err(|_| {
                    GameClientError::SubsystemError("In-game UI lock poisoned".to_string())
                })?;
                ui_guard
                    .handle_beacon_notification(&notification)
                    .map_err(|err| {
                        GameClientError::SubsystemError(format!(
                            "Failed to handle beacon notification: {err}"
                        ))
                    })?;
            } else {
                log::info!("Beacon event: {:?}", notification);
            }
        }

        Ok(())
    }

    fn set_frame_rate(&mut self, duration_per_frame: Duration) -> GameClientResult<()> {
        if duration_per_frame.is_zero() {
            return Err(GameClientError::InvalidOperation(
                "frame duration must be greater than zero".to_string(),
            ));
        }

        self.target_frame_duration = duration_per_frame;
        log::info!(
            "Target frame duration set to {:?} (~{:.2} FPS)",
            duration_per_frame,
            1.0 / duration_per_frame.as_secs_f64()
        );
        Ok(())
    }
}

fn log_startup_shell_mapped_images() {
    let collection = get_mapped_image_collection();
    let collection = collection.read();
    for name in [
        "MainMenuBackdrop",
        "MainMenuPulse",
        "GeneralsLogo",
        "MainMenuRuler",
    ] {
        match collection.find_image_by_name(name) {
            Some(image) => log::debug!(
                "startup mapped image: name={} file={} uv=({}, {}, {}, {}) size={}x{} tex={}x{}",
                name,
                image.get_filename(),
                image.get_uv().min.x,
                image.get_uv().min.y,
                image.get_uv().max.x,
                image.get_uv().max.y,
                image.get_image_width(),
                image.get_image_height(),
                image.get_texture_size().x,
                image.get_texture_size().y,
            ),
            None => log::debug!("startup mapped image missing: {name}"),
        }
    }
}

impl Snapshotable for GameClient {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 3;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        let mut frame = self.frame;
        xfer.xfer_unsigned_int(&mut frame)
            .map_err(|e| e.to_string())?;
        self.frame = frame;

        self.xfer_drawable_toc(xfer)?;

        let save_entries = if xfer.is_writing() {
            self.collect_saveable_drawables_sorted()?
        } else {
            self.drawable_map.clear();
            self.drawable_object_map.clear();
            Vec::new()
        };

        let mut drawable_count: u16 = save_entries
            .len()
            .try_into()
            .map_err(|_| "Too many drawables to serialize".to_string())?;

        xfer.xfer_unsigned_short(&mut drawable_count)
            .map_err(|e| e.to_string())?;

        if xfer.is_writing() {
            let toc_lookup: HashMap<String, u16> = self
                .drawable_toc
                .iter()
                .map(|entry| (entry.name.clone(), entry.id))
                .collect();

            for (drawable_id, template_name) in save_entries {
                let Some(drawable) = self.drawable_map.get_mut(&drawable_id) else {
                    return Err(format!(
                        "Drawable '{}' disappeared during save serialization",
                        drawable_id.0
                    ));
                };

                let mut toc_id = toc_lookup
                    .get(&template_name)
                    .copied()
                    .ok_or_else(|| "Drawable TOC entry not found".to_string())?;
                xfer.xfer_unsigned_short(&mut toc_id)
                    .map_err(|e| e.to_string())?;

                xfer.begin_block().map_err(|e| format!("{:?}", e))?;

                let mut object_id: ObjectID = drawable.get_object_id().unwrap_or(INVALID_ID);
                xfer.xfer_unsigned_int(&mut object_id)
                    .map_err(|e| e.to_string())?;

                Self::xfer_drawable_snapshot(drawable.as_mut(), xfer)?;

                xfer.end_block().map_err(|e| format!("{:?}", e))?;
            }
        } else {
            let factory_guard = get_thing_factory().map_err(|_| "ThingFactory lock failed")?;
            let factory = factory_guard
                .as_ref()
                .ok_or_else(|| "ThingFactory not initialized".to_string())?;

            for _ in 0..drawable_count {
                let mut toc_id: u16 = 0;
                xfer.xfer_unsigned_short(&mut toc_id)
                    .map_err(|e| e.to_string())?;

                let toc_name = self
                    .find_toc_entry_by_id(toc_id)
                    .map(|entry| entry.name.clone())
                    .ok_or_else(|| "Drawable TOC entry not found for id".to_string())?;

                let data_size = xfer.begin_block().map_err(|e| format!("{:?}", e))?;

                let Some(template) = factory.find_template(&toc_name, false) else {
                    xfer.skip(data_size).map_err(|e| format!("{:?}", e))?;
                    continue;
                };

                let mut object_id: ObjectID = INVALID_ID;
                xfer.xfer_unsigned_int(&mut object_id)
                    .map_err(|e| e.to_string())?;

                if object_id != INVALID_ID && OBJECT_REGISTRY.get_object(object_id).is_none() {
                    return Err(format!(
                        "GameClient::xfer - Cannot find object '{}' for drawable '{}'",
                        object_id, toc_name
                    ));
                }

                let mut reuse_id = None;
                if object_id != INVALID_ID {
                    if let Some(existing_id) = self.get_drawable_for_object(object_id) {
                        reuse_id = Some(existing_id);
                    }
                }

                let mut drawable = if let Some(existing_id) = reuse_id {
                    let needs_replace = self
                        .drawable_map
                        .get(&existing_id)
                        .map(|existing| {
                            !Self::drawable_matches_saved_template(
                                existing.as_ref(),
                                &template,
                                factory,
                            )
                        })
                        .unwrap_or(true);
                    if needs_replace {
                        self.destroy_drawable(existing_id)
                            .map_err(|e| e.to_string())?;
                        None
                    } else {
                        self.drawable_map.remove(&existing_id)
                    }
                } else {
                    None
                };

                if drawable.is_none() {
                    let created_id = self
                        .create_drawable_from_template(template.as_ref())
                        .map_err(|e| {
                            format!(
                                "GameClient::xfer - Unable to create drawable for '{}': {}",
                                template.get_name(),
                                e
                            )
                        })?;
                    let mut created = self.drawable_map.remove(&created_id).ok_or_else(|| {
                        format!(
                            "GameClient::xfer - Created drawable '{}' was not registered",
                            created_id.0
                        )
                    })?;
                    if object_id != INVALID_ID {
                        created.set_object_id(Some(object_id));
                    }
                    drawable = Some(created);
                }

                let mut drawable = drawable.expect("drawable exists");
                Self::xfer_drawable_snapshot(drawable.as_mut(), xfer)?;

                let id = drawable.get_id();
                if let Some(object_id) = drawable.get_object_id() {
                    self.drawable_object_map.insert(object_id, id);
                }
                self.drawable_map.insert(id, drawable);

                xfer.end_block().map_err(|e| format!("{:?}", e))?;

                if object_id != INVALID_ID {
                    if OBJECT_REGISTRY.get_object(object_id).is_some() {
                        let _ = self.bind_drawable_to_object(id, object_id);
                    } else {
                        return Err(format!(
                            "GameClient::xfer - Drawable '{}' references missing object ID '{}'",
                            toc_name, object_id
                        ));
                    }
                }
            }
        }

        if xfer.is_reading() {
            self.load_post_process()?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.drawable_object_map.clear();
        let mut next_drawable_id = self.next_drawable_id.0.max(1);

        for drawable in self.drawable_map.values() {
            let id = drawable.get_id();
            if id.0 >= next_drawable_id {
                next_drawable_id = id.0.saturating_add(1).max(1);
            }
            if let Some(object_id) = drawable.get_object_id() {
                self.drawable_object_map.insert(object_id, id);
            }
        }

        // C++ scans the global drawable list; include GameLogic-owned drawables as well
        // so the next ID counter cannot regress after load.
        for obj_ref in OBJECT_REGISTRY.get_all_objects() {
            let Ok(obj_guard) = obj_ref.read() else {
                continue;
            };
            let Some(drawable_ref) = obj_guard.get_drawable() else {
                continue;
            };
            let Ok(drawable_guard) = drawable_ref.read() else {
                continue;
            };

            let drawable_id = drawable_guard.get_drawable_id();
            if drawable_id >= next_drawable_id {
                next_drawable_id = drawable_id.saturating_add(1).max(1);
            }

            let object_id = drawable_guard.get_object_id();
            if object_id != INVALID_ID {
                self.drawable_object_map
                    .insert(object_id, DrawableId(drawable_id));
            }
        }

        self.next_drawable_id = DrawableId(next_drawable_id.max(1));
        self.set_drawable_id_counter(self.next_drawable_id.0);
        Ok(())
    }
}

impl Drop for GameClient {
    fn drop(&mut self) {
        log::info!("GameClient shutting down");
        GameClient::reset_global_video_player_streams();
        reset_script_action_runtime_state();
        register_script_display_bridge(None);
        shutdown_video_player();

        // Clear all drawables (they'll be dropped automatically)
        self.drawable_map.clear();

        // Subsystems will be dropped automatically through Arc

        log::info!("GameClient shutdown complete");
    }
}

// Subsystem manager implementation

impl SubsystemManager {
    fn new() -> Self {
        Self {
            display: None,
            audio: None,
            input_keyboard: None,
            input_mouse: None,
            terrain_visual: None,
            window_manager: None,
            font_library: None,
            header_templates: None,
            display_strings: None,
            hot_key_manager: None,
            in_game_ui: None,
            video_player: None,
            decal_manager: None,
            asset_manager: None,
            platform_context: None,
        }
    }

    fn reset_all(&mut self) -> GameClientResult<()> {
        if let Some(ref display) = self.display {
            display.lock().unwrap().reset()?;
        }

        if let Some(ref audio) = self.audio {
            audio.lock().unwrap().reset()?;
        }

        if let Some(ref keyboard) = self.input_keyboard {
            keyboard.lock().unwrap().reset()?;
        }

        if let Some(ref mouse) = self.input_mouse {
            mouse.lock().unwrap().reset()?;
        }

        if let Some(ref terrain) = self.terrain_visual {
            terrain.lock().unwrap().reset()?;
        }

        if let Some(ref window_manager) = self.window_manager {
            window_manager.lock().unwrap().reset()?;
        }

        if let Some(ref font_library) = self.font_library {
            font_library.lock().unwrap().reset()?;
        }

        if let Some(ref header_templates) = self.header_templates {
            header_templates.lock().unwrap().reset()?;
        }

        if let Some(ref display_strings) = self.display_strings {
            display_strings.lock().unwrap().reset()?;
        }

        if let Some(ref hot_keys) = self.hot_key_manager {
            hot_keys.lock().unwrap().reset()?;
        }

        if let Some(ref ui) = self.in_game_ui {
            ui.lock().unwrap().reset()?;
        }

        if let Some(ref video) = self.video_player {
            video.lock().unwrap().reset()?;
        }

        if let Some(ref decals) = self.decal_manager {
            if let Ok(mut guard) = decals.lock() {
                guard.clear_all();
            }
        }

        crate::eva::reset_eva_system();

        Ok(())
    }
}

// Message dispatcher implementation

impl GameClientMessageDispatcher {
    pub fn new() -> Self {
        Self {
            message_filters: Vec::new(),
        }
    }

    pub fn translate_game_message(&self, msg: &GameMessage) -> GameMessageDisposition {
        let msg_type = msg.get_type().clone();
        // Keep network messages (placeholder until network layer implemented)
        if self.is_network_message(&msg_type) {
            return GameMessageDisposition::KeepMessage;
        }

        // Keep game control messages
        match msg_type {
            GameMessageType::NewGame
            | GameMessageType::ClearGameData
            | GameMessageType::FrameTick(_) => GameMessageDisposition::KeepMessage,
            _ => GameMessageDisposition::DestroyMessage,
        }
    }

    fn is_network_message(&self, msg_type: &GameMessageType) -> bool {
        is_network_command_message(msg_type.clone())
    }

    pub fn add_filter(&mut self, filter: Box<dyn MessageFilter + Send + Sync>) {
        self.message_filters.push(filter);
    }
}

impl Default for GameClientMessageDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::is_network_command_message;
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use game_engine::common::thing::{
        get_thing_factory, init_thing_factory, ThingFactory as CommonThingFactory,
    };
    use game_engine::common::{ini::get_global_data, recorder::Recorder};
    use gamelogic::common::types::ObjectStatusMaskType;
    use gamelogic::thing_template::DefaultThingTemplate as LogicDefaultThingTemplate;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};

    fn serialize_client(client: &mut GameClient) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut xfer = XferSave::new(cursor, 3);
            client
                .xfer(&mut xfer)
                .expect("game client serialization should succeed");
        }
        bytes
    }

    fn deserialize_client(bytes: &[u8]) -> GameClient {
        let mut loaded = GameClient::new().expect("game client creation should succeed");
        let cursor = Cursor::new(bytes.to_vec());
        let mut xfer = XferLoad::new(cursor, 3);
        loaded
            .xfer(&mut xfer)
            .expect("game client deserialization should succeed");
        loaded
    }

    fn read_utf16_z_end(bytes: &[u8], mut offset: usize) -> usize {
        loop {
            assert!(
                offset + 1 < bytes.len(),
                "Malformed replay header while reading UTF-16 string"
            );
            let code_unit = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
            offset += 2;
            if code_unit == 0 {
                return offset;
            }
        }
    }

    fn replay_version_offsets(bytes: &[u8]) -> (usize, usize, usize, usize, usize, usize) {
        // Magic + fixed replay stats block
        let mut offset = 6 + 8 + 8 + 4 + 1 + 1 + 8;
        // Replay name
        offset = read_utf16_z_end(bytes, offset);
        // Timestamp
        offset += 8;
        let version_string_start = offset;
        let version_string_end = read_utf16_z_end(bytes, offset);
        let version_time_start = version_string_end;
        let version_time_end = read_utf16_z_end(bytes, version_time_start);
        let version_number_offset = version_time_end;
        let exe_crc_offset = version_number_offset + 4;
        let ini_crc_offset = version_number_offset + 8;
        (
            version_string_start,
            version_string_end,
            version_time_start,
            version_number_offset,
            exe_crc_offset,
            ini_crc_offset,
        )
    }

    fn mutate_utf16_first_code_unit(bytes: &mut [u8], start: usize, end: usize, field_name: &str) {
        assert!(
            end >= start + 4,
            "Replay {field_name} field is unexpectedly empty"
        );
        let current = u16::from_le_bytes([bytes[start], bytes[start + 1]]);
        let next = current.wrapping_add(1).max(1);
        bytes[start..start + 2].copy_from_slice(&next.to_le_bytes());
    }

    fn write_variant(
        base_path: &Path,
        replays_dir: &Path,
        variant_name: &str,
        mutate: impl FnOnce(&mut Vec<u8>),
    ) -> PathBuf {
        let mut bytes = std::fs::read(base_path).expect("base replay should be readable");
        mutate(&mut bytes);
        let variant_path = replays_dir.join(variant_name);
        std::fs::write(&variant_path, bytes).expect("variant replay should be writable");
        variant_path
    }

    fn ensure_templates_registered(names: &[&str]) {
        let _ = init_thing_factory();
        let mut guard = get_thing_factory().expect("thing factory lock should be available");
        let factory = guard
            .as_mut()
            .expect("thing factory should be initialized for save/load tests");
        for &name in names {
            if factory.find_template(name, false).is_none() {
                factory.new_template(name);
            }
        }
    }

    fn insert_basic_drawable_for_test(
        client: &mut GameClient,
        id: u32,
        template_name: &str,
        position: Vector3,
    ) {
        let drawable_id = DrawableId(id);
        let mut drawable = BasicDrawable::new(drawable_id);
        drawable.set_id(drawable_id);
        drawable.set_template_name(Some(template_name.to_string()));
        drawable.set_position(position);
        client.drawable_map.insert(drawable_id, Box::new(drawable));
    }

    #[test]
    fn test_drawable_id_creation() {
        let id = DrawableId(42);
        assert!(id.is_valid());
        assert_eq!(id.0, 42);

        let invalid = DrawableId::INVALID;
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_game_client_creation() {
        let client = GameClient::new();
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.get_frame(), 0);
        assert!(!client.initialized);
    }

    #[test]
    fn test_startup_movie_action_prefers_logo_before_after_intro() {
        assert_eq!(
            startup_movie_action(true, true, true, true, false),
            Some(StartupMovieAction::PlayLogo("EALogoMovie"))
        );
    }

    #[test]
    fn test_startup_movie_action_uses_low_res_variants() {
        assert_eq!(
            startup_movie_action(true, false, false, false, true),
            Some(StartupMovieAction::PlayLogo("EALogoMovie640"))
        );
        assert_eq!(
            startup_movie_action(false, true, true, true, true),
            Some(StartupMovieAction::PlaySizzle("Sizzle640"))
        );
    }

    #[test]
    fn test_startup_movie_action_only_plays_sizzle_when_pending() {
        assert_eq!(
            startup_movie_action(false, true, true, true, false),
            Some(StartupMovieAction::PlaySizzle("Sizzle"))
        );
        assert_eq!(
            startup_movie_action(false, true, true, false, false),
            Some(StartupMovieAction::FinalizeStartup)
        );
    }

    #[test]
    fn test_startup_movie_action_ignores_sizzle_when_after_intro_is_clear() {
        assert_eq!(startup_movie_action(false, false, true, true, false), None);
    }

    #[test]
    fn test_drawable_id_allocation() {
        let mut client = GameClient::new().unwrap();

        let id1 = client.alloc_drawable_id();
        let id2 = client.alloc_drawable_id();

        assert_ne!(id1, id2);
        assert_eq!(id1.0 + 1, id2.0);
    }

    #[test]
    fn test_register_drawable_replaces_object_lookup_owner() {
        let mut client = GameClient::new().unwrap();

        let mut first = BasicDrawable::new(DrawableId::INVALID);
        first.set_object_id(Some(77));
        let first_id = client.register_drawable(Box::new(first)).unwrap();

        let mut second = BasicDrawable::new(DrawableId::INVALID);
        second.set_object_id(Some(77));
        let second_id = client.register_drawable(Box::new(second)).unwrap();

        assert_eq!(client.get_drawable_for_object(77), Some(second_id));
        assert_eq!(
            client
                .find_drawable_by_id(first_id)
                .and_then(|d| d.get_object_id()),
            None
        );
    }

    #[test]
    fn test_bind_drawable_to_object_rebinds_and_destroy_keeps_new_owner() {
        let mut client = GameClient::new().unwrap();

        let first_id = client
            .register_drawable(Box::new(BasicDrawable::new(DrawableId::INVALID)))
            .unwrap();
        let second_id = client
            .register_drawable(Box::new(BasicDrawable::new(DrawableId::INVALID)))
            .unwrap();

        client.bind_drawable_to_object(first_id, 99).unwrap();
        client.bind_drawable_to_object(second_id, 99).unwrap();

        assert_eq!(client.get_drawable_for_object(99), Some(second_id));
        assert_eq!(
            client
                .find_drawable_by_id(first_id)
                .and_then(|d| d.get_object_id()),
            None
        );

        client.destroy_drawable(first_id).unwrap();
        assert_eq!(client.get_drawable_for_object(99), Some(second_id));
    }

    #[test]
    fn test_snapshot_serialization_is_deterministic_for_same_state() {
        let mut client = GameClient::new().unwrap();

        let mut first = BasicDrawable::new(DrawableId::INVALID);
        first.set_template_name(Some("Tank".to_string()));
        first.set_position(Vector3::new(10.0, 20.0, 0.0));
        client.register_drawable(Box::new(first)).unwrap();

        let mut second = BasicDrawable::new(DrawableId::INVALID);
        second.set_template_name(Some("Jeep".to_string()));
        second.set_position(Vector3::new(-5.0, 4.0, 0.0));
        client.register_drawable(Box::new(second)).unwrap();

        let mut skipped = BasicDrawable::new(DrawableId::INVALID);
        skipped.set_template_name(Some("ShouldSkip".to_string()));
        let mut status = skipped.get_status();
        status.set(DrawableStatus::NO_SAVE);
        skipped.set_status(status);
        client.register_drawable(Box::new(skipped)).unwrap();

        let first_save = serialize_client(&mut client);
        let second_save = serialize_client(&mut client);
        assert_eq!(first_save, second_save);
    }

    #[test]
    fn test_snapshot_serialization_is_stable_across_drawable_hashmap_insertion_order() {
        let mut client_a = GameClient::new().unwrap();
        insert_basic_drawable_for_test(&mut client_a, 100, "Tank", Vector3::new(10.0, 20.0, 0.0));
        insert_basic_drawable_for_test(&mut client_a, 10, "Jeep", Vector3::new(-2.0, 3.0, 0.0));
        insert_basic_drawable_for_test(&mut client_a, 55, "Humvee", Vector3::new(1.0, 9.0, 0.0));

        let mut client_b = GameClient::new().unwrap();
        insert_basic_drawable_for_test(&mut client_b, 55, "Humvee", Vector3::new(1.0, 9.0, 0.0));
        insert_basic_drawable_for_test(&mut client_b, 100, "Tank", Vector3::new(10.0, 20.0, 0.0));
        insert_basic_drawable_for_test(&mut client_b, 10, "Jeep", Vector3::new(-2.0, 3.0, 0.0));

        let bytes_a = serialize_client(&mut client_a);
        let bytes_b = serialize_client(&mut client_b);
        assert_eq!(bytes_a, bytes_b);
    }

    #[test]
    fn test_snapshot_round_trip_serialization_is_stable() {
        ensure_templates_registered(&["RoundTripAlpha", "RoundTripBeta", "RoundTripGamma"]);

        let mut original = GameClient::new().unwrap();
        insert_basic_drawable_for_test(
            &mut original,
            30,
            "RoundTripAlpha",
            Vector3::new(1.0, 2.0, 0.0),
        );
        insert_basic_drawable_for_test(
            &mut original,
            5,
            "RoundTripBeta",
            Vector3::new(-4.0, 7.5, 0.0),
        );
        insert_basic_drawable_for_test(
            &mut original,
            77,
            "RoundTripGamma",
            Vector3::new(9.0, -3.0, 0.0),
        );

        let first_save = serialize_client(&mut original);
        let mut loaded = deserialize_client(&first_save);
        let second_save = serialize_client(&mut loaded);

        assert_eq!(first_save, second_save);
    }

    #[test]
    fn test_snapshot_serialization_is_stable_across_many_insertion_permutations() {
        let fixtures: Vec<(u32, &str, Vector3)> = vec![
            (41, "Alpha", Vector3::new(1.0, 2.0, 0.0)),
            (7, "Beta", Vector3::new(3.0, -2.0, 0.0)),
            (18, "Alpha", Vector3::new(-4.0, 5.0, 0.0)),
            (99, "Gamma", Vector3::new(6.0, 1.0, 0.0)),
            (3, "Delta", Vector3::new(-7.0, -8.0, 0.0)),
            (64, "Gamma", Vector3::new(2.5, 9.5, 0.0)),
            (12, "Epsilon", Vector3::new(0.0, 0.0, 0.0)),
            (55, "Beta", Vector3::new(8.0, -3.0, 0.0)),
        ];

        let mut baseline_client = GameClient::new().unwrap();
        for (id, name, pos) in &fixtures {
            insert_basic_drawable_for_test(&mut baseline_client, *id, name, *pos);
        }
        let baseline = serialize_client(&mut baseline_client);

        for seed in 0_u64..32_u64 {
            let mut indices: Vec<usize> = (0..fixtures.len()).collect();
            let mut state = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
            for i in (1..indices.len()).rev() {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let j = (state as usize) % (i + 1);
                indices.swap(i, j);
            }

            let mut client = GameClient::new().unwrap();
            for idx in indices {
                let (id, name, pos) = fixtures[idx];
                insert_basic_drawable_for_test(&mut client, id, name, pos);
            }

            let bytes = serialize_client(&mut client);
            assert_eq!(
                bytes, baseline,
                "serialization drift for permutation seed {}",
                seed
            );
        }
    }

    #[test]
    fn test_collect_saveable_drawables_sorted_orders_by_drawable_id_and_skips_nonsave() {
        let mut client = GameClient::new().unwrap();
        insert_basic_drawable_for_test(&mut client, 7, "Seven", Vector3::new(0.0, 0.0, 0.0));
        insert_basic_drawable_for_test(&mut client, 2, "Two", Vector3::new(0.0, 0.0, 0.0));
        insert_basic_drawable_for_test(&mut client, 5, "Five", Vector3::new(0.0, 0.0, 0.0));

        let mut skipped = BasicDrawable::new(DrawableId(4));
        skipped.set_id(DrawableId(4));
        skipped.set_template_name(Some("SkipMe".to_string()));
        let mut skipped_status = skipped.get_status();
        skipped_status.set(DrawableStatus::NO_SAVE);
        skipped.set_status(skipped_status);
        client.drawable_map.insert(DrawableId(4), Box::new(skipped));

        let saveable = client.collect_saveable_drawables_sorted().unwrap();
        let ids: Vec<u32> = saveable.iter().map(|(id, _)| id.0).collect();
        let names: Vec<&str> = saveable.iter().map(|(_, name)| name.as_str()).collect();
        assert_eq!(ids, vec![2, 5, 7]);
        assert_eq!(names, vec!["Two", "Five", "Seven"]);
    }

    #[test]
    fn test_save_uses_object_template_when_drawable_template_missing() {
        let mut client = GameClient::new().unwrap();
        let object_id: ObjectID = 990_001;

        let template: Arc<dyn gamelogic::thing_template::ThingTemplate> = Arc::new(
            LogicDefaultThingTemplate::new("FallbackTemplate".to_string()),
        );
        let object = Arc::new(RwLock::new(GameLogicObject::new_raw(
            template,
            object_id,
            ObjectStatusMaskType::none(),
            None,
        )));
        OBJECT_REGISTRY.register_object(object_id, &object);

        let mut drawable = BasicDrawable::new(DrawableId::INVALID);
        drawable.set_object_id(Some(object_id));
        let drawable_id = client.register_drawable(Box::new(drawable)).unwrap();

        let bytes = serialize_client(&mut client);
        assert!(!bytes.is_empty());
        assert_eq!(
            client
                .find_drawable_by_id(drawable_id)
                .and_then(|d| d.get_template_name()),
            Some("FallbackTemplate")
        );
        assert!(client
            .drawable_toc
            .iter()
            .any(|entry| entry.name == "FallbackTemplate"));

        OBJECT_REGISTRY.unregister_object(object_id);
    }

    #[test]
    fn test_snapshot_round_trip_mixed_no_save_drawables_matches_cpp_rules() {
        ensure_templates_registered(&[
            "FallbackPersistTemplate",
            "SkippedTemplate",
            "PersistedTemplate",
        ]);

        let mut client = GameClient::new().unwrap();
        let object_id: ObjectID = 990_010;

        let template: Arc<dyn gamelogic::thing_template::ThingTemplate> = Arc::new(
            LogicDefaultThingTemplate::new("FallbackPersistTemplate".to_string()),
        );
        let object = Arc::new(RwLock::new(GameLogicObject::new_raw(
            template,
            object_id,
            ObjectStatusMaskType::none(),
            None,
        )));
        OBJECT_REGISTRY.register_object(object_id, &object);

        let mut bound_no_save = BasicDrawable::new(DrawableId::INVALID);
        bound_no_save.set_object_id(Some(object_id));
        let mut bound_status = bound_no_save.get_status();
        bound_status.set(DrawableStatus::NO_SAVE);
        bound_no_save.set_status(bound_status);
        client.register_drawable(Box::new(bound_no_save)).unwrap();

        let mut skipped_no_save = BasicDrawable::new(DrawableId::INVALID);
        skipped_no_save.set_template_name(Some("SkippedTemplate".to_string()));
        let mut skipped_status = skipped_no_save.get_status();
        skipped_status.set(DrawableStatus::NO_SAVE);
        skipped_no_save.set_status(skipped_status);
        client.register_drawable(Box::new(skipped_no_save)).unwrap();

        let mut persisted = BasicDrawable::new(DrawableId::INVALID);
        persisted.set_template_name(Some("PersistedTemplate".to_string()));
        persisted.set_position(Vector3::new(2.0, 3.0, 0.0));
        client.register_drawable(Box::new(persisted)).unwrap();

        let first_save = serialize_client(&mut client);
        let mut loaded = deserialize_client(&first_save);
        let second_save = serialize_client(&mut loaded);

        assert_eq!(first_save, second_save);

        let loaded_bound_id = loaded
            .get_drawable_for_object(object_id)
            .expect("object-bound drawable should persist even with NO_SAVE");
        assert_eq!(
            loaded
                .find_drawable_by_id(loaded_bound_id)
                .and_then(|d| d.get_template_name()),
            Some("FallbackPersistTemplate")
        );

        assert_eq!(loaded.drawable_map.len(), 2);
        assert!(!loaded
            .drawable_map
            .values()
            .any(|drawable| { drawable.get_template_name() == Some("SkippedTemplate") }));

        OBJECT_REGISTRY.unregister_object(object_id);
    }

    #[test]
    fn test_register_drawable_preserves_explicit_template_name_over_object_fallback() {
        let mut client = GameClient::new().unwrap();
        let object_id: ObjectID = 990_002;

        let template: Arc<dyn gamelogic::thing_template::ThingTemplate> = Arc::new(
            LogicDefaultThingTemplate::new("FallbackTemplate".to_string()),
        );
        let object = Arc::new(RwLock::new(GameLogicObject::new_raw(
            template,
            object_id,
            ObjectStatusMaskType::none(),
            None,
        )));
        OBJECT_REGISTRY.register_object(object_id, &object);

        let mut drawable = BasicDrawable::new(DrawableId::INVALID);
        drawable.set_object_id(Some(object_id));
        drawable.set_template_name(Some("ExplicitTemplate".to_string()));
        let drawable_id = client.register_drawable(Box::new(drawable)).unwrap();

        let bytes = serialize_client(&mut client);
        assert!(!bytes.is_empty());
        assert_eq!(
            client
                .find_drawable_by_id(drawable_id)
                .and_then(|d| d.get_template_name()),
            Some("ExplicitTemplate")
        );
        assert!(client
            .drawable_toc
            .iter()
            .any(|entry| entry.name == "ExplicitTemplate"));
        assert!(!client
            .drawable_toc
            .iter()
            .any(|entry| entry.name == "FallbackTemplate"));

        OBJECT_REGISTRY.unregister_object(object_id);
    }

    #[test]
    fn test_drawable_template_equivalence_uses_final_override() {
        let mut factory = CommonThingFactory::new();
        let base_a = factory.new_template("TemplateA");
        let base_b = factory.new_template("TemplateB");
        let shared_final = factory.new_template("SharedFinal");
        base_a.set_next_override(Some(shared_final.clone()));
        base_b.set_next_override(Some(shared_final));

        let mut drawable = BasicDrawable::new(DrawableId::INVALID);
        drawable.set_template_name(Some("TemplateA".to_string()));

        assert!(GameClient::drawable_matches_saved_template(
            &drawable, &base_b, &factory
        ));

        let different = factory.new_template("DifferentFinal");
        assert!(!GameClient::drawable_matches_saved_template(
            &drawable, &different, &factory
        ));
    }

    #[test]
    fn test_message_dispatcher() {
        let dispatcher = GameClientMessageDispatcher::new();
        assert_eq!(dispatcher.message_filters.len(), 0);

        let move_cmd = GameMessage::new(GameMessageType::DoMoveTo(Coord3D::default()));
        assert_eq!(
            dispatcher.translate_game_message(&move_cmd),
            GameMessageDisposition::KeepMessage
        );

        let crc_cmd = GameMessage::new(GameMessageType::LogicCRC(0xABCD1234));
        assert_eq!(
            dispatcher.translate_game_message(&crc_cmd),
            GameMessageDisposition::KeepMessage
        );

        let new_game = GameMessage::new(GameMessageType::NewGame);
        assert_eq!(
            dispatcher.translate_game_message(&new_game),
            GameMessageDisposition::KeepMessage
        );

        let meta_toggle = GameMessage::new(GameMessageType::MetaToggleControlBar);
        assert_eq!(
            dispatcher.translate_game_message(&meta_toggle),
            GameMessageDisposition::DestroyMessage
        );
    }

    #[test]
    fn test_replay_update_culls_local_network_commands_but_keeps_crc() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = "Maps/ReplayCullParity.map".to_string();
            data.pending_file.clear();
        }

        let mut writer = Recorder::new();
        writer
            .start_recording(1, 2, 3, 60)
            .expect("recording should start");
        writer.set_current_frame(5);
        writer
            .write_to_file(&GameMessage::new(GameMessageType::LogicCRC(0x1234ABCD)))
            .expect("recorded replay message should be written");
        writer.stop_recording();

        let replay_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );

        let command_list_arc = get_command_list();
        {
            let mut command_list = command_list_arc
                .write()
                .expect("command list lock should be writable");
            command_list.clear_all_commands();
            command_list.append_message(GameMessage::new(GameMessageType::DoMoveTo(
                Coord3D::default(),
            )));
            command_list.append_message(GameMessage::new(GameMessageType::LogicCRC(0xDEADBEEF)));
            command_list.append_message(GameMessage::new(GameMessageType::NewGame));
        }

        let mut reader = Recorder::new();
        let command_cull: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {
            let command_list_arc = get_command_list();
            if let Ok(mut command_list) = command_list_arc.write() {
                command_list.retain_messages(|msg| {
                    let msg_type = msg.get_type().clone();
                    !(is_network_command_message(msg_type.clone())
                        && !matches!(msg_type, GameMessageType::LogicCRC(_)))
                });
            };
        });
        reader.set_command_cull(Some(command_cull));

        assert!(reader
            .playback_file(replay_name)
            .expect("replay playback should start"));
        reader.set_current_frame(0);
        reader.update();

        let messages = command_list_arc
            .read()
            .expect("command list lock should be readable")
            .snapshot_messages();

        assert!(messages
            .iter()
            .all(|msg| !matches!(msg.get_type(), GameMessageType::DoMoveTo(_))));
        assert!(messages
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::LogicCRC(_))));
        assert!(messages
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::NewGame)));

        reader.stop_playback();
        command_list_arc
            .write()
            .expect("command list lock should be writable")
            .clear_all_commands();
    }

    #[test]
    fn test_recorder_update_records_network_commands_from_source() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = "Maps/ReplayRecordSourceParity.map".to_string();
            data.pending_file.clear();
        }

        let mut writer = Recorder::new();
        writer
            .start_recording(1, 2, 3, 60)
            .expect("recording should start");

        let source_state = std::sync::Arc::new(std::sync::Mutex::new(true));
        let source_state_clone = source_state.clone();
        let command_source: Arc<dyn Fn() -> Vec<GameMessage> + Send + Sync> = Arc::new(move || {
            let mut emit = source_state_clone
                .lock()
                .expect("command source mutex should not be poisoned");
            if !*emit {
                return Vec::new();
            }
            *emit = false;
            vec![
                GameMessage::new(GameMessageType::DoMoveTo(Coord3D {
                    x: 11.0,
                    y: 22.0,
                    z: 0.0,
                })),
                GameMessage::new(GameMessageType::MetaToggleControlBar),
            ]
        });
        writer.set_command_source(Some(command_source));
        writer.set_current_frame(9);
        writer.update();
        writer.stop_recording();

        let replay_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );
        let replay_path = writer.replay_dir().join(&replay_name);
        assert!(replay_path.exists());

        let mut reader = Recorder::new();
        assert!(reader
            .playback_file(replay_name)
            .expect("recorded replay should be playable"));
        reader.set_current_frame(9);
        reader.update();

        let pending = reader.drain_pending_commands();
        assert!(pending.iter().any(|msg| {
            matches!(
                msg.get_type(),
                GameMessageType::DoMoveTo(coord)
                if (coord.x - 11.0).abs() <= f32::EPSILON
                    && (coord.y - 22.0).abs() <= f32::EPSILON
            )
        }));
        assert!(!pending
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::MetaToggleControlBar)));
    }

    #[test]
    fn test_playback_file_clears_stale_pending_commands_when_sink_absent() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = "Maps/ReplayPendingQueueParity.map".to_string();
            data.pending_file.clear();
        }

        let mut writer = Recorder::new();
        writer
            .start_recording(1, 2, 3, 60)
            .expect("recording should start");
        writer.set_current_frame(6);
        writer
            .write_to_file(&GameMessage::new(GameMessageType::DoMoveTo(
                Coord3D::default(),
            )))
            .expect("recorded replay command should be written");
        writer.stop_recording();

        let replay_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );

        let mut reader = Recorder::new();
        assert!(reader
            .playback_file(replay_name.clone())
            .expect("first playback should start"));
        reader.set_current_frame(6);
        reader.update();
        reader.stop_playback();

        assert!(reader
            .playback_file(replay_name)
            .expect("second playback should start"));
        reader.set_current_frame(6);
        reader.update();
        let pending = reader.drain_pending_commands();

        let new_game_count = pending
            .iter()
            .filter(|msg| matches!(msg.get_type(), GameMessageType::NewGame))
            .count();
        let move_count = pending
            .iter()
            .filter(|msg| matches!(msg.get_type(), GameMessageType::DoMoveTo(_)))
            .count();

        assert_eq!(pending.len(), 2);
        assert_eq!(new_game_count, 1);
        assert_eq!(move_count, 1);
    }

    #[test]
    fn test_replay_version_playback_detects_combined_header_mismatches() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = "Maps/ReplayVersionCombined.map".to_string();
            data.pending_file.clear();
            data.exe_crc = 0x0102_0304;
            data.ini_crc = 0x0506_0708;
        }

        let mut writer = Recorder::new();
        writer
            .start_recording(1, 2, 3, 60)
            .expect("recording should start");
        writer.set_current_frame(4);
        writer
            .write_to_file(&GameMessage::new(GameMessageType::LogicCRC(0x0A0B0C0D)))
            .expect("recorded replay message should be written");
        writer.stop_recording();

        let base_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );
        let replays_dir = writer.replay_dir();
        let base_path = replays_dir.join(&base_name);
        assert!(base_path.exists());

        let (
            version_string_start,
            version_string_end,
            _version_time_start,
            version_number_offset,
            exe_crc_offset,
            ini_crc_offset,
        ) = replay_version_offsets(
            &std::fs::read(&base_path).expect("base replay should be readable for offset parsing"),
        );

        // Baseline: exact match must report no mismatch.
        assert!(!Recorder::new()
            .test_version_playback(base_name.clone())
            .expect("baseline replay should be readable"));

        let ext = writer.replay_extension();

        let version_and_exe_crc = format!("combined_version_exe_crc{ext}");
        write_variant(&base_path, &replays_dir, &version_and_exe_crc, |bytes| {
            mutate_utf16_first_code_unit(
                bytes,
                version_string_start,
                version_string_end,
                "version string",
            );

            let current = u32::from_le_bytes(
                bytes[exe_crc_offset..exe_crc_offset + 4]
                    .try_into()
                    .expect("exe CRC slice should be 4 bytes"),
            );
            bytes[exe_crc_offset..exe_crc_offset + 4]
                .copy_from_slice(&current.wrapping_add(1).to_le_bytes());
        });
        assert!(Recorder::new()
            .test_version_playback(version_and_exe_crc)
            .expect("combined mismatch replay should be readable"));

        let version_number_and_ini_crc = format!("combined_version_number_ini_crc{ext}");
        write_variant(
            &base_path,
            &replays_dir,
            &version_number_and_ini_crc,
            |bytes| {
                let version_number = u32::from_le_bytes(
                    bytes[version_number_offset..version_number_offset + 4]
                        .try_into()
                        .expect("version number slice should be 4 bytes"),
                );
                bytes[version_number_offset..version_number_offset + 4]
                    .copy_from_slice(&version_number.wrapping_add(1).to_le_bytes());

                let ini_crc = u32::from_le_bytes(
                    bytes[ini_crc_offset..ini_crc_offset + 4]
                        .try_into()
                        .expect("ini CRC slice should be 4 bytes"),
                );
                bytes[ini_crc_offset..ini_crc_offset + 4]
                    .copy_from_slice(&ini_crc.wrapping_add(1).to_le_bytes());
            },
        );
        assert!(Recorder::new()
            .test_version_playback(version_number_and_ini_crc)
            .expect("combined mismatch replay should be readable"));
    }

    #[test]
    fn test_region_3d_containment() {
        let region = Region3D {
            lo: Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            hi: Coord3D {
                x: 10.0,
                y: 10.0,
                z: 10.0,
            },
        };

        let point_inside = Coord3D {
            x: 5.0,
            y: 5.0,
            z: 5.0,
        };
        let point_outside = Coord3D {
            x: 15.0,
            y: 5.0,
            z: 5.0,
        };

        // Test containment logic
        let inside = point_inside.x >= region.lo.x
            && point_inside.x <= region.hi.x
            && point_inside.y >= region.lo.y
            && point_inside.y <= region.hi.y
            && point_inside.z >= region.lo.z
            && point_inside.z <= region.hi.z;

        let outside = point_outside.x >= region.lo.x
            && point_outside.x <= region.hi.x
            && point_outside.y >= region.lo.y
            && point_outside.y <= region.hi.y
            && point_outside.z >= region.lo.z
            && point_outside.z <= region.hi.z;

        assert!(inside);
        assert!(!outside);
    }
}
