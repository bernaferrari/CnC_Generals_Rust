////////////////////////////////////////////////////////////////////////////////
//
//  (c) 2001-2003 Electronic Arts Inc.
//
////////////////////////////////////////////////////////////////////////////////

// FILE: engine_factory.rs
//
// Factory pattern implementation for game engine creation
// Matches the C++ GameEngineFactory and subsystem factory patterns
//
// Author: Colin Day, April 2001 (Converted to Rust)
//
///////////////////////////////////////////////////////////////////////////////

use anyhow::Result;
use async_trait::async_trait;
use log::info;
use std::any::Any;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::debug_system::DebugConfig;
use crate::game_logic::{self, GameMode};
use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
use crate::subsystem_interfaces::*;

/// Base game engine trait - equivalent to C++ GameEngine interface
#[async_trait]
pub trait GameEngine: Send + Sync {
    /// Initialize the game engine with command line arguments
    async fn init(&mut self, args: &[String]) -> Result<()>;

    /// Execute the main game loop
    async fn execute(&mut self) -> Result<()>;

    /// Shutdown the game engine
    async fn shutdown(&mut self) -> Result<()>;

    /// Get the engine's name
    fn get_name(&self) -> &str;

    /// Check if the engine is initialized
    fn is_initialized(&self) -> bool;

    /// Get the subsystem manager
    fn get_subsystem_manager(&self) -> Option<&SubsystemManager>;

    /// Get the subsystem manager (mutable)
    fn get_subsystem_manager_mut(&mut self) -> Option<&mut SubsystemManager>;
}

/// Game engine factory trait for creating game engines
pub trait GameEngineFactory: Send + Sync {
    /// Create a new game engine instance
    fn create_game_engine(&self) -> Box<dyn GameEngine>;

    /// Get the factory's name
    fn get_name(&self) -> &str;

    /// Check if this factory can create engines for the current platform
    fn is_supported(&self) -> bool;

    /// Get factory priority (higher = preferred)
    fn get_priority(&self) -> i32;
}

/// Default subsystem factory implementation
pub struct DefaultSubsystemFactory {
    _debug_config: DebugConfig,
}

impl DefaultSubsystemFactory {
    pub fn new(debug_config: DebugConfig) -> Self {
        Self {
            _debug_config: debug_config,
        }
    }
}

impl SubsystemFactory for DefaultSubsystemFactory {
    fn create_audio_subsystem(&self) -> Box<dyn AudioSubsystem> {
        Box::new(DefaultAudioSubsystem::new())
    }

    fn create_graphics_subsystem(&self) -> Box<dyn GraphicsSubsystem> {
        Box::new(DefaultGraphicsSubsystem::new())
    }

    fn create_input_subsystem(&self) -> Box<dyn InputSubsystem> {
        Box::new(DefaultInputSubsystem::new())
    }

    fn create_network_subsystem(&self) -> Box<dyn NetworkSubsystem> {
        Box::new(DefaultNetworkSubsystem::new())
    }

    fn create_file_system_subsystem(&self) -> Box<dyn FileSystemSubsystem> {
        Box::new(DefaultFileSystemSubsystem::new())
    }

    fn create_asset_subsystem(&self) -> Box<dyn AssetSubsystem> {
        Box::new(DefaultAssetSubsystem::new())
    }

    fn create_game_logic_subsystem(&self) -> Box<dyn GameLogicSubsystem> {
        Box::new(DefaultGameLogicSubsystem::new())
    }

    fn create_config_subsystem(&self) -> Box<dyn ConfigSubsystem> {
        Box::new(DefaultConfigSubsystem::new())
    }
}

// Default implementations of subsystems (stubs for now)

/// Default audio subsystem implementation
pub struct DefaultAudioSubsystem {
    initialized: bool,
    master_volume: f32,
}

impl DefaultAudioSubsystem {
    pub fn new() -> Self {
        Self {
            initialized: false,
            master_volume: 1.0,
        }
    }
}

#[async_trait]
impl SubsystemInterface for DefaultAudioSubsystem {
    async fn init(&mut self) -> Result<()> {
        info!("Initializing default audio subsystem");
        self.init_audio_hardware().await?;
        self.initialized = true;
        Ok(())
    }

    async fn update(&mut self, _delta_time: f32) -> Result<()> {
        // Audio updates happen in background threads
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down audio subsystem");
        self.stop_all_audio().await?;
        self.initialized = false;
        Ok(())
    }

    fn get_name(&self) -> &str {
        "DefaultAudioSubsystem"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn get_priority(&self) -> i32 {
        10 // Audio has high priority
    }
}

#[async_trait]
impl AudioSubsystem for DefaultAudioSubsystem {
    async fn init_audio_hardware(&mut self) -> Result<()> {
        info!("Audio hardware initialized");
        Ok(())
    }

    async fn play_sound(&mut self, sound_name: &str, volume: f32) -> Result<()> {
        info!("Playing sound: {} at volume {}", sound_name, volume);
        Ok(())
    }

    async fn play_music(&mut self, music_name: &str, loop_music: bool) -> Result<()> {
        info!("Playing music: {} (loop: {})", music_name, loop_music);
        Ok(())
    }

    async fn stop_all_audio(&mut self) -> Result<()> {
        info!("Stopping all audio");
        Ok(())
    }

    fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 1.0);
    }

    fn get_master_volume(&self) -> f32 {
        self.master_volume
    }
}

/// Default graphics subsystem implementation
pub struct DefaultGraphicsSubsystem {
    initialized: bool,
    resolution: (u32, u32),
}

impl DefaultGraphicsSubsystem {
    pub fn new() -> Self {
        Self {
            initialized: false,
            resolution: (1280, 800),
        }
    }
}

#[async_trait]
impl SubsystemInterface for DefaultGraphicsSubsystem {
    async fn init(&mut self) -> Result<()> {
        info!("Initializing default graphics subsystem");
        self.init_graphics_hardware(self.resolution.0, self.resolution.1, false)
            .await?;
        self.initialized = true;
        Ok(())
    }

    async fn update(&mut self, _delta_time: f32) -> Result<()> {
        // Graphics updates happen during rendering
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down graphics subsystem");
        self.initialized = false;
        Ok(())
    }

    fn get_name(&self) -> &str {
        "DefaultGraphicsSubsystem"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn get_priority(&self) -> i32 {
        5 // Graphics has medium-high priority
    }
}

#[async_trait]
impl GraphicsSubsystem for DefaultGraphicsSubsystem {
    async fn init_graphics_hardware(
        &mut self,
        width: u32,
        height: u32,
        fullscreen: bool,
    ) -> Result<()> {
        self.resolution = (width, height);
        info!(
            "Graphics hardware initialized: {}x{} (fullscreen: {})",
            width, height, fullscreen
        );
        Ok(())
    }

    async fn begin_frame(&mut self) -> Result<()> {
        // Begin frame rendering
        Ok(())
    }

    async fn end_frame(&mut self) -> Result<()> {
        // End frame and present
        Ok(())
    }

    fn clear_screen(&mut self, color: [f32; 4]) {
        log::trace!("Clearing screen with color: {:?}", color);
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.resolution = (width, height);
        info!("Graphics resized to: {}x{}", width, height);
    }

    fn get_resolution(&self) -> (u32, u32) {
        self.resolution
    }
}

// Similar stub implementations for other subsystems...

/// Default input subsystem implementation
pub struct DefaultInputSubsystem {
    initialized: bool,
}

impl DefaultInputSubsystem {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

#[async_trait]
impl SubsystemInterface for DefaultInputSubsystem {
    async fn init(&mut self) -> Result<()> {
        info!("Initializing default input subsystem");
        self.init_input_devices().await?;
        self.initialized = true;
        Ok(())
    }

    async fn update(&mut self, _delta_time: f32) -> Result<()> {
        self.update_input().await
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down input subsystem");
        self.initialized = false;
        Ok(())
    }

    fn get_name(&self) -> &str {
        "DefaultInputSubsystem"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn is_initialized(&self) -> bool {
        self.initialized
    }
    fn get_priority(&self) -> i32 {
        15
    } // Input has highest priority
}

#[async_trait]
impl InputSubsystem for DefaultInputSubsystem {
    async fn init_input_devices(&mut self) -> Result<()> {
        info!("Input devices initialized");
        Ok(())
    }

    async fn update_input(&mut self) -> Result<()> {
        // Update input state
        Ok(())
    }

    fn is_key_pressed(&self, _key_code: u32) -> bool {
        false
    }
    fn is_mouse_button_pressed(&self, _button: u32) -> bool {
        false
    }
    fn get_mouse_position(&self) -> (i32, i32) {
        (0, 0)
    }
    fn get_mouse_delta(&self) -> (i32, i32) {
        (0, 0)
    }
}

// Keep a minimal network subsystem in the main factory while multiplayer remains deferred.
macro_rules! impl_stub_subsystem {
    ($name:ident, $trait:ident, $priority:expr) => {
        pub struct $name {
            initialized: bool,
        }

        impl $name {
            pub fn new() -> Self {
                Self { initialized: false }
            }
        }

        #[async_trait]
        impl SubsystemInterface for $name {
            async fn init(&mut self) -> Result<()> {
                info!("Initializing {}", stringify!($name));
                self.initialized = true;
                Ok(())
            }

            async fn update(&mut self, _delta_time: f32) -> Result<()> {
                Ok(())
            }

            async fn shutdown(&mut self) -> Result<()> {
                info!("Shutting down {}", stringify!($name));
                self.initialized = false;
                Ok(())
            }

            fn get_name(&self) -> &str {
                stringify!($name)
            }
            fn as_any(&self) -> &dyn Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn Any {
                self
            }
            fn is_initialized(&self) -> bool {
                self.initialized
            }
            fn get_priority(&self) -> i32 {
                $priority
            }
        }
    };
}

impl_stub_subsystem!(DefaultNetworkSubsystem, NetworkSubsystem, 0);

#[async_trait]
impl NetworkSubsystem for DefaultNetworkSubsystem {
    async fn init_network(&mut self) -> Result<()> {
        Ok(())
    }
    async fn start_server(&mut self, _port: u16) -> Result<()> {
        Ok(())
    }
    async fn connect_to_server(&mut self, _host: &str, _port: u16) -> Result<()> {
        Ok(())
    }
    async fn broadcast_data(&mut self, _data: &[u8]) -> Result<()> {
        Ok(())
    }
    async fn send_to_server(&mut self, _data: &[u8]) -> Result<()> {
        Ok(())
    }
    async fn receive_data(&mut self) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }
    async fn disconnect(&mut self) -> Result<()> {
        Ok(())
    }
    fn is_connected(&self) -> bool {
        false
    }
}

pub struct DefaultFileSystemSubsystem {
    initialized: bool,
    root_dir: PathBuf,
}

impl DefaultFileSystemSubsystem {
    pub fn new() -> Self {
        Self {
            initialized: false,
            root_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let candidate = Path::new(path);
        if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            self.root_dir.join(candidate)
        }
    }
}

#[async_trait]
impl SubsystemInterface for DefaultFileSystemSubsystem {
    async fn init(&mut self) -> Result<()> {
        info!(
            "Initializing DefaultFileSystemSubsystem at {:?}",
            self.root_dir
        );
        self.initialized = true;
        self.init_file_system().await
    }

    async fn update(&mut self, _delta_time: f32) -> Result<()> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down DefaultFileSystemSubsystem");
        self.initialized = false;
        Ok(())
    }

    fn get_name(&self) -> &str {
        "DefaultFileSystemSubsystem"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn get_priority(&self) -> i32 {
        20
    }
}

#[async_trait]
impl FileSystemSubsystem for DefaultFileSystemSubsystem {
    async fn init_file_system(&mut self) -> Result<()> {
        tokio::fs::create_dir_all(&self.root_dir).await?;
        Ok(())
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        let resolved = self.resolve_path(path);
        Ok(tokio::fs::read(resolved).await?)
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<()> {
        let resolved = self.resolve_path(path);
        if let Some(parent) = resolved.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(resolved, data).await?;
        Ok(())
    }

    fn file_exists(&self, path: &str) -> bool {
        self.resolve_path(path).exists()
    }

    fn list_directory(&self, path: &str) -> Result<Vec<String>> {
        let resolved = self.resolve_path(path);
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(resolved)? {
            let entry = entry?;
            entries.push(entry.file_name().to_string_lossy().to_string());
        }
        Ok(entries)
    }

    fn get_file_size(&self, path: &str) -> Result<u64> {
        Ok(std::fs::metadata(self.resolve_path(path))?.len())
    }
}

#[derive(Debug)]
struct LoadedTextureAsset {
    path: String,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct LoadedModelAsset {
    path: String,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct LoadedAudioAsset {
    path: String,
    bytes: Vec<u8>,
}

pub struct DefaultAssetSubsystem {
    initialized: bool,
    next_asset_id: u32,
    assets: HashMap<u32, Box<dyn Any + Send + Sync>>,
}

impl DefaultAssetSubsystem {
    pub fn new() -> Self {
        Self {
            initialized: false,
            next_asset_id: 1,
            assets: HashMap::new(),
        }
    }

    async fn read_asset_bytes(path: &str) -> Result<Vec<u8>> {
        Ok(tokio::fs::read(path).await?)
    }

    fn insert_asset(&mut self, asset: Box<dyn Any + Send + Sync>) -> u32 {
        let id = self.next_asset_id;
        self.next_asset_id = self.next_asset_id.saturating_add(1);
        self.assets.insert(id, asset);
        id
    }
}

#[async_trait]
impl SubsystemInterface for DefaultAssetSubsystem {
    async fn init(&mut self) -> Result<()> {
        info!("Initializing DefaultAssetSubsystem");
        self.initialized = true;
        self.init_assets().await
    }

    async fn update(&mut self, _delta_time: f32) -> Result<()> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down DefaultAssetSubsystem");
        self.initialized = false;
        self.assets.clear();
        Ok(())
    }

    fn get_name(&self) -> &str {
        "DefaultAssetSubsystem"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn get_priority(&self) -> i32 {
        8
    }
}

#[async_trait]
impl AssetSubsystem for DefaultAssetSubsystem {
    async fn init_assets(&mut self) -> Result<()> {
        Ok(())
    }

    async fn load_texture(&mut self, path: &str) -> Result<u32> {
        let bytes = Self::read_asset_bytes(path).await?;
        Ok(self.insert_asset(Box::new(LoadedTextureAsset {
            path: path.to_string(),
            bytes,
        })))
    }

    async fn load_model(&mut self, path: &str) -> Result<u32> {
        let bytes = Self::read_asset_bytes(path).await?;
        Ok(self.insert_asset(Box::new(LoadedModelAsset {
            path: path.to_string(),
            bytes,
        })))
    }

    async fn load_audio(&mut self, path: &str) -> Result<u32> {
        let bytes = Self::read_asset_bytes(path).await?;
        Ok(self.insert_asset(Box::new(LoadedAudioAsset {
            path: path.to_string(),
            bytes,
        })))
    }

    async fn unload_asset(&mut self, asset_id: u32) -> Result<()> {
        self.assets.remove(&asset_id);
        Ok(())
    }

    fn get_asset(&self, asset_id: u32) -> Option<&dyn Any> {
        self.assets
            .get(&asset_id)
            .map(|asset| asset.as_ref() as &dyn Any)
    }

    async fn preload_assets(&mut self, asset_list: &[&str]) -> Result<()> {
        for path in asset_list {
            let lower = path.to_ascii_lowercase();
            if lower.ends_with(".wav") || lower.ends_with(".ogg") || lower.ends_with(".mp3") {
                let _ = self.load_audio(path).await;
            } else if lower.ends_with(".w3d") || lower.ends_with(".gltf") || lower.ends_with(".obj")
            {
                let _ = self.load_model(path).await;
            } else {
                let _ = self.load_texture(path).await;
            }
        }
        Ok(())
    }
}

pub struct DefaultGameLogicSubsystem {
    initialized: bool,
    game_logic: game_logic::GameLogic,
    save_manager: std::sync::Mutex<SaveFileManager>,
}

impl DefaultGameLogicSubsystem {
    pub fn new() -> Self {
        Self {
            initialized: false,
            game_logic: game_logic::GameLogic::new(),
            save_manager: std::sync::Mutex::new(SaveFileManager::new()),
        }
    }
}

#[async_trait]
impl SubsystemInterface for DefaultGameLogicSubsystem {
    async fn init(&mut self) -> Result<()> {
        info!("Initializing DefaultGameLogicSubsystem");
        self.initialized = true;
        self.init_game_logic().await
    }

    async fn update(&mut self, delta_time: f32) -> Result<()> {
        self.update_simulation(delta_time).await?;
        self.process_commands().await
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down DefaultGameLogicSubsystem");
        self.initialized = false;
        Ok(())
    }

    fn get_name(&self) -> &str {
        "DefaultGameLogicSubsystem"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn get_priority(&self) -> i32 {
        -5
    }
}

#[async_trait]
impl GameLogicSubsystem for DefaultGameLogicSubsystem {
    async fn init_game_logic(&mut self) -> Result<()> {
        self.game_logic.start_new_game(GameMode::Skirmish);
        self.save_manager
            .lock()
            .expect("save manager mutex poisoned")
            .init()?;
        Ok(())
    }

    async fn update_simulation(&mut self, delta_time: f32) -> Result<()> {
        self.game_logic.update_with_dt(delta_time);
        Ok(())
    }

    async fn process_commands(&mut self) -> Result<()> {
        self.game_logic.process_commands();
        Ok(())
    }

    async fn handle_input(&mut self, input_events: &[InputEvent]) -> Result<()> {
        for input in input_events {
            if let InputEvent::KeyPressed { key_code } = input {
                // C++ parity: Escape toggles pause state in high-level front-end loops.
                if *key_code == 27 {
                    self.game_logic.set_paused(!self.game_logic.is_paused());
                }
            }
        }
        Ok(())
    }

    async fn save_game(&self, slot: u32) -> Result<()> {
        let filename = format!("slot_{slot}");
        let save_info = SaveGameInfo {
            filename: filename.clone(),
            display_name: format!("Slot {slot}"),
            description: format!("Subsystem save slot {slot}"),
            map_name: {
                let name = self.game_logic.get_current_map_name();
                if name.is_empty() {
                    "Unknown".to_string()
                } else {
                    name.to_string()
                }
            },
            campaign_side: None,
            mission_number: None,
            save_date: SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            play_time: Duration::from_secs_f32(self.game_logic.get_total_play_time()),
            difficulty: match self.game_logic.get_difficulty() {
                crate::ai::AIDifficulty::Easy => GameDifficulty::Easy,
                crate::ai::AIDifficulty::Medium => GameDifficulty::Medium,
                crate::ai::AIDifficulty::Hard => GameDifficulty::Hard,
                crate::ai::AIDifficulty::Brutal => GameDifficulty::Hard,
            },
            save_type: SaveFileType::Normal,
        };

        self.save_manager
            .lock()
            .expect("save manager mutex poisoned")
            .save_game(&filename, &self.game_logic, &save_info)?;
        Ok(())
    }

    async fn load_game(&mut self, slot: u32) -> Result<()> {
        let filename = format!("slot_{slot}");
        self.save_manager
            .lock()
            .expect("save manager mutex poisoned")
            .load_game(&filename, &mut self.game_logic)?;
        Ok(())
    }
}

pub struct DefaultConfigSubsystem {
    initialized: bool,
    values: HashMap<String, String>,
}

impl DefaultConfigSubsystem {
    pub fn new() -> Self {
        Self {
            initialized: false,
            values: HashMap::new(),
        }
    }

    fn flatten_toml(prefix: &str, value: &toml::Value, out: &mut HashMap<String, String>) {
        match value {
            toml::Value::Table(table) => {
                for (key, nested) in table {
                    let full_key = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    Self::flatten_toml(&full_key, nested, out);
                }
            }
            _ => {
                out.insert(prefix.to_string(), value.to_string());
            }
        }
    }
}

#[async_trait]
impl SubsystemInterface for DefaultConfigSubsystem {
    async fn init(&mut self) -> Result<()> {
        info!("Initializing DefaultConfigSubsystem");
        self.initialized = true;
        self.init_config().await
    }

    async fn update(&mut self, _delta_time: f32) -> Result<()> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down DefaultConfigSubsystem");
        self.initialized = false;
        Ok(())
    }

    fn get_name(&self) -> &str {
        "DefaultConfigSubsystem"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn get_priority(&self) -> i32 {
        25
    }
}

#[async_trait]
impl ConfigSubsystem for DefaultConfigSubsystem {
    async fn init_config(&mut self) -> Result<()> {
        Ok(())
    }

    async fn load_config(&mut self, path: &str) -> Result<()> {
        let text = tokio::fs::read_to_string(path).await?;
        let value: toml::Value = text.parse()?;
        self.values.clear();
        Self::flatten_toml("", &value, &mut self.values);
        Ok(())
    }

    async fn save_config(&self, path: &str) -> Result<()> {
        if let Some(parent) = Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut keys: Vec<&String> = self.values.keys().collect();
        keys.sort();

        let mut out = String::new();
        for key in keys {
            if let Some(value) = self.values.get(key) {
                out.push_str(key);
                out.push_str(" = ");
                out.push_str(value);
                out.push('\n');
            }
        }

        tokio::fs::write(path, out).await?;
        Ok(())
    }

    fn get_string_value(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }

    fn set_string_value(&mut self, key: &str, value: String) {
        self.values.insert(key.to_string(), value);
    }

    fn get_keys(&self) -> Vec<String> {
        self.values.keys().cloned().collect()
    }
}

/// Factory registry for managing multiple engine factories
pub struct EngineFactoryRegistry {
    factories: Vec<Box<dyn GameEngineFactory>>,
}

impl EngineFactoryRegistry {
    /// Create a new factory registry
    pub fn new() -> Self {
        Self {
            factories: Vec::new(),
        }
    }

    /// Register a factory
    pub fn register_factory(&mut self, factory: Box<dyn GameEngineFactory>) {
        info!("Registering engine factory: {}", factory.get_name());
        self.factories.push(factory);
    }

    /// Get the best available factory for the current platform
    pub fn get_best_factory(&self) -> Option<&dyn GameEngineFactory> {
        self.factories
            .iter()
            .filter(|f| f.is_supported())
            .max_by_key(|f| f.get_priority())
            .map(|f| f.as_ref())
    }

    /// Create a game engine using the best available factory
    pub fn create_game_engine(&self) -> Result<Box<dyn GameEngine>> {
        let factory = self
            .get_best_factory()
            .ok_or_else(|| anyhow::anyhow!("No suitable engine factory found for this platform"))?;

        info!("Creating game engine using factory: {}", factory.get_name());
        Ok(factory.create_game_engine())
    }
}

impl Default for EngineFactoryRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the engine factory system
pub fn initialize_engine_factories() -> EngineFactoryRegistry {
    let mut registry = EngineFactoryRegistry::new();

    // Register platform-specific factories
    #[cfg(target_os = "windows")]
    {
        registry.register_factory(Box::new(
            crate::win32_game_engine::Win32GameEngineFactory::new(),
        ));
    }

    #[cfg(target_os = "macos")]
    {
        // On macOS, we'll use a cross-platform factory
        registry.register_factory(Box::new(CrossPlatformGameEngineFactory::new()));
    }

    #[cfg(target_os = "linux")]
    {
        // On Linux, we'll use a cross-platform factory
        registry.register_factory(Box::new(CrossPlatformGameEngineFactory::new()));
    }

    // Always register cross-platform factory as fallback
    registry.register_factory(Box::new(CrossPlatformGameEngineFactory::new()));

    info!(
        "Engine factory system initialized with {} factories",
        registry.factories.len()
    );
    registry
}

/// Cross-platform game engine factory
pub struct CrossPlatformGameEngineFactory;

impl CrossPlatformGameEngineFactory {
    pub fn new() -> Self {
        Self
    }
}

impl GameEngineFactory for CrossPlatformGameEngineFactory {
    fn create_game_engine(&self) -> Box<dyn GameEngine> {
        Box::new(crate::game_engine::CrossPlatformGameEngine::new())
    }

    fn get_name(&self) -> &str {
        "CrossPlatformGameEngineFactory"
    }

    fn is_supported(&self) -> bool {
        true // Always supported
    }

    fn get_priority(&self) -> i32 {
        1 // Low priority, used as fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subsystem_factory() {
        let factory = DefaultSubsystemFactory::new(DebugConfig::default());
        let mut audio = factory.create_audio_subsystem();

        assert_eq!(audio.get_name(), "DefaultAudioSubsystem");
        assert!(!audio.is_initialized());

        audio.init().await.unwrap();
        assert!(audio.is_initialized());

        audio.shutdown().await.unwrap();
        assert!(!audio.is_initialized());
    }

    #[test]
    fn test_factory_registry() {
        let mut registry = EngineFactoryRegistry::new();
        registry.register_factory(Box::new(CrossPlatformGameEngineFactory::new()));

        let factory = registry.get_best_factory().unwrap();
        assert_eq!(factory.get_name(), "CrossPlatformGameEngineFactory");
        assert!(factory.is_supported());
    }

    #[test]
    fn test_engine_creation() {
        let registry = initialize_engine_factories();
        let _engine = registry.create_game_engine().unwrap();
    }
}
