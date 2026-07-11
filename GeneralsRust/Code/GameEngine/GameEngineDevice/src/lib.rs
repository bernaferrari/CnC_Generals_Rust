//! # `GameEngineDevice` - Complete Hardware Abstraction Layer
//!
//! This crate provides the complete device abstraction layer for Command & Conquer Generals Zero Hour,
//! converting the original C++ `GameEngineDevice` to modern Rust with enhanced performance and safety.
//!
//! ## Architecture Overview
//!
//! The `GameEngineDevice` system provides three main device layers:
//!
//! ### Audio Device Layer (`audio/`)
//! - **`MilesAudioDevice`**: Modern conversion of the original Miles Sound System integration
//! - **`AudioDriver`**: Cross-platform audio driver abstraction (WASAPI, ALSA, `CoreAudio`)
//! - **`SoundBuffer`**: High-performance audio buffer management with zero-copy where possible
//! - **`StreamingSound`**: Efficient streaming audio for large files (music, speech)
//! - **`DeviceManager`**: Audio device enumeration, selection, and lifecycle management
//!
//! ### Video Device Layer (`video/`)
//! - **`VideoDevice`**: Display device abstraction and management
//! - **`DisplayAdapter`**: Graphics adapter enumeration and capability detection
//! - **`RenderDevice`**: Rendering device abstraction for various graphics APIs
//!
//! ### W3D Device Layer (`w3d/`)
//! - **`W3DDevice`**: Westwood 3D graphics system integration
//! - **Renderer**: High-performance 3D rendering with modern graphics APIs
//! - **`GraphicsContext`**: Graphics context management and state tracking
//!
//! ### Platform Layer (`platform/`)
//! - **`Win32Device`**: Windows-specific device implementations
//! - **`DeviceInterface`**: Cross-platform device interface abstractions
//!
//! ## Key Features
//!
//! - **Memory Safety**: Zero-cost abstractions with compile-time guarantees
//! - **Cross-Platform**: Native support for Windows, Linux, and macOS
//! - **High Performance**: SIMD optimizations and hardware acceleration where available
//! - **Async Support**: Full async/await support for non-blocking operations
//! - **Hardware Detection**: Automatic device capability detection and optimization
//! - **Resource Management**: Automatic resource cleanup and leak prevention
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use game_engine_device::{GameEngineDevice, DeviceConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Initialize the complete device system
//!     let mut device_system = GameEngineDevice::new().await?;
//!
//!     // Configure audio with high performance settings
//!     let audio_config = DeviceConfig::audio()
//!         .with_sample_rate(44100)
//!         .with_channels(2)
//!         .with_low_latency(true)
//!         .with_hardware_acceleration(true);
//!
//!     // Initialize audio device
//!     let audio_device = device_system.init_audio_device(audio_config).await?;
//!
//!     // Play a sound with 3D positioning
//!     let sound_handle = audio_device
//!         .play_sound("explosion.wav")
//!         .at_position([10.0, 0.0, 5.0])
//!         .with_volume(0.8)
//!         .with_priority(crate::audio::Priority::High)
//!         .await?;
//!
//!     // Initialize video device for rendering
//!     let video_config = DeviceConfig::video()
//!         .with_resolution(1920, 1080)
//!         .with_fullscreen(false)
//!         .with_vsync(true);
//!
//!     let video_device = device_system.init_video_device(video_config).await?;
//!
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::struct_excessive_bools)]
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

// Core device modules
#[cfg(feature = "audio")]
#[cfg_attr(docsrs, doc(cfg(feature = "audio")))]
pub mod audio;

#[cfg(feature = "video")]
#[cfg_attr(docsrs, doc(cfg(feature = "video")))]
pub mod video;

#[cfg(feature = "w3d")]
#[cfg_attr(docsrs, doc(cfg(feature = "w3d")))]
pub mod w3d;

#[cfg(feature = "input")]
#[cfg_attr(docsrs, doc(cfg(feature = "input")))]
pub mod input;

pub mod platform;
pub mod w3_d_device;

// Re-exports for convenience
#[cfg(feature = "audio")]
pub use audio::{
    AudioDeviceError, AudioDriver, AudioFormat, AudioHandle, DeviceManager, MilesAudioDevice,
    Priority as AudioPriority, SoundBuffer, StreamingSound, Volume,
};

#[cfg(feature = "video")]
pub use video::{
    DisplayAdapter, RefreshRate, RenderDevice, Resolution, VideoDevice, VideoDeviceError,
};

#[cfg(feature = "w3d")]
pub use w3d::{GraphicsContext, RenderTarget, Shader, W3DDevice, W3DError, W3DRenderer};

#[cfg(feature = "input")]
pub use input::{
    ActionBinding, BindingConfig, GamepadAxis, GamepadButton, GamepadDevice, GamepadId,
    GamepadState, Hotkey, HotkeyManager, HotkeyTrigger, InputBinding, InputConfig, InputError,
    InputEvent, InputFrame, InputManager, InputRecorder, InputState, InputStateTracker,
    KeyBindingManager, KeyCode, KeyboardDevice, KeyboardState, ModifierKeys, MouseButton,
    MouseDevice, MouseState, PlaybackMode,
};

pub use platform::{DeviceInterface, PlatformError};

/// Main error type for `GameEngineDevice` operations
#[derive(Error, Debug)]
pub enum GameEngineDeviceError {
    /// Audio device related errors
    #[cfg(feature = "audio")]
    #[error("Audio device error: {0}")]
    Audio(#[from] AudioDeviceError),

    /// Video device related errors  
    #[cfg(feature = "video")]
    #[error("Video device error: {0}")]
    Video(#[from] VideoDeviceError),

    /// W3D device related errors
    #[cfg(feature = "w3d")]
    #[error("W3D device error: {0}")]
    W3D(#[from] W3DError),

    /// Platform-specific errors
    #[error("Platform error: {0}")]
    Platform(#[from] PlatformError),

    /// Device initialization failed
    #[error("Device initialization failed: {0}")]
    InitializationFailed(String),

    /// Device not available
    #[error("Device not available: {0}")]
    DeviceNotAvailable(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Resource error
    #[error("Resource error: {0}")]
    Resource(String),
}

/// Result type for `GameEngineDevice` operations
pub type Result<T> = std::result::Result<T, GameEngineDeviceError>;

/// Device configuration builder
#[derive(Debug, Clone)]
pub struct DeviceConfig {
    /// Device type
    pub device_type: DeviceType,
    /// Configuration parameters
    pub parameters: std::collections::HashMap<String, serde_json::Value>,
}

/// Supported device types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceType {
    /// Audio device
    #[cfg(feature = "audio")]
    Audio,
    /// Video device
    #[cfg(feature = "video")]
    Video,
    /// W3D graphics device
    #[cfg(feature = "w3d")]
    W3D,
    /// Input device
    #[cfg(feature = "input")]
    Input,
}

/// Device status information
#[derive(Debug, Clone)]
pub struct DeviceStatus {
    /// Device type
    pub device_type: DeviceType,
    /// Whether device is initialized
    pub initialized: bool,
    /// Whether device is active
    pub active: bool,
    /// Device capabilities
    pub capabilities: DeviceCapabilities,
    /// Performance metrics
    pub performance: PerformanceMetrics,
}

/// Device capabilities
#[derive(Debug, Clone, Default)]
pub struct DeviceCapabilities {
    /// Hardware acceleration available
    pub hardware_acceleration: bool,
    /// Multi-threading support
    pub multi_threading: bool,
    /// SIMD instruction support
    pub simd_support: bool,
    /// Platform-specific features
    pub platform_features: Vec<String>,
}

/// Performance metrics for device monitoring
#[derive(Debug, Clone, Default)]
pub struct PerformanceMetrics {
    /// CPU usage percentage (0.0 - 1.0)
    pub cpu_usage: f32,
    /// Memory usage in bytes
    pub memory_usage: u64,
    /// Latency in milliseconds
    pub latency_ms: f32,
    /// Throughput in operations per second
    pub throughput: f32,
}

/// Main `GameEngineDevice` system
pub struct GameEngineDevice {
    /// Audio device manager
    #[cfg(feature = "audio")]
    audio_manager: Arc<RwLock<Option<DeviceManager>>>,

    /// Video device
    #[cfg(feature = "video")]
    video_device: Arc<RwLock<Option<VideoDevice>>>,

    /// W3D device
    #[cfg(feature = "w3d")]
    w3d_device: Arc<RwLock<Option<W3DDevice>>>,

    /// Platform interface
    platform_interface: Arc<DeviceInterface>,

    /// System configuration
    config: Arc<RwLock<SystemConfig>>,
}

/// System-wide configuration
#[derive(Debug, Clone)]
pub struct SystemConfig {
    /// Enable debug logging
    pub debug_mode: bool,
    /// Enable performance monitoring
    pub performance_monitoring: bool,
    /// Maximum device count per type
    pub max_devices: usize,
    /// Device initialization timeout in milliseconds
    pub init_timeout_ms: u64,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            debug_mode: cfg!(debug_assertions),
            performance_monitoring: true,
            max_devices: 16,
            init_timeout_ms: 5000,
        }
    }
}

impl DeviceConfig {
    /// Create new audio device configuration
    #[cfg(feature = "audio")]
    pub fn audio() -> Self {
        Self {
            device_type: DeviceType::Audio,
            parameters: std::collections::HashMap::new(),
        }
    }

    /// Create new video device configuration
    #[cfg(feature = "video")]
    pub fn video() -> Self {
        Self {
            device_type: DeviceType::Video,
            parameters: std::collections::HashMap::new(),
        }
    }

    /// Create new W3D device configuration
    #[cfg(feature = "w3d")]
    pub fn w3d() -> Self {
        Self {
            device_type: DeviceType::W3D,
            parameters: std::collections::HashMap::new(),
        }
    }

    /// Set a configuration parameter
    pub fn with_parameter<T: serde::Serialize>(mut self, key: &str, value: T) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.parameters.insert(key.to_string(), json_value);
        }
        self
    }
}

impl GameEngineDevice {
    /// Create a new `GameEngineDevice` system
    pub async fn new() -> Result<Self> {
        Self::with_config(SystemConfig::default()).await
    }

    /// Create a new `GameEngineDevice` system with custom configuration
    pub async fn with_config(config: SystemConfig) -> Result<Self> {
        let platform_interface = Arc::new(DeviceInterface::new().await?);

        Ok(Self {
            #[cfg(feature = "audio")]
            audio_manager: Arc::new(RwLock::new(None)),

            #[cfg(feature = "video")]
            video_device: Arc::new(RwLock::new(None)),

            #[cfg(feature = "w3d")]
            w3d_device: Arc::new(RwLock::new(None)),

            platform_interface,
            config: Arc::new(RwLock::new(config)),
        })
    }

    /// Initialize audio device with configuration
    #[cfg(feature = "audio")]
    pub async fn init_audio_device(&self, config: DeviceConfig) -> Result<Arc<DeviceManager>> {
        let mut audio_lock = self.audio_manager.write().await;

        if audio_lock.is_none() {
            let manager = DeviceManager::new()
                .await
                .map_err(GameEngineDeviceError::Audio)?;
            *audio_lock = Some(manager);
        }

        Ok(Arc::new(audio_lock.as_ref().unwrap().clone()))
    }

    /// Initialize video device with configuration
    #[cfg(feature = "video")]
    pub async fn init_video_device(&self, config: DeviceConfig) -> Result<Arc<VideoDevice>> {
        let mut video_lock = self.video_device.write().await;

        if video_lock.is_none() {
            let video_config = video_config_from_device_config(&config);
            let mut device = VideoDevice::new_with_config(video_config)
                .await
                .map_err(GameEngineDeviceError::Video)?;
            device.init().await.map_err(GameEngineDeviceError::Video)?;
            *video_lock = Some(device);
        }

        Ok(Arc::new(video_lock.as_ref().unwrap().clone()))
    }

    /// Initialize W3D device with configuration
    #[cfg(feature = "w3d")]
    pub async fn init_w3d_device(&self, config: DeviceConfig) -> Result<Arc<W3DDevice>> {
        let mut w3d_lock = self.w3d_device.write().await;

        if w3d_lock.is_none() {
            let w3d_config = w3d_config_from_device_config(&config);
            let device = W3DDevice::new_with_config(w3d_config)
                .await
                .map_err(GameEngineDeviceError::W3D)?;
            device.init().await.map_err(GameEngineDeviceError::W3D)?;
            *w3d_lock = Some(device);
        }

        Ok(Arc::new(w3d_lock.as_ref().unwrap().clone()))
    }

    /// Get system status for all initialized devices
    pub async fn get_system_status(&self) -> Result<Vec<DeviceStatus>> {
        let statuses = Vec::new();

        #[cfg(feature = "audio")]
        if let Some(audio_manager) = self.audio_manager.read().await.as_ref() {
            statuses.push(audio_manager.get_status().await?);
        }

        #[cfg(feature = "video")]
        if let Some(video_device) = self.video_device.read().await.as_ref() {
            statuses.push(video_device.get_status().await?);
        }

        #[cfg(feature = "w3d")]
        if let Some(w3d_device) = self.w3d_device.read().await.as_ref() {
            statuses.push(w3d_device.get_status().await?);
        }

        Ok(statuses)
    }

    /// Shutdown all devices gracefully
    pub async fn shutdown(&self) -> Result<()> {
        // Shutdown in reverse order of initialization

        #[cfg(feature = "w3d")]
        if let Some(w3d_device) = self.w3d_device.write().await.take() {
            w3d_device.shutdown().await?;
        }

        #[cfg(feature = "video")]
        if let Some(video_device) = self.video_device.write().await.take() {
            video_device.shutdown().await?;
        }

        #[cfg(feature = "audio")]
        if let Some(audio_manager) = self.audio_manager.write().await.take() {
            audio_manager
                .shutdown()
                .await
                .map_err(GameEngineDeviceError::Audio)?;
        }

        Ok(())
    }

    /// Get performance metrics for all devices
    pub async fn get_performance_metrics(
        &self,
    ) -> Result<std::collections::HashMap<DeviceType, PerformanceMetrics>> {
        let metrics = std::collections::HashMap::new();

        #[cfg(feature = "audio")]
        if let Some(audio_manager) = self.audio_manager.read().await.as_ref() {
            let performance = audio_manager.get_performance_metrics().await?;
            metrics.insert(DeviceType::Audio, performance);
        }

        #[cfg(feature = "video")]
        if let Some(video_device) = self.video_device.read().await.as_ref() {
            let performance = video_device.get_performance_metrics().await?;
            metrics.insert(DeviceType::Video, performance);
        }

        #[cfg(feature = "w3d")]
        if let Some(w3d_device) = self.w3d_device.read().await.as_ref() {
            let performance = w3d_device.get_performance_metrics().await?;
            metrics.insert(DeviceType::W3D, performance);
        }

        Ok(metrics)
    }
}

#[cfg(feature = "video")]
fn video_config_from_device_config(config: &DeviceConfig) -> video::VideoDeviceConfig {
    let mut video_config = video::VideoDeviceConfig::default();

    if let (Some(width), Some(height)) = (
        config
            .parameters
            .get("width")
            .and_then(serde_json::Value::as_u64),
        config
            .parameters
            .get("height")
            .and_then(serde_json::Value::as_u64),
    ) {
        video_config.resolution = video::Resolution::new(width as u32, height as u32);
    }
    if let Some(fullscreen) = config
        .parameters
        .get("fullscreen")
        .and_then(serde_json::Value::as_bool)
    {
        video_config.fullscreen = fullscreen;
    }
    if let Some(vsync_enabled) = config
        .parameters
        .get("vsync")
        .and_then(serde_json::Value::as_bool)
    {
        video_config.vsync = if vsync_enabled {
            video::VSync::Enabled
        } else {
            video::VSync::Disabled
        };
    }
    if let Some(msaa_samples) = config
        .parameters
        .get("msaa_samples")
        .and_then(serde_json::Value::as_u64)
    {
        video_config.msaa = match msaa_samples as u32 {
            0 | 1 => video::MsaaSettings::none(),
            2 => video::MsaaSettings::msaa_2x(),
            4 => video::MsaaSettings::msaa_4x(),
            8 => video::MsaaSettings::msaa_8x(),
            _ => video_config.msaa,
        };
    }
    if let Some(title) = config
        .parameters
        .get("window_title")
        .and_then(serde_json::Value::as_str)
    {
        video_config.window_title = title.to_string();
    }
    if let Some(resizable) = config
        .parameters
        .get("window_resizable")
        .and_then(serde_json::Value::as_bool)
    {
        video_config.window_resizable = resizable;
    }

    video_config
}

#[cfg(feature = "w3d")]
fn w3d_config_from_device_config(config: &DeviceConfig) -> w3d::W3DConfig {
    let mut w3d_config = w3d::W3DConfig::default();

    if let (Some(width), Some(height)) = (
        config
            .parameters
            .get("width")
            .and_then(serde_json::Value::as_u64),
        config
            .parameters
            .get("height")
            .and_then(serde_json::Value::as_u64),
    ) {
        w3d_config.resolution = video::Resolution::new(width as u32, height as u32);
    }
    if let Some(vsync_enabled) = config
        .parameters
        .get("vsync")
        .and_then(serde_json::Value::as_bool)
    {
        w3d_config.vsync = vsync_enabled;
    }
    if let Some(msaa_samples) = config
        .parameters
        .get("msaa_samples")
        .and_then(serde_json::Value::as_u64)
    {
        w3d_config.msaa = match msaa_samples as u32 {
            0 | 1 => video::MsaaSettings::none(),
            2 => video::MsaaSettings::msaa_2x(),
            4 => video::MsaaSettings::msaa_4x(),
            8 => video::MsaaSettings::msaa_8x(),
            _ => w3d_config.msaa,
        };
    }
    if let Some(debug_mode) = config
        .parameters
        .get("debug_mode")
        .and_then(serde_json::Value::as_bool)
    {
        w3d_config.debug_mode = debug_mode;
    }
    if let Some(max_lights) = config
        .parameters
        .get("max_lights")
        .and_then(serde_json::Value::as_u64)
    {
        w3d_config.max_lights = max_lights as u32;
    }

    w3d_config
}

impl Drop for GameEngineDevice {
    fn drop(&mut self) {
        // Note: In a real implementation, we'd need to handle async shutdown
        // This is a simplified version for demonstration
        tracing::info!("GameEngineDevice shutting down");
    }
}

/// Initialize the `GameEngineDevice` system with default configuration
pub async fn init() -> Result<GameEngineDevice> {
    GameEngineDevice::new().await
}

/// Initialize the `GameEngineDevice` system with custom configuration  
pub async fn init_with_config(config: SystemConfig) -> Result<GameEngineDevice> {
    GameEngineDevice::with_config(config).await
}

/// Get version information
#[must_use]
pub fn version_info() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_device_system_creation() {
        let device_system = GameEngineDevice::new().await;
        assert!(device_system.is_ok());
    }

    #[tokio::test]
    async fn test_device_config_builder() {
        #[cfg(feature = "audio")]
        {
            let config = DeviceConfig::audio()
                .with_parameter("sample_rate", 44100)
                .with_parameter("channels", 2);

            assert_eq!(config.device_type, DeviceType::Audio);
            assert!(config.parameters.contains_key("sample_rate"));
            assert!(config.parameters.contains_key("channels"));
        }
    }

    #[test]
    fn test_version_info() {
        let version = version_info();
        assert!(!version.is_empty());
        assert!(version.contains('.'));
    }

    #[cfg(feature = "w3d")]
    #[test]
    fn reload_all_textures_clears_model_loader_texture_cache() {
        let mut loader = crate::w3d::model_loader::W3DModelLoader::new();
        loader.cache_texture_for_test("particle.tga", vec![1, 2, 3, 4]);
        loader.cache_texture_for_test("terrain.tga", vec![5, 6]);

        loader.reload_all_textures();

        assert_eq!(loader.texture_cache_len_for_test(), 0);
    }

    #[cfg(feature = "w3d")]
    #[test]
    fn global_reload_all_textures_delegates_to_model_loader() {
        let mut loader = crate::w3d::model_loader::W3DModelLoader::new();
        loader.cache_texture_for_test("effect.tga", vec![9, 8, 7]);

        crate::w3d::model_loader::reload_all_textures(&mut loader);

        assert_eq!(loader.texture_cache_len_for_test(), 0);
    }
}
