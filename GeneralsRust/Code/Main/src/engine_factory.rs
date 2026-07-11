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

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use log::info;
use std::any::Any;
use std::collections::{BTreeMap, HashMap, VecDeque};
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
    debug_config: DebugConfig,
}

impl DefaultSubsystemFactory {
    pub fn new(debug_config: DebugConfig) -> Self {
        Self { debug_config }
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
        Box::new(DefaultConfigSubsystem::new(self.debug_config.clone()))
    }
}

// Default implementations of subsystems (stubs for now)

/// Default audio subsystem implementation
pub struct DefaultAudioSubsystem {
    initialized: bool,
    hardware_ready: bool,
    master_volume: f32,
    current_music: Option<String>,
    active_sounds: Vec<(String, f32)>,
}

impl Default for DefaultAudioSubsystem {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultAudioSubsystem {
    pub fn new() -> Self {
        Self {
            initialized: false,
            hardware_ready: false,
            master_volume: 1.0,
            current_music: None,
            active_sounds: Vec::new(),
        }
    }

    pub fn is_hardware_ready(&self) -> bool {
        self.hardware_ready
    }

    pub fn active_sound_count(&self) -> usize {
        self.active_sounds.len()
    }

    pub fn current_music_name(&self) -> Option<&str> {
        self.current_music.as_deref()
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
        self.hardware_ready = false;
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
        self.hardware_ready = true;
        Ok(())
    }

    async fn play_sound(&mut self, sound_name: &str, volume: f32) -> Result<()> {
        if !self.initialized || !self.hardware_ready {
            return Err(anyhow!("Audio subsystem is not initialized"));
        }
        info!("Playing sound: {} at volume {}", sound_name, volume);
        self.active_sounds
            .push((sound_name.to_string(), volume.clamp(0.0, 1.0)));
        Ok(())
    }

    async fn play_music(&mut self, music_name: &str, loop_music: bool) -> Result<()> {
        if !self.initialized || !self.hardware_ready {
            return Err(anyhow!("Audio subsystem is not initialized"));
        }
        info!("Playing music: {} (loop: {})", music_name, loop_music);
        self.current_music = Some(music_name.to_string());
        Ok(())
    }

    async fn stop_all_audio(&mut self) -> Result<()> {
        info!("Stopping all audio");
        self.current_music = None;
        self.active_sounds.clear();
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
    fullscreen: bool,
    frame_active: bool,
    last_clear_color: [f32; 4],
}

impl Default for DefaultGraphicsSubsystem {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultGraphicsSubsystem {
    pub fn new() -> Self {
        Self {
            initialized: false,
            resolution: (1280, 800),
            fullscreen: false,
            frame_active: false,
            last_clear_color: [0.0, 0.0, 0.0, 1.0],
        }
    }

    pub fn is_frame_active(&self) -> bool {
        self.frame_active
    }

    pub fn is_fullscreen(&self) -> bool {
        self.fullscreen
    }

    pub fn last_clear_color(&self) -> [f32; 4] {
        self.last_clear_color
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
        self.frame_active = false;
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
        self.fullscreen = fullscreen;
        info!(
            "Graphics hardware initialized: {}x{} (fullscreen: {})",
            width, height, fullscreen
        );
        Ok(())
    }

    async fn begin_frame(&mut self) -> Result<()> {
        self.frame_active = true;
        Ok(())
    }

    async fn end_frame(&mut self) -> Result<()> {
        self.frame_active = false;
        Ok(())
    }

    fn clear_screen(&mut self, color: [f32; 4]) {
        self.last_clear_color = color;
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
    key_states: std::collections::HashSet<u32>,
    mouse_button_states: std::collections::HashSet<u32>,
    mouse_position: (i32, i32),
    mouse_delta: (i32, i32),
}

impl Default for DefaultInputSubsystem {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultInputSubsystem {
    pub fn new() -> Self {
        Self {
            initialized: false,
            key_states: std::collections::HashSet::new(),
            mouse_button_states: std::collections::HashSet::new(),
            mouse_position: (0, 0),
            mouse_delta: (0, 0),
        }
    }

    pub fn set_key_state(&mut self, key_code: u32, pressed: bool) {
        if pressed {
            self.key_states.insert(key_code);
        } else {
            self.key_states.remove(&key_code);
        }
    }

    pub fn set_mouse_button_state(&mut self, button: u32, pressed: bool) {
        if pressed {
            self.mouse_button_states.insert(button);
        } else {
            self.mouse_button_states.remove(&button);
        }
    }

    pub fn set_mouse_position(&mut self, x: i32, y: i32) {
        self.mouse_delta = (x - self.mouse_position.0, y - self.mouse_position.1);
        self.mouse_position = (x, y);
    }

    pub fn set_mouse_delta(&mut self, dx: i32, dy: i32) {
        self.mouse_delta = (dx, dy);
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
        self.key_states.clear();
        self.mouse_button_states.clear();
        self.mouse_position = (0, 0);
        self.mouse_delta = (0, 0);
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
        Ok(())
    }

    fn is_key_pressed(&self, _key_code: u32) -> bool {
        self.key_states.contains(&_key_code)
    }
    fn is_mouse_button_pressed(&self, _button: u32) -> bool {
        self.mouse_button_states.contains(&_button)
    }
    fn get_mouse_position(&self) -> (i32, i32) {
        self.mouse_position
    }
    fn get_mouse_delta(&self) -> (i32, i32) {
        self.mouse_delta
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DefaultNetworkMode {
    Disconnected,
    Server { port: u16 },
    Client { host: String, port: u16 },
}

/// Network subsystem placeholder with local state tracking only.
///
/// Multiplayer protocol behavior is still deferred, but this object now
/// remembers mode transitions, validates lifecycle calls, and buffers data so
/// the factory path behaves like a real subsystem rather than a no-op stub.
pub struct DefaultNetworkSubsystem {
    initialized: bool,
    mode: DefaultNetworkMode,
    inbound_packets: VecDeque<Vec<u8>>,
    outbound_packets: VecDeque<Vec<u8>>,
}

impl DefaultNetworkSubsystem {
    pub fn new() -> Self {
        Self {
            initialized: false,
            mode: DefaultNetworkMode::Disconnected,
            inbound_packets: VecDeque::new(),
            outbound_packets: VecDeque::new(),
        }
    }

    fn mode(&self) -> &DefaultNetworkMode {
        &self.mode
    }

    pub fn pending_inbound_packets(&self) -> usize {
        self.inbound_packets.len()
    }

    pub fn pending_outbound_packets(&self) -> usize {
        self.outbound_packets.len()
    }

    pub fn connected_endpoint(&self) -> Option<String> {
        match &self.mode {
            DefaultNetworkMode::Server { port } => Some(format!("server:{port}")),
            DefaultNetworkMode::Client { host, port } => Some(format!("{host}:{port}")),
            DefaultNetworkMode::Disconnected => None,
        }
    }

    fn ensure_initialized(&self) -> Result<()> {
        if self.initialized {
            Ok(())
        } else {
            Err(anyhow!("Network subsystem is not initialized"))
        }
    }
}

impl Default for DefaultNetworkSubsystem {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SubsystemInterface for DefaultNetworkSubsystem {
    async fn init(&mut self) -> Result<()> {
        info!("Initializing DefaultNetworkSubsystem");
        self.init_network().await?;
        self.initialized = true;
        Ok(())
    }

    async fn update(&mut self, _delta_time: f32) -> Result<()> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down DefaultNetworkSubsystem");
        self.disconnect().await?;
        self.initialized = false;
        Ok(())
    }

    fn get_name(&self) -> &str {
        "DefaultNetworkSubsystem"
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
        0
    }
}

#[async_trait]
impl NetworkSubsystem for DefaultNetworkSubsystem {
    async fn init_network(&mut self) -> Result<()> {
        self.mode = DefaultNetworkMode::Disconnected;
        self.inbound_packets.clear();
        self.outbound_packets.clear();
        Ok(())
    }
    async fn start_server(&mut self, _port: u16) -> Result<()> {
        self.ensure_initialized()?;
        self.mode = DefaultNetworkMode::Server { port: _port };
        self.inbound_packets.clear();
        self.outbound_packets.clear();
        Ok(())
    }
    async fn connect_to_server(&mut self, _host: &str, _port: u16) -> Result<()> {
        self.ensure_initialized()?;
        self.mode = DefaultNetworkMode::Client {
            host: _host.to_string(),
            port: _port,
        };
        self.inbound_packets.clear();
        self.outbound_packets.clear();
        Ok(())
    }
    async fn broadcast_data(&mut self, _data: &[u8]) -> Result<()> {
        self.ensure_initialized()?;
        match self.mode {
            DefaultNetworkMode::Server { .. } => {
                self.outbound_packets.push_back(_data.to_vec());
                self.inbound_packets.push_back(_data.to_vec());
                Ok(())
            }
            _ => Err(anyhow!("broadcast_data requires server mode")),
        }
    }
    async fn send_to_server(&mut self, _data: &[u8]) -> Result<()> {
        self.ensure_initialized()?;
        match self.mode {
            DefaultNetworkMode::Client { .. } => {
                self.outbound_packets.push_back(_data.to_vec());
                self.inbound_packets.push_back(_data.to_vec());
                Ok(())
            }
            _ => Err(anyhow!("send_to_server requires client mode")),
        }
    }
    async fn receive_data(&mut self) -> Result<Vec<u8>> {
        self.ensure_initialized()?;
        Ok(self.inbound_packets.pop_front().unwrap_or_default())
    }
    async fn disconnect(&mut self) -> Result<()> {
        self.mode = DefaultNetworkMode::Disconnected;
        self.inbound_packets.clear();
        self.outbound_packets.clear();
        Ok(())
    }
    fn is_connected(&self) -> bool {
        self.initialized && !matches!(self.mode, DefaultNetworkMode::Disconnected)
    }
}

pub struct DefaultFileSystemSubsystem {
    initialized: bool,
    root_dir: PathBuf,
}

impl Default for DefaultFileSystemSubsystem {
    fn default() -> Self {
        Self::new()
    }
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
        self.init_file_system().await?;
        self.initialized = true;
        Ok(())
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
    search_paths: Vec<PathBuf>,
    loaded_asset_paths: Vec<String>,
}

impl Default for DefaultAssetSubsystem {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultAssetSubsystem {
    pub fn new() -> Self {
        Self {
            initialized: false,
            next_asset_id: 1,
            assets: HashMap::new(),
            search_paths: Vec::new(),
            loaded_asset_paths: Vec::new(),
        }
    }

    pub fn set_search_paths<I>(&mut self, paths: I)
    where
        I: IntoIterator<Item = PathBuf>,
    {
        self.search_paths.clear();
        for path in paths {
            self.register_search_path(path);
        }
    }

    pub fn register_search_path<P>(&mut self, path: P)
    where
        P: Into<PathBuf>,
    {
        let path = path.into();
        if !self.search_paths.iter().any(|existing| existing == &path) {
            self.search_paths.push(path);
        }
    }

    pub fn search_paths(&self) -> &[PathBuf] {
        &self.search_paths
    }

    pub fn loaded_asset_paths(&self) -> &[String] {
        &self.loaded_asset_paths
    }

    fn ensure_default_search_paths(&mut self) {
        let mut defaults = vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))];
        defaults.extend([
            PathBuf::from("Data"),
            PathBuf::from("Data/INI/Default"),
            PathBuf::from("Data/INI"),
            PathBuf::from("Data/Audio"),
            PathBuf::from("Data/Art"),
        ]);

        for path in defaults {
            self.register_search_path(path);
        }
    }

    fn resolve_asset_path(&self, path: &str) -> PathBuf {
        let candidate = Path::new(path);
        if candidate.is_absolute() {
            return candidate.to_path_buf();
        }

        if candidate.exists() {
            return candidate.to_path_buf();
        }

        for root in &self.search_paths {
            let resolved = root.join(candidate);
            if resolved.exists() {
                return resolved;
            }
        }

        self.search_paths
            .first()
            .cloned()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
            .join(candidate)
    }

    async fn read_asset_bytes(&self, path: &str) -> Result<(PathBuf, Vec<u8>)> {
        let resolved = self.resolve_asset_path(path);
        Ok((resolved.clone(), tokio::fs::read(resolved).await?))
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
        self.init_assets().await?;
        self.initialized = true;
        Ok(())
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
        self.ensure_default_search_paths();
        Ok(())
    }

    async fn load_texture(&mut self, path: &str) -> Result<u32> {
        let (resolved_path, bytes) = self.read_asset_bytes(path).await?;
        self.loaded_asset_paths
            .push(resolved_path.to_string_lossy().to_string());
        Ok(self.insert_asset(Box::new(LoadedTextureAsset {
            path: resolved_path.to_string_lossy().to_string(),
            bytes,
        })))
    }

    async fn load_model(&mut self, path: &str) -> Result<u32> {
        let (resolved_path, bytes) = self.read_asset_bytes(path).await?;
        self.loaded_asset_paths
            .push(resolved_path.to_string_lossy().to_string());
        Ok(self.insert_asset(Box::new(LoadedModelAsset {
            path: resolved_path.to_string_lossy().to_string(),
            bytes,
        })))
    }

    async fn load_audio(&mut self, path: &str) -> Result<u32> {
        let (resolved_path, bytes) = self.read_asset_bytes(path).await?;
        self.loaded_asset_paths
            .push(resolved_path.to_string_lossy().to_string());
        Ok(self.insert_asset(Box::new(LoadedAudioAsset {
            path: resolved_path.to_string_lossy().to_string(),
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
                self.load_audio(path).await?;
            } else if lower.ends_with(".w3d") || lower.ends_with(".gltf") || lower.ends_with(".obj")
            {
                self.load_model(path).await?;
            } else {
                self.load_texture(path).await?;
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

impl Default for DefaultGameLogicSubsystem {
    fn default() -> Self {
        Self::new()
    }
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
        self.init_game_logic().await?;
        self.initialized = true;
        Ok(())
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
        self.game_logic.reset();
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
    debug_config: DebugConfig,
    values: HashMap<String, String>,
    bootstrap_paths: Vec<PathBuf>,
    loaded_bootstrap_paths: Vec<PathBuf>,
}

impl DefaultConfigSubsystem {
    pub fn new(debug_config: DebugConfig) -> Self {
        Self {
            initialized: false,
            debug_config,
            values: HashMap::new(),
            bootstrap_paths: Self::default_bootstrap_paths(),
            loaded_bootstrap_paths: Vec::new(),
        }
    }

    pub fn set_bootstrap_paths<I>(&mut self, paths: I)
    where
        I: IntoIterator<Item = PathBuf>,
    {
        self.bootstrap_paths = paths.into_iter().collect();
    }

    pub fn bootstrap_paths(&self) -> &[PathBuf] {
        &self.bootstrap_paths
    }

    pub fn loaded_bootstrap_paths(&self) -> &[PathBuf] {
        &self.loaded_bootstrap_paths
    }

    fn default_bootstrap_paths() -> Vec<PathBuf> {
        vec![
            PathBuf::from("Data/INI/Default/GameData.ini"),
            PathBuf::from("Data/INI/GameData.ini"),
        ]
    }

    fn debug_bootstrap_path(&self) -> Option<PathBuf> {
        if self.debug_config.debug_ui_enabled || self.debug_config.log_level == "debug" {
            Some(PathBuf::from("Data/INI/GameDataDebug.ini"))
        } else {
            None
        }
    }

    fn full_key(section: Option<&str>, key: &str) -> String {
        match section {
            Some(section) if !section.is_empty() => format!("{section}.{key}"),
            _ => key.to_string(),
        }
    }

    fn split_key(key: &str) -> (Option<&str>, &str) {
        if let Some((section, entry)) = key.split_once('.') {
            (Some(section), entry)
        } else {
            (None, key)
        }
    }

    fn parse_ini_text(&mut self, text: &str) -> Result<()> {
        let mut section: Option<String> = None;

        for (line_idx, raw_line) in text.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                let name = line[1..line.len() - 1].trim();
                section = if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                };
                continue;
            }

            let (key, value) = line.split_once('=').ok_or_else(|| {
                anyhow!(
                    "Invalid config entry on line {}: {}",
                    line_idx + 1,
                    raw_line
                )
            })?;
            let key = key.trim();
            if key.is_empty() {
                return Err(anyhow!(
                    "Invalid config entry on line {}: empty key",
                    line_idx + 1
                ));
            }

            let value = value.trim().trim_matches('"').trim_matches('\'');
            let full_key = Self::full_key(section.as_deref(), key);
            self.values.insert(full_key, value.to_string());
        }

        Ok(())
    }

    fn serialize_ini_value(value: &str) -> String {
        let needs_quotes = value.is_empty()
            || value
                .chars()
                .any(|ch| ch.is_whitespace() || matches!(ch, ';' | '#' | '[' | ']' | '=' | '"'));

        if needs_quotes {
            format!("\"{}\"", value.replace('"', "\\\""))
        } else {
            value.to_string()
        }
    }

    async fn load_bootstrap_path(&mut self, path: &Path) -> Result<bool> {
        match tokio::fs::read_to_string(path).await {
            Ok(text) => {
                self.parse_ini_text(&text)?;
                self.loaded_bootstrap_paths.push(path.to_path_buf());
                Ok(true)
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err.into()),
        }
    }

    fn group_values(&self) -> BTreeMap<Option<String>, BTreeMap<String, String>> {
        let mut grouped: BTreeMap<Option<String>, BTreeMap<String, String>> = BTreeMap::new();

        for (key, value) in &self.values {
            let (section, entry) = Self::split_key(key);
            grouped
                .entry(section.map(|s| s.to_string()))
                .or_default()
                .insert(entry.to_string(), value.clone());
        }

        grouped
    }
}

#[async_trait]
impl SubsystemInterface for DefaultConfigSubsystem {
    async fn init(&mut self) -> Result<()> {
        info!("Initializing DefaultConfigSubsystem");
        self.init_config().await?;
        self.initialized = true;
        Ok(())
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
        self.values.clear();
        self.loaded_bootstrap_paths.clear();

        for path in self.bootstrap_paths.clone() {
            self.load_bootstrap_path(&path).await?;
        }

        if let Some(debug_path) = self.debug_bootstrap_path() {
            self.load_bootstrap_path(&debug_path).await?;
        }

        Ok(())
    }

    async fn load_config(&mut self, path: &str) -> Result<()> {
        self.values.clear();
        self.loaded_bootstrap_paths.clear();

        let text = tokio::fs::read_to_string(path).await?;
        self.parse_ini_text(&text)?;
        Ok(())
    }

    async fn save_config(&self, path: &str) -> Result<()> {
        if let Some(parent) = Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut out = String::new();
        for (section, entries) in self.group_values() {
            if let Some(section) = section {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push('[');
                out.push_str(&section);
                out.push_str("]\n");
            }

            for (key, value) in entries {
                out.push_str(&key);
                out.push_str(" = ");
                out.push_str(&Self::serialize_ini_value(&value));
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

    fn has_factory_named(&self, name: &str) -> bool {
        self.factories
            .iter()
            .any(|factory| factory.get_name() == name)
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

    // Register cross-platform as the final fallback only when a platform block
    // did not already choose it as the real startup factory.
    if !registry.has_factory_named(CrossPlatformGameEngineFactory::NAME) {
        registry.register_factory(Box::new(CrossPlatformGameEngineFactory::new()));
    }

    info!(
        "Engine factory system initialized with {} factories",
        registry.factories.len()
    );
    registry
}

/// Cross-platform game engine factory
pub struct CrossPlatformGameEngineFactory;

impl Default for CrossPlatformGameEngineFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl CrossPlatformGameEngineFactory {
    const NAME: &'static str = "CrossPlatformGameEngineFactory";

    pub fn new() -> Self {
        Self
    }
}

impl GameEngineFactory for CrossPlatformGameEngineFactory {
    fn create_game_engine(&self) -> Box<dyn GameEngine> {
        Box::new(crate::game_engine::CrossPlatformGameEngine::new())
    }

    fn get_name(&self) -> &str {
        Self::NAME
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
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_subsystem_factory() {
        let factory = DefaultSubsystemFactory::new(DebugConfig::default());
        let mut audio = factory.create_audio_subsystem();

        assert_eq!(audio.get_name(), "DefaultAudioSubsystem");
        assert!(!audio.is_initialized());

        audio.init().await.unwrap();
        assert!(audio.is_initialized());

        {
            let audio = audio
                .as_any_mut()
                .downcast_mut::<DefaultAudioSubsystem>()
                .expect("expected DefaultAudioSubsystem");
            assert!(audio.is_hardware_ready());
            assert_eq!(audio.active_sound_count(), 0);
            assert_eq!(audio.current_music_name(), None);
        }

        audio
            .play_music("Data/Audio/Tracks/chi_10.mp3", true)
            .await
            .unwrap();
        {
            let audio = audio
                .as_any_mut()
                .downcast_mut::<DefaultAudioSubsystem>()
                .expect("expected DefaultAudioSubsystem");
            assert_eq!(
                audio.current_music_name(),
                Some("Data/Audio/Tracks/chi_10.mp3")
            );
        }

        audio.shutdown().await.unwrap();
        assert!(!audio.is_initialized());
    }

    #[tokio::test]
    async fn test_default_config_subsystem_layers_ini_bootstrap_files() {
        let tempdir = tempdir().unwrap();
        let defaults = tempdir.path().join("GameData.ini");
        let overrides = tempdir.path().join("GameDataDebug.ini");
        let output = tempdir.path().join("SavedConfig.ini");

        std::fs::write(
            &defaults,
            "[General]\naudio_on = false\nmusic_on = false\nresolution = 1024x768\n",
        )
        .unwrap();
        std::fs::write(
            &overrides,
            "[General]\nmusic_on = true\n[Startup]\nplay_intro = false\n",
        )
        .unwrap();

        let debug_config = DebugConfig {
            debug_ui_enabled: false,
            log_level: "info".to_string(),
            ..DebugConfig::default()
        };

        let mut config = DefaultConfigSubsystem::new(debug_config);
        config.set_bootstrap_paths(vec![defaults.clone()]);
        config.init().await.unwrap();

        assert_eq!(config.loaded_bootstrap_paths(), &[defaults.clone()]);
        assert_eq!(
            config.get_string_value("General.audio_on").as_deref(),
            Some("false")
        );
        assert_eq!(
            config.get_string_value("General.music_on").as_deref(),
            Some("false")
        );

        config.set_bootstrap_paths(vec![defaults.clone(), overrides.clone()]);
        config.init_config().await.unwrap();

        assert_eq!(
            config.loaded_bootstrap_paths(),
            &[defaults.clone(), overrides.clone()]
        );
        assert_eq!(
            config.get_string_value("General.audio_on").as_deref(),
            Some("false")
        );
        assert_eq!(
            config.get_string_value("General.music_on").as_deref(),
            Some("true")
        );
        assert_eq!(
            config.get_string_value("Startup.play_intro").as_deref(),
            Some("false")
        );

        config.save_config(output.to_str().unwrap()).await.unwrap();
        let saved = std::fs::read_to_string(&output).unwrap();
        assert!(saved.contains("[General]"));
        assert!(saved.contains("music_on = true"));
        assert!(saved.contains("play_intro = false"));
    }

    #[tokio::test]
    async fn test_default_asset_subsystem_uses_search_path_order() {
        let tempdir = tempdir().unwrap();
        let first_root = tempdir.path().join("first");
        let second_root = tempdir.path().join("second");
        let first_asset = first_root.join("ui/texture.dds");
        let second_asset = second_root.join("ui/texture.dds");
        let audio_asset = first_root.join("audio/boot.wav");

        std::fs::create_dir_all(first_asset.parent().unwrap()).unwrap();
        std::fs::create_dir_all(second_asset.parent().unwrap()).unwrap();
        std::fs::create_dir_all(audio_asset.parent().unwrap()).unwrap();

        std::fs::write(&first_asset, b"first").unwrap();
        std::fs::write(&second_asset, b"second").unwrap();
        std::fs::write(&audio_asset, b"boot").unwrap();

        let factory = DefaultSubsystemFactory::new(DebugConfig::default());
        let mut asset = factory.create_asset_subsystem();
        let asset = asset
            .as_any_mut()
            .downcast_mut::<DefaultAssetSubsystem>()
            .expect("expected DefaultAssetSubsystem");
        asset.set_search_paths(vec![first_root.clone(), second_root.clone()]);
        asset.init().await.unwrap();

        let texture_id = asset.load_texture("ui/texture.dds").await.unwrap();
        let texture = asset
            .get_asset(texture_id)
            .and_then(|asset| asset.downcast_ref::<LoadedTextureAsset>())
            .expect("expected texture asset");
        assert_eq!(texture.path, first_asset.to_string_lossy());
        assert_eq!(texture.bytes, b"first");
        assert_eq!(
            asset.loaded_asset_paths(),
            &[first_asset.to_string_lossy().to_string()]
        );

        asset
            .preload_assets(&["audio/boot.wav", "ui/texture.dds"])
            .await
            .unwrap();
        assert_eq!(
            asset.loaded_asset_paths(),
            &[
                first_asset.to_string_lossy().to_string(),
                audio_asset.to_string_lossy().to_string(),
                first_asset.to_string_lossy().to_string()
            ]
        );
    }

    #[tokio::test]
    async fn test_default_graphics_subsystem_tracks_frame_state() {
        let factory = DefaultSubsystemFactory::new(DebugConfig::default());
        let mut graphics = factory.create_graphics_subsystem();
        graphics.init().await.unwrap();
        graphics.begin_frame().await.unwrap();
        graphics.clear_screen([0.1, 0.2, 0.3, 1.0]);
        graphics.end_frame().await.unwrap();

        let graphics = graphics
            .as_any()
            .downcast_ref::<DefaultGraphicsSubsystem>()
            .expect("expected DefaultGraphicsSubsystem");
        assert!(!graphics.is_frame_active());
        assert!(!graphics.is_fullscreen());
        assert_eq!(graphics.last_clear_color(), [0.1, 0.2, 0.3, 1.0]);
    }

    #[tokio::test]
    async fn test_default_input_subsystem_tracks_key_and_mouse_state() {
        let factory = DefaultSubsystemFactory::new(DebugConfig::default());
        let mut input = factory.create_input_subsystem();
        input.init().await.unwrap();

        let input = input
            .as_any_mut()
            .downcast_mut::<DefaultInputSubsystem>()
            .expect("expected DefaultInputSubsystem");
        input.set_key_state(27, true);
        input.set_mouse_button_state(1, true);
        input.set_mouse_position(320, 240);
        input.set_mouse_delta(8, -3);

        assert!(input.is_key_pressed(27));
        assert!(input.is_mouse_button_pressed(1));
        assert_eq!(input.get_mouse_position(), (320, 240));
        assert_eq!(input.get_mouse_delta(), (8, -3));
    }

    #[tokio::test]
    async fn test_default_network_subsystem_tracks_mode_and_buffers() {
        let factory = DefaultSubsystemFactory::new(DebugConfig::default());
        let mut network = factory.create_network_subsystem();
        network.init().await.unwrap();

        {
            let network = network
                .as_any_mut()
                .downcast_mut::<DefaultNetworkSubsystem>()
                .expect("expected DefaultNetworkSubsystem");
            assert!(!network.is_connected());
            assert!(matches!(network.mode(), DefaultNetworkMode::Disconnected));
        }

        network.start_server(1234).await.unwrap();
        network.broadcast_data(b"hello").await.unwrap();

        {
            let network = network
                .as_any_mut()
                .downcast_mut::<DefaultNetworkSubsystem>()
                .expect("expected DefaultNetworkSubsystem");
            assert!(network.is_connected());
            assert!(matches!(
                network.mode(),
                DefaultNetworkMode::Server { port } if *port == 1234
            ));
            assert_eq!(network.pending_inbound_packets(), 1);
            assert_eq!(network.pending_outbound_packets(), 1);
        }

        assert_eq!(network.receive_data().await.unwrap(), b"hello".to_vec());
        network.disconnect().await.unwrap();

        network.connect_to_server("127.0.0.1", 4321).await.unwrap();
        network.send_to_server(b"ping").await.unwrap();

        {
            let network = network
                .as_any()
                .downcast_ref::<DefaultNetworkSubsystem>()
                .expect("expected DefaultNetworkSubsystem");
            assert!(network.is_connected());
            assert_eq!(
                network.connected_endpoint().as_deref(),
                Some("127.0.0.1:4321")
            );
            assert!(matches!(network.mode(), DefaultNetworkMode::Client { .. }));
        }

        assert_eq!(network.receive_data().await.unwrap(), b"ping".to_vec());
        network.disconnect().await.unwrap();

        let network = network
            .as_any()
            .downcast_ref::<DefaultNetworkSubsystem>()
            .expect("expected DefaultNetworkSubsystem");
        assert!(!network.is_connected());
        assert!(matches!(network.mode(), DefaultNetworkMode::Disconnected));
        assert_eq!(network.pending_inbound_packets(), 0);
        assert_eq!(network.pending_outbound_packets(), 0);
    }

    #[tokio::test]
    async fn test_default_game_logic_subsystem_starts_idle_until_requested() {
        let mut logic = DefaultGameLogicSubsystem::new();
        logic.init().await.unwrap();

        assert!(logic.is_initialized());
        assert_eq!(logic.game_logic.game_mode(), GameMode::None);
        assert!(logic.game_logic.get_players().is_empty());
        assert!(logic.game_logic.get_objects().is_empty());

        logic.game_logic.start_new_game(GameMode::Skirmish);
        assert_eq!(logic.game_logic.game_mode(), GameMode::Skirmish);
        assert!(!logic.game_logic.get_players().is_empty());
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

    #[test]
    fn startup_factory_registry_does_not_duplicate_cross_platform_fallback() {
        let registry = initialize_engine_factories();
        let cross_platform_count = registry
            .factories
            .iter()
            .filter(|factory| factory.get_name() == CrossPlatformGameEngineFactory::NAME)
            .count();

        assert_eq!(
            cross_platform_count, 1,
            "startup should not register a duplicate fallback client factory"
        );
    }
}
