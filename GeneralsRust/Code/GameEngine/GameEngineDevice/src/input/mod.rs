//! # Input Device Abstraction
//!
//! Cross-platform input device abstraction for keyboard, mouse, and gamepad support.
//!
//! ## Architecture
//!
//! The input system provides:
//! - **Keyboard**: Key press/release events with modifier support
//! - **Mouse**: Position, button, and scroll events
//! - **Gamepad**: Controller input with standard button mapping
//! - **Hotkeys**: Configurable keyboard shortcuts
//! - **Key Bindings**: Remappable action bindings
//! - **Input Recording**: Replay system for deterministic playback
//!
//! ## Example Usage
//!
//! ```rust
//! use game_engine_device::input::{InputManager, InputEvent, KeyCode};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let mut input_manager = InputManager::new().await?;
//!
//!     // Poll for input events
//!     while let Some(event) = input_manager.poll_event().await {
//!         match event {
//!             InputEvent::KeyPressed { key, modifiers } => {
//!                 println!("Key pressed: {:?} with modifiers: {:?}", key, modifiers);
//!             }
//!             InputEvent::MouseMoved { x, y } => {
//!                 println!("Mouse moved to: ({}, {})", x, y);
//!             }
//!             _ => {}
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod bindings;
mod gamepad;
mod hotkey;
mod keyboard;
mod mouse;
mod recorder;
mod state;

pub use bindings::{ActionBinding, BindingConfig, InputBinding, KeyBindingManager};
pub use gamepad::{GamepadButton, GamepadDevice, GamepadId, GamepadState};
pub use hotkey::{Hotkey, HotkeyManager, HotkeyTrigger};
pub use keyboard::{KeyCode, KeyboardDevice, KeyboardState, ModifierKeys};
pub use mouse::{MouseButton, MouseDevice, MouseState};
pub use recorder::{InputFrame, InputRecorder, PlaybackMode};
pub use state::{InputState, InputStateTracker};

// Platform-specific implementations
#[cfg(target_os = "windows")]
mod platform {
    pub mod windows;
    pub use windows::*;
}

#[cfg(target_os = "linux")]
mod platform {
    pub mod linux;
    pub use linux::*;
}

#[cfg(target_os = "macos")]
mod platform {
    pub mod macos;
    pub use macos::*;
}

/// Input device errors
#[derive(Error, Debug)]
pub enum InputError {
    /// Device initialization failed
    #[error("Device initialization failed: {0}")]
    InitializationFailed(String),

    /// Device not found
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    /// Platform-specific error
    #[error("Platform error: {0}")]
    PlatformError(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    /// Recording error
    #[error("Recording error: {0}")]
    RecordingError(String),

    /// Playback error
    #[error("Playback error: {0}")]
    PlaybackError(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Result type for input operations
pub type Result<T> = std::result::Result<T, InputError>;

/// Input event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputEvent {
    /// Key was pressed
    KeyPressed {
        key: KeyCode,
        modifiers: ModifierKeys,
        timestamp: Duration,
    },

    /// Key was released
    KeyReleased {
        key: KeyCode,
        modifiers: ModifierKeys,
        timestamp: Duration,
    },

    /// Key is being held (repeat event)
    KeyRepeat {
        key: KeyCode,
        modifiers: ModifierKeys,
        timestamp: Duration,
    },

    /// Mouse moved
    MouseMoved {
        x: i32,
        y: i32,
        delta_x: i32,
        delta_y: i32,
        timestamp: Duration,
    },

    /// Mouse button pressed
    MouseButtonPressed {
        button: MouseButton,
        x: i32,
        y: i32,
        timestamp: Duration,
    },

    /// Mouse button released
    MouseButtonReleased {
        button: MouseButton,
        x: i32,
        y: i32,
        timestamp: Duration,
    },

    /// Mouse wheel scrolled
    MouseWheel {
        delta_x: f32,
        delta_y: f32,
        timestamp: Duration,
    },

    /// Gamepad connected
    GamepadConnected {
        id: GamepadId,
        name: String,
        timestamp: Duration,
    },

    /// Gamepad disconnected
    GamepadDisconnected { id: GamepadId, timestamp: Duration },

    /// Gamepad button pressed
    GamepadButtonPressed {
        id: GamepadId,
        button: GamepadButton,
        timestamp: Duration,
    },

    /// Gamepad button released
    GamepadButtonReleased {
        id: GamepadId,
        button: GamepadButton,
        timestamp: Duration,
    },

    /// Gamepad axis moved
    GamepadAxisMoved {
        id: GamepadId,
        axis: GamepadAxis,
        value: f32,
        timestamp: Duration,
    },

    /// Hotkey triggered
    HotkeyTriggered { name: String, timestamp: Duration },

    /// Action triggered via binding
    ActionTriggered {
        action: String,
        value: f32,
        timestamp: Duration,
    },
}

/// Gamepad axis types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GamepadAxis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
    LeftTrigger,
    RightTrigger,
}

/// Input device configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConfig {
    /// Enable keyboard input
    pub keyboard_enabled: bool,

    /// Enable mouse input
    pub mouse_enabled: bool,

    /// Enable gamepad input
    pub gamepad_enabled: bool,

    /// Mouse sensitivity multiplier
    pub mouse_sensitivity: f32,

    /// Enable raw mouse input (no OS acceleration)
    pub raw_mouse_input: bool,

    /// Key repeat delay in milliseconds
    pub key_repeat_delay_ms: u64,

    /// Key repeat rate in milliseconds
    pub key_repeat_rate_ms: u64,

    /// Gamepad dead zone (0.0 - 1.0)
    pub gamepad_dead_zone: f32,

    /// Enable input recording
    pub recording_enabled: bool,

    /// Maximum event queue size
    pub max_queue_size: usize,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            keyboard_enabled: true,
            mouse_enabled: true,
            gamepad_enabled: true,
            mouse_sensitivity: 1.0,
            raw_mouse_input: true,
            key_repeat_delay_ms: 500,
            key_repeat_rate_ms: 33,
            gamepad_dead_zone: 0.15,
            recording_enabled: false,
            max_queue_size: 1024,
        }
    }
}

/// Main input manager coordinating all input devices
pub struct InputManager {
    /// Configuration
    config: Arc<RwLock<InputConfig>>,

    /// Keyboard device
    keyboard: Arc<RwLock<KeyboardDevice>>,

    /// Mouse device
    mouse: Arc<RwLock<MouseDevice>>,

    /// Gamepad devices
    gamepads: Arc<RwLock<HashMap<GamepadId, GamepadDevice>>>,

    /// Hotkey manager
    hotkey_manager: Arc<RwLock<HotkeyManager>>,

    /// Key binding manager
    binding_manager: Arc<RwLock<KeyBindingManager>>,

    /// Input state tracker
    state_tracker: Arc<RwLock<InputStateTracker>>,

    /// Input recorder
    recorder: Arc<RwLock<Option<InputRecorder>>>,

    /// Event queue
    event_queue: Arc<RwLock<VecDeque<InputEvent>>>,

    /// Start time for timestamp calculation
    start_time: Instant,

    /// Platform-specific backend
    #[cfg(target_os = "windows")]
    platform_backend: Arc<Mutex<platform::WindowsInputBackend>>,

    #[cfg(target_os = "linux")]
    platform_backend: Arc<Mutex<platform::LinuxInputBackend>>,

    #[cfg(target_os = "macos")]
    platform_backend: Arc<Mutex<platform::MacOSInputBackend>>,
}

impl InputManager {
    /// Create a new input manager with default configuration
    pub async fn new() -> Result<Self> {
        Self::with_config(InputConfig::default()).await
    }

    /// Create a new input manager with custom configuration
    pub async fn with_config(config: InputConfig) -> Result<Self> {
        let start_time = Instant::now();

        // Initialize platform backend
        #[cfg(target_os = "windows")]
        let platform_backend = Arc::new(Mutex::new(platform::WindowsInputBackend::new()?));

        #[cfg(target_os = "linux")]
        let platform_backend = Arc::new(Mutex::new(platform::LinuxInputBackend::new()?));

        #[cfg(target_os = "macos")]
        let platform_backend = Arc::new(Mutex::new(platform::MacOSInputBackend::new()?));

        // Initialize devices
        let keyboard = Arc::new(RwLock::new(KeyboardDevice::new(
            config.key_repeat_delay_ms,
            config.key_repeat_rate_ms,
        )?));

        let mouse = Arc::new(RwLock::new(MouseDevice::new(
            config.mouse_sensitivity,
            config.raw_mouse_input,
        )?));

        let gamepads = Arc::new(RwLock::new(HashMap::new()));

        let hotkey_manager = Arc::new(RwLock::new(HotkeyManager::new()));
        let binding_manager = Arc::new(RwLock::new(KeyBindingManager::new()));
        let state_tracker = Arc::new(RwLock::new(InputStateTracker::new()));

        let recorder = if config.recording_enabled {
            Arc::new(RwLock::new(Some(InputRecorder::new())))
        } else {
            Arc::new(RwLock::new(None))
        };

        let event_queue = Arc::new(RwLock::new(VecDeque::with_capacity(config.max_queue_size)));

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            keyboard,
            mouse,
            gamepads,
            hotkey_manager,
            binding_manager,
            state_tracker,
            recorder,
            event_queue,
            start_time,
            platform_backend,
        })
    }

    /// Poll for the next input event
    pub async fn poll_event(&mut self) -> Option<InputEvent> {
        // Update platform backend
        if let Err(e) = self.update_platform_events().await {
            tracing::error!("Failed to update platform events: {}", e);
            return None;
        }

        // Get next event from queue
        self.event_queue.write().pop_front()
    }

    /// Get all pending events
    pub async fn poll_all_events(&mut self) -> Vec<InputEvent> {
        // Update platform backend
        if let Err(e) = self.update_platform_events().await {
            tracing::error!("Failed to update platform events: {}", e);
            return Vec::new();
        }

        // Drain all events from queue
        let mut queue = self.event_queue.write();
        queue.drain(..).collect()
    }

    /// Update input state from platform backend
    async fn update_platform_events(&mut self) -> Result<()> {
        let timestamp = self.start_time.elapsed();
        let events = {
            let mut backend = self.platform_backend.lock();
            backend.poll_events()?
        };

        // Process events
        for event in events {
            self.process_event(event, timestamp).await;
        }

        Ok(())
    }

    /// Process a single input event
    async fn process_event(&mut self, event: InputEvent, timestamp: Duration) {
        // Update state tracker
        self.state_tracker.write().update(&event);

        // Check hotkeys
        let hotkey_name = self.hotkey_manager.write().check_event(&event);
        if let Some(hotkey_name) = hotkey_name {
            let hotkey_event = InputEvent::HotkeyTriggered {
                name: hotkey_name.clone(),
                timestamp,
            };
            self.queue_event(hotkey_event.clone()).await;

            // Record hotkey event
            if let Some(recorder) = self.recorder.write().as_mut() {
                recorder.record_event(&hotkey_event);
            }
        }

        // Check bindings
        let binding_result = self.binding_manager.write().check_event(&event);
        if let Some((action, value)) = binding_result {
            let action_event = InputEvent::ActionTriggered {
                action: action.clone(),
                value,
                timestamp,
            };
            self.queue_event(action_event.clone()).await;

            // Record action event
            if let Some(recorder) = self.recorder.write().as_mut() {
                recorder.record_event(&action_event);
            }
        }

        // Queue original event
        self.queue_event(event.clone()).await;

        // Record original event
        if let Some(recorder) = self.recorder.write().as_mut() {
            recorder.record_event(&event);
        }
    }

    /// Queue an input event
    async fn queue_event(&mut self, event: InputEvent) {
        let mut queue = self.event_queue.write();
        let config = self.config.read();

        // Check queue size limit
        if queue.len() >= config.max_queue_size {
            // Remove oldest event
            queue.pop_front();
            tracing::warn!("Input event queue full, dropping oldest event");
        }

        queue.push_back(event);
    }

    /// Get current keyboard state
    pub fn keyboard_state(&self) -> KeyboardState {
        self.keyboard.read().state()
    }

    /// Get current mouse state
    pub fn mouse_state(&self) -> MouseState {
        self.mouse.read().state()
    }

    /// Get gamepad state by ID
    pub fn gamepad_state(&self, id: GamepadId) -> Option<GamepadState> {
        self.gamepads.read().get(&id).map(|g| g.state())
    }

    /// Get all connected gamepad IDs
    pub fn connected_gamepads(&self) -> Vec<GamepadId> {
        self.gamepads.read().keys().copied().collect()
    }

    /// Register a hotkey
    pub fn register_hotkey(&self, name: impl Into<String>, hotkey: Hotkey) -> Result<()> {
        self.hotkey_manager.write().register(name.into(), hotkey);
        Ok(())
    }

    /// Unregister a hotkey
    pub fn unregister_hotkey(&self, name: &str) -> Result<()> {
        self.hotkey_manager.write().unregister(name);
        Ok(())
    }

    /// Bind an action to input
    pub fn bind_action(&self, action: impl Into<String>, binding: InputBinding) -> Result<()> {
        self.binding_manager
            .write()
            .bind_action(action.into(), binding);
        Ok(())
    }

    /// Unbind an action
    pub fn unbind_action(&self, action: &str) -> Result<()> {
        self.binding_manager.write().unbind_action(action);
        Ok(())
    }

    /// Load binding configuration from file
    pub async fn load_bindings(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let config = BindingConfig::load(path)?;
        self.binding_manager.write().load_config(config);
        Ok(())
    }

    /// Save binding configuration to file
    pub async fn save_bindings(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let config = self.binding_manager.read().save_config();
        config.save(path)?;
        Ok(())
    }

    /// Start recording input
    pub fn start_recording(&self) -> Result<()> {
        let mut recorder_lock = self.recorder.write();
        if recorder_lock.is_none() {
            *recorder_lock = Some(InputRecorder::new());
        }

        if let Some(recorder) = recorder_lock.as_mut() {
            recorder.start();
        }

        Ok(())
    }

    /// Stop recording input
    pub fn stop_recording(&self) -> Result<()> {
        if let Some(recorder) = self.recorder.write().as_mut() {
            recorder.stop();
        }
        Ok(())
    }

    /// Save recording to file
    pub async fn save_recording(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        if let Some(recorder) = self.recorder.read().as_ref() {
            recorder.save(path)?;
        } else {
            return Err(InputError::RecordingError("No recording available".into()));
        }
        Ok(())
    }

    /// Load and start playback of recording
    pub async fn load_recording(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let mut recorder_lock = self.recorder.write();
        let mut recorder = InputRecorder::new();
        recorder.load(path)?;
        *recorder_lock = Some(recorder);
        Ok(())
    }

    /// Start playback of loaded recording
    pub fn start_playback(&self, mode: PlaybackMode) -> Result<()> {
        if let Some(recorder) = self.recorder.write().as_mut() {
            recorder.start_playback(mode)?;
        } else {
            return Err(InputError::PlaybackError("No recording loaded".into()));
        }
        Ok(())
    }

    /// Stop playback
    pub fn stop_playback(&self) -> Result<()> {
        if let Some(recorder) = self.recorder.write().as_mut() {
            recorder.stop_playback();
        }
        Ok(())
    }

    /// Update input system (call once per frame)
    pub async fn update(&mut self, delta_time: Duration) -> Result<()> {
        // Update keyboard (handle key repeats)
        self.keyboard.write().update(delta_time);

        // Update mouse
        self.mouse.write().update(delta_time);

        // Update gamepads
        for gamepad in self.gamepads.write().values_mut() {
            gamepad.update(delta_time)?;
        }

        // Update recorder/playback
        let mut playback_events = Vec::new();
        {
            let mut recorder_guard = self.recorder.write();
            if let Some(recorder) = recorder_guard.as_mut() {
                if recorder.is_playing() {
                    // Collect playback events
                    while let Some(event) = recorder.get_playback_event(self.start_time.elapsed()) {
                        playback_events.push(event);
                    }
                }
            }
        }

        // Queue playback events after releasing the lock
        for event in playback_events {
            self.queue_event(event).await;
        }

        Ok(())
    }

    /// Get input configuration
    pub fn config(&self) -> InputConfig {
        self.config.read().clone()
    }

    /// Update input configuration
    pub fn set_config(&mut self, config: InputConfig) {
        *self.config.write() = config;
    }

    /// Get current input state snapshot
    pub fn get_state_snapshot(&self) -> InputState {
        self.state_tracker.read().snapshot()
    }

    /// Clear all input state
    pub fn clear_state(&mut self) {
        self.keyboard.write().clear();
        self.mouse.write().clear();
        for gamepad in self.gamepads.write().values_mut() {
            gamepad.clear();
        }
        self.state_tracker.write().clear();
        self.event_queue.write().clear();
    }

    /// Shutdown input system
    pub async fn shutdown(&mut self) -> Result<()> {
        // Stop any active recording/playback
        self.stop_recording()?;
        self.stop_playback()?;

        // Clear all state
        self.clear_state();

        // Shutdown platform backend
        self.platform_backend.lock().shutdown()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_input_manager_creation() {
        let manager = InputManager::new().await;
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_custom_config() {
        let config = InputConfig {
            mouse_sensitivity: 2.0,
            gamepad_dead_zone: 0.2,
            ..Default::default()
        };

        let manager = InputManager::with_config(config).await;
        assert!(manager.is_ok());
    }

    #[test]
    fn test_event_serialization() {
        let event = InputEvent::KeyPressed {
            key: KeyCode::A,
            modifiers: ModifierKeys::empty(),
            timestamp: Duration::from_millis(100),
        };

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: InputEvent = serde_json::from_str(&json).unwrap();

        match deserialized {
            InputEvent::KeyPressed { key, .. } => assert_eq!(key, KeyCode::A),
            _ => panic!("Wrong event type"),
        }
    }
}
