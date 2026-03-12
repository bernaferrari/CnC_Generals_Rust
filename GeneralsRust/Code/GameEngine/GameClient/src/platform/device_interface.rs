//! # Device Interface Definitions
//!
//! Core trait definitions for platform-agnostic device abstraction.
//! These interfaces provide a unified API across all supported platforms
//! while allowing for platform-specific optimizations.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::fmt::Debug;

use super::{AudioApi, DeviceCapabilities, GraphicsApi, PlatformResult};

/// Unique identifier for platform devices
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub u64);

impl DeviceId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        DeviceId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

/// Device status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceStatus {
    /// Device is available and ready for use
    Available,
    /// Device is currently in use by this application
    InUse,
    /// Device is in use by another application
    Busy,
    /// Device is present but disabled
    Disabled,
    /// Device has encountered an error
    Error,
    /// Device is not present or disconnected
    NotPresent,
}

/// Base trait for all platform devices
#[async_trait]
pub trait PlatformDevice: Send + Sync + Debug {
    /// Get unique device identifier
    fn device_id(&self) -> DeviceId;

    /// Get human-readable device name
    fn name(&self) -> &str;

    /// Get device capabilities
    fn capabilities(&self) -> DeviceCapabilities;

    /// Get current device status
    fn status(&self) -> DeviceStatus;

    /// Check if device is available for use
    fn is_available(&self) -> bool {
        self.status() == DeviceStatus::Available
    }

    /// Initialize the device
    async fn initialize(&mut self) -> PlatformResult<()>;

    /// Shutdown and cleanup device resources
    async fn shutdown(&mut self) -> PlatformResult<()>;

    /// Reset device to initial state
    async fn reset(&mut self) -> PlatformResult<()>;

    /// Get platform-specific device handle (if available)
    fn platform_handle(&self) -> Option<&dyn Any>;

    /// Get device-specific properties as key-value pairs
    fn properties(&self) -> std::collections::HashMap<String, String>;
}

/// Display mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
    pub bit_depth: u32,
    pub hdr_enabled: bool,
}

/// Display/Graphics device interface
#[async_trait]
pub trait DisplayDevice: PlatformDevice {
    /// Get supported graphics APIs
    fn supported_graphics_apis(&self) -> Vec<GraphicsApi>;

    /// Get current graphics API
    fn current_graphics_api(&self) -> Option<GraphicsApi>;

    /// Set graphics API (requires reinitialization)
    async fn set_graphics_api(&mut self, api: GraphicsApi) -> PlatformResult<()>;

    /// Get available display modes
    async fn get_available_modes(&self) -> PlatformResult<Vec<DisplayMode>>;

    /// Get current display mode
    fn current_mode(&self) -> Option<DisplayMode>;

    /// Set display mode
    async fn set_display_mode(
        &mut self,
        width: u32,
        height: u32,
        fullscreen: bool,
    ) -> PlatformResult<()>;

    /// Enable/disable VSync
    async fn set_vsync(&mut self, enabled: bool) -> PlatformResult<()>;

    /// Get VSync status
    fn vsync_enabled(&self) -> bool;

    /// Set HDR mode
    async fn set_hdr(&mut self, enabled: bool) -> PlatformResult<()>;

    /// Get HDR status
    fn hdr_enabled(&self) -> bool;

    /// Get display adapter information
    fn adapter_info(&self) -> DisplayAdapterInfo;

    /// Take screenshot
    async fn capture_screenshot(&self) -> PlatformResult<Vec<u8>>;

    /// Set gamma/color correction
    async fn set_gamma(&mut self, gamma: f32, brightness: f32, contrast: f32)
        -> PlatformResult<()>;

    /// Create platform-specific surface/context
    async fn create_render_surface(
        &mut self,
        window_handle: &dyn Any,
    ) -> PlatformResult<Box<dyn Any>>;
}

/// Display adapter information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayAdapterInfo {
    pub name: String,
    pub vendor: String,
    pub device_id: u32,
    pub vendor_id: u32,
    pub memory_mb: u64,
    pub driver_version: String,
    pub supports_ray_tracing: bool,
    pub supports_variable_rate_shading: bool,
    pub supports_mesh_shaders: bool,
}

/// Audio format specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u32,
    pub bit_depth: u32,
    pub buffer_size: u32,
}

/// Audio device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub is_input: bool,
    pub is_output: bool,
    pub is_default: bool,
    pub max_channels: u32,
    pub supported_sample_rates: Vec<u32>,
    pub supported_formats: Vec<AudioFormat>,
}

/// Audio device interface
#[async_trait]
pub trait AudioDevice: PlatformDevice {
    /// Get supported audio APIs
    fn supported_audio_apis(&self) -> Vec<AudioApi>;

    /// Get current audio API
    fn current_audio_api(&self) -> Option<AudioApi>;

    /// Set audio API (requires reinitialization)
    async fn set_audio_api(&mut self, api: AudioApi) -> PlatformResult<()>;

    /// Get audio device information
    fn device_info(&self) -> AudioDeviceInfo;

    /// Get supported audio formats
    fn supported_formats(&self) -> Vec<AudioFormat>;

    /// Get current audio format
    fn current_format(&self) -> Option<AudioFormat>;

    /// Set audio format
    async fn set_format(&mut self, format: AudioFormat) -> PlatformResult<()>;

    /// Start audio processing
    async fn start(&mut self) -> PlatformResult<()>;

    /// Stop audio processing
    async fn stop(&mut self) -> PlatformResult<()>;

    /// Check if audio is running
    fn is_running(&self) -> bool;

    /// Set master volume (0.0 to 1.0)
    async fn set_volume(&mut self, volume: f32) -> PlatformResult<()>;

    /// Get master volume
    fn volume(&self) -> f32;

    /// Mute/unmute audio
    async fn set_muted(&mut self, muted: bool) -> PlatformResult<()>;

    /// Get mute status
    fn is_muted(&self) -> bool;

    /// Get current latency in milliseconds
    fn latency_ms(&self) -> f32;

    /// Enable low-latency mode (if supported)
    async fn set_low_latency_mode(&mut self, enabled: bool) -> PlatformResult<()>;

    /// Register audio callback for real-time processing
    async fn set_audio_callback(&mut self, callback: Box<dyn AudioCallback>) -> PlatformResult<()>;
}

/// Audio processing callback trait
pub trait AudioCallback: Send + Sync {
    /// Process audio samples
    /// - `input`: Input audio samples (empty if output-only device)
    /// - `output`: Output buffer to fill with audio samples
    /// - `sample_rate`: Current sample rate
    /// - `channels`: Number of audio channels
    fn process_audio(&mut self, input: &[f32], output: &mut [f32], sample_rate: u32, channels: u32);
}

/// Input event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputEventType {
    KeyboardEvent {
        key_code: u32,
        scan_code: u32,
        pressed: bool,
        modifiers: u32,
    },
    MouseEvent {
        button: u32,
        pressed: bool,
        x: f32,
        y: f32,
    },
    MouseMove {
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
    },
    MouseWheel {
        delta_x: f32,
        delta_y: f32,
    },
    GamepadEvent {
        gamepad_id: u32,
        button: u32,
        value: f32,
    },
    TouchEvent {
        finger_id: u64,
        x: f32,
        y: f32,
        pressure: f32,
        phase: u32, // 0=begin, 1=move, 2=end, 3=cancel
    },
}

/// Input device types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputDeviceType {
    Keyboard,
    Mouse,
    Gamepad,
    Joystick,
    TouchScreen,
    Stylus,
    Other,
}

/// Input device interface
#[async_trait]
pub trait InputDevice: PlatformDevice {
    /// Get input device type
    fn device_type(&self) -> InputDeviceType;

    /// Check if device supports force feedback
    fn supports_force_feedback(&self) -> bool;

    /// Set force feedback effect
    async fn set_force_feedback(&mut self, effect: ForceFeedbackEffect) -> PlatformResult<()>;

    /// Enable/disable input device
    async fn set_enabled(&mut self, enabled: bool) -> PlatformResult<()>;

    /// Check if device is enabled
    fn is_enabled(&self) -> bool;

    /// Poll for input events
    async fn poll_events(&mut self) -> PlatformResult<Vec<InputEventType>>;

    /// Set input event callback
    async fn set_event_callback(
        &mut self,
        callback: Box<dyn InputEventCallback>,
    ) -> PlatformResult<()>;

    /// Get device-specific properties
    fn input_properties(&self) -> InputDeviceProperties;
}

/// Force feedback effect types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ForceFeedbackEffect {
    Rumble { intensity: f32, duration_ms: u32 },
    Spring { strength: f32, offset: f32 },
    Damper { strength: f32 },
    Inertia { strength: f32 },
    Friction { strength: f32 },
}

/// Input device properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDeviceProperties {
    pub has_keys: bool,
    pub key_count: u32,
    pub has_pointer: bool,
    pub has_wheel: bool,
    pub has_touch: bool,
    pub max_touch_points: u32,
    pub has_accelerometer: bool,
    pub has_gyroscope: bool,
    pub has_force_feedback: bool,
    pub axis_count: u32,
    pub button_count: u32,
}

/// Input event callback trait
pub trait InputEventCallback: Send + Sync {
    /// Handle input event
    fn on_input_event(&mut self, event: InputEventType);
}

/// Power states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerState {
    Active,
    Sleep,
    Hibernate,
    Shutdown,
}

/// Battery information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryInfo {
    pub present: bool,
    pub charging: bool,
    pub level_percent: u32,
    pub time_remaining_minutes: Option<u32>,
}

/// Power management interface
#[async_trait]
pub trait PowerDevice: PlatformDevice {
    /// Get battery information
    async fn battery_info(&self) -> PlatformResult<BatteryInfo>;

    /// Check if running on AC power
    async fn is_on_ac_power(&self) -> PlatformResult<bool>;

    /// Request system power state
    async fn request_power_state(&mut self, state: PowerState) -> PlatformResult<()>;

    /// Prevent system sleep
    async fn prevent_sleep(&mut self, prevent: bool) -> PlatformResult<()>;

    /// Register power event callback
    async fn set_power_callback(
        &mut self,
        callback: Box<dyn PowerEventCallback>,
    ) -> PlatformResult<()>;
}

/// Power event callback trait
pub trait PowerEventCallback: Send + Sync {
    fn on_battery_changed(&mut self, info: BatteryInfo);
    fn on_power_source_changed(&mut self, on_ac: bool);
    fn on_power_state_requested(&mut self, state: PowerState);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_id_generation() {
        let id1 = DeviceId::new();
        let id2 = DeviceId::new();
        assert_ne!(id1, id2);
        assert!(id1.0 > 0);
        assert!(id2.0 > 0);
    }

    #[test]
    fn test_device_status_equality() {
        assert_eq!(DeviceStatus::Available, DeviceStatus::Available);
        assert_ne!(DeviceStatus::Available, DeviceStatus::InUse);
    }

    #[test]
    fn test_display_adapter_info_creation() {
        let info = DisplayAdapterInfo {
            name: "Test GPU".to_string(),
            vendor: "Test Vendor".to_string(),
            device_id: 0x1234,
            vendor_id: 0x5678,
            memory_mb: 8192,
            driver_version: "1.0.0".to_string(),
            supports_ray_tracing: true,
            supports_variable_rate_shading: false,
            supports_mesh_shaders: true,
        };

        assert_eq!(info.name, "Test GPU");
        assert_eq!(info.memory_mb, 8192);
        assert!(info.supports_ray_tracing);
        assert!(!info.supports_variable_rate_shading);
    }
}
