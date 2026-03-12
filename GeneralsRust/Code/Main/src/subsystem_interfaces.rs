////////////////////////////////////////////////////////////////////////////////
//
//  (c) 2001-2003 Electronic Arts Inc.
//
////////////////////////////////////////////////////////////////////////////////

// FILE: subsystem_interfaces.rs
//
// Enhanced subsystem interfaces for the game engine
// Defines the contracts for all major game subsystems
//
// Author: Colin Day, April 2001 (Converted to Rust)
//
///////////////////////////////////////////////////////////////////////////////

use anyhow::Result;
use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;
use ww3d_engine::FrameTiming;

/// Base trait for all game subsystems
/// Equivalent to C++ SubsystemInterface
#[async_trait]
pub trait SubsystemInterface: Send + Sync {
    /// Initialize the subsystem
    async fn init(&mut self) -> Result<()>;

    /// Update the subsystem (called every frame)
    async fn update(&mut self, delta_time: f32) -> Result<()>;

    /// Update hook that includes full frame timing (defaults to `update`)
    async fn update_with_timing(&mut self, timing: &FrameTiming) -> Result<()> {
        self.update(timing.delta_seconds()).await
    }

    /// Shutdown and cleanup the subsystem
    async fn shutdown(&mut self) -> Result<()>;

    /// Get the subsystem's name
    fn get_name(&self) -> &str;

    /// Get the subsystem as Any for downcasting
    fn as_any(&self) -> &dyn Any;

    /// Get the subsystem as mutable Any for downcasting
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Check if the subsystem is initialized
    fn is_initialized(&self) -> bool;

    /// Get subsystem priority for initialization order
    fn get_priority(&self) -> i32 {
        0
    }
}

/// Audio subsystem interface
#[async_trait]
pub trait AudioSubsystem: SubsystemInterface {
    /// Initialize audio hardware
    async fn init_audio_hardware(&mut self) -> Result<()>;

    /// Play a sound effect
    async fn play_sound(&mut self, sound_name: &str, volume: f32) -> Result<()>;

    /// Play background music
    async fn play_music(&mut self, music_name: &str, loop_music: bool) -> Result<()>;

    /// Stop all audio
    async fn stop_all_audio(&mut self) -> Result<()>;

    /// Set master volume
    fn set_master_volume(&mut self, volume: f32);

    /// Get master volume
    fn get_master_volume(&self) -> f32;
}

/// Graphics subsystem interface
#[async_trait]
pub trait GraphicsSubsystem: SubsystemInterface {
    /// Initialize graphics hardware
    async fn init_graphics_hardware(
        &mut self,
        width: u32,
        height: u32,
        fullscreen: bool,
    ) -> Result<()>;

    /// Begin frame rendering
    async fn begin_frame(&mut self) -> Result<()>;

    /// End frame rendering and present
    async fn end_frame(&mut self) -> Result<()>;

    /// Clear the screen
    fn clear_screen(&mut self, color: [f32; 4]);

    /// Resize the display
    fn resize(&mut self, width: u32, height: u32);

    /// Get current resolution
    fn get_resolution(&self) -> (u32, u32);
}

/// Input subsystem interface
#[async_trait]
pub trait InputSubsystem: SubsystemInterface {
    /// Initialize input devices
    async fn init_input_devices(&mut self) -> Result<()>;

    /// Update input state
    async fn update_input(&mut self) -> Result<()>;

    /// Check if a key is pressed
    fn is_key_pressed(&self, key_code: u32) -> bool;

    /// Check if a mouse button is pressed
    fn is_mouse_button_pressed(&self, button: u32) -> bool;

    /// Get mouse position
    fn get_mouse_position(&self) -> (i32, i32);

    /// Get mouse delta movement
    fn get_mouse_delta(&self) -> (i32, i32);
}

/// Network subsystem interface
#[async_trait]
pub trait NetworkSubsystem: SubsystemInterface {
    /// Initialize networking
    async fn init_network(&mut self) -> Result<()>;

    /// Start as a server
    async fn start_server(&mut self, port: u16) -> Result<()>;

    /// Connect to a server
    async fn connect_to_server(&mut self, host: &str, port: u16) -> Result<()>;

    /// Send data to all clients (server mode)
    async fn broadcast_data(&mut self, data: &[u8]) -> Result<()>;

    /// Send data to server (client mode)
    async fn send_to_server(&mut self, data: &[u8]) -> Result<()>;

    /// Receive data
    async fn receive_data(&mut self) -> Result<Vec<u8>>;

    /// Disconnect from network
    async fn disconnect(&mut self) -> Result<()>;

    /// Check if connected
    fn is_connected(&self) -> bool;
}

/// File system subsystem interface
#[async_trait]
pub trait FileSystemSubsystem: SubsystemInterface {
    /// Initialize file system
    async fn init_file_system(&mut self) -> Result<()>;

    /// Read a file
    async fn read_file(&self, path: &str) -> Result<Vec<u8>>;

    /// Write a file
    async fn write_file(&self, path: &str, data: &[u8]) -> Result<()>;

    /// Check if a file exists
    fn file_exists(&self, path: &str) -> bool;

    /// List files in a directory
    fn list_directory(&self, path: &str) -> Result<Vec<String>>;

    /// Get file size
    fn get_file_size(&self, path: &str) -> Result<u64>;
}

/// Asset management subsystem interface
#[async_trait]
pub trait AssetSubsystem: SubsystemInterface {
    /// Initialize asset management
    async fn init_assets(&mut self) -> Result<()>;

    /// Load a texture
    async fn load_texture(&mut self, path: &str) -> Result<u32>; // Returns texture ID

    /// Load a 3D model
    async fn load_model(&mut self, path: &str) -> Result<u32>; // Returns model ID

    /// Load an audio file
    async fn load_audio(&mut self, path: &str) -> Result<u32>; // Returns audio ID

    /// Unload an asset
    async fn unload_asset(&mut self, asset_id: u32) -> Result<()>;

    /// Get asset by ID
    fn get_asset(&self, asset_id: u32) -> Option<&dyn Any>;

    /// Preload assets
    async fn preload_assets(&mut self, asset_list: &[&str]) -> Result<()>;
}

/// Game logic subsystem interface
#[async_trait]
pub trait GameLogicSubsystem: SubsystemInterface {
    /// Initialize game logic
    async fn init_game_logic(&mut self) -> Result<()>;

    /// Update game simulation
    async fn update_simulation(&mut self, delta_time: f32) -> Result<()>;

    /// Process game commands
    async fn process_commands(&mut self) -> Result<()>;

    /// Handle player input
    async fn handle_input(&mut self, input_events: &[InputEvent]) -> Result<()>;

    /// Save game state
    async fn save_game(&self, slot: u32) -> Result<()>;

    /// Load game state
    async fn load_game(&mut self, slot: u32) -> Result<()>;
}

/// Configuration subsystem interface
#[async_trait]
pub trait ConfigSubsystem: SubsystemInterface {
    /// Initialize configuration system
    async fn init_config(&mut self) -> Result<()>;

    /// Load configuration from file
    async fn load_config(&mut self, path: &str) -> Result<()>;

    /// Save configuration to file
    async fn save_config(&self, path: &str) -> Result<()>;

    /// Get a configuration value as a string
    fn get_string_value(&self, key: &str) -> Option<String>;

    /// Set a configuration value from a string
    fn set_string_value(&mut self, key: &str, value: String);

    /// Get all configuration keys
    fn get_keys(&self) -> Vec<String>;
}

/// Input event types
#[derive(Debug, Clone)]
pub enum InputEvent {
    KeyPressed {
        key_code: u32,
    },
    KeyReleased {
        key_code: u32,
    },
    MousePressed {
        button: u32,
        x: i32,
        y: i32,
    },
    MouseReleased {
        button: u32,
        x: i32,
        y: i32,
    },
    MouseMoved {
        x: i32,
        y: i32,
        delta_x: i32,
        delta_y: i32,
    },
    MouseWheel {
        delta: i32,
    },
}

/// Subsystem factory trait for creating subsystems
pub trait SubsystemFactory: Send + Sync {
    /// Create an audio subsystem
    fn create_audio_subsystem(&self) -> Box<dyn AudioSubsystem>;

    /// Create a graphics subsystem
    fn create_graphics_subsystem(&self) -> Box<dyn GraphicsSubsystem>;

    /// Create an input subsystem
    fn create_input_subsystem(&self) -> Box<dyn InputSubsystem>;

    /// Create a network subsystem
    fn create_network_subsystem(&self) -> Box<dyn NetworkSubsystem>;

    /// Create a file system subsystem
    fn create_file_system_subsystem(&self) -> Box<dyn FileSystemSubsystem>;

    /// Create an asset subsystem
    fn create_asset_subsystem(&self) -> Box<dyn AssetSubsystem>;

    /// Create a game logic subsystem
    fn create_game_logic_subsystem(&self) -> Box<dyn GameLogicSubsystem>;

    /// Create a configuration subsystem
    fn create_config_subsystem(&self) -> Box<dyn ConfigSubsystem>;
}

/// Simplified subsystem manager for handling all subsystems
pub struct SubsystemManager {
    initialized: bool,
    subsystems: Vec<SubsystemSlot>,
}

struct SubsystemSlot {
    name: String,
    subsystem: Box<dyn SubsystemInterface + Send + Sync>,
}

impl SubsystemManager {
    /// Create a new subsystem manager
    pub fn new() -> Self {
        Self {
            initialized: false,
            subsystems: Vec::new(),
        }
    }

    /// Initialize all subsystems using a factory
    pub async fn init_with_factory(&mut self, factory: Arc<dyn SubsystemFactory>) -> Result<()> {
        log::info!("Initializing subsystems via factory...");

        let mut created: Vec<Box<dyn SubsystemInterface + Send + Sync>> = vec![
            factory.create_file_system_subsystem(),
            factory.create_config_subsystem(),
            factory.create_audio_subsystem(),
            factory.create_graphics_subsystem(),
            factory.create_input_subsystem(),
            factory.create_network_subsystem(),
            factory.create_asset_subsystem(),
            factory.create_game_logic_subsystem(),
        ];

        for subsystem in created.iter_mut() {
            subsystem.init().await?;
        }

        self.subsystems = created
            .into_iter()
            .map(|subsystem| SubsystemSlot {
                name: subsystem.get_name().to_string(),
                subsystem,
            })
            .collect();

        self.initialized = true;

        log::info!(
            "All {} subsystems initialized successfully",
            self.subsystems.len()
        );
        Ok(())
    }

    /// Update all subsystems (simplified implementation)
    pub async fn update_all(&mut self, delta_time: f32) -> Result<()> {
        if !self.initialized {
            return Err(anyhow::anyhow!("Subsystems not initialized"));
        }

        for slot in &mut self.subsystems {
            log::trace!("Updating subsystem {}", slot.name);
            slot.subsystem.update(delta_time).await?;
        }
        Ok(())
    }

    /// Update all subsystems with WW3D frame timing data
    pub async fn update_all_with_timing(&mut self, timing: &FrameTiming) -> Result<()> {
        if !self.initialized {
            return Err(anyhow::anyhow!("Subsystems not initialized"));
        }

        for slot in &mut self.subsystems {
            log::trace!("Updating subsystem {} with timing", slot.name);
            slot.subsystem.update_with_timing(timing).await?;
        }
        Ok(())
    }

    /// Shutdown all subsystems (simplified implementation)
    pub async fn shutdown_all(&mut self) -> Result<()> {
        if !self.initialized {
            return Ok(()); // Already shut down
        }

        log::info!("Shutting down {} subsystems...", self.subsystems.len());

        for slot in self.subsystems.iter_mut().rev() {
            log::info!("Shutting down subsystem {}", slot.name);
            slot.subsystem.shutdown().await?;
        }

        self.initialized = false;
        self.subsystems.clear();

        log::info!("All subsystems shut down successfully");
        Ok(())
    }
}

impl Default for SubsystemManager {
    fn default() -> Self {
        Self::new()
    }
}

// Safe wrapper for subsystem management
pub struct SafeSubsystemManager {
    inner: Arc<tokio::sync::RwLock<SubsystemManager>>,
}

impl SafeSubsystemManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(tokio::sync::RwLock::new(SubsystemManager::new())),
        }
    }

    pub async fn init_with_factory(&self, factory: Arc<dyn SubsystemFactory>) -> Result<()> {
        let mut manager = self.inner.write().await;
        manager.init_with_factory(factory).await
    }

    pub async fn update_all(&self, delta_time: f32) -> Result<()> {
        let mut manager = self.inner.write().await;
        manager.update_all(delta_time).await
    }

    pub async fn update_all_with_timing(&self, timing: &FrameTiming) -> Result<()> {
        let mut manager = self.inner.write().await;
        manager.update_all_with_timing(timing).await
    }

    pub async fn shutdown_all(&self) -> Result<()> {
        let mut manager = self.inner.write().await;
        manager.shutdown_all().await
    }
}

impl Default for SafeSubsystemManager {
    fn default() -> Self {
        Self::new()
    }
}
