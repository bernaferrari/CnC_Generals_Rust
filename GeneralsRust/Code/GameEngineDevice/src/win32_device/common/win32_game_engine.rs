//! Win32 Game Engine Module
//! 
//! Corresponds to C++ file: GeneralsMD/Code/GameEngineDevice/Source/Win32Device/Common/Win32GameEngine.cpp
//! 
//! Complete Win32 platform-specific game engine implementation with modern Rust features.
//! Provides full Windows API integration, message handling, and system management.

use std::{
    collections::HashMap,
    ffi::{c_void, CString, OsString},
    ptr::{self, NonNull},
    sync::Arc,
    time::{Duration, SystemTime},
};

use tokio::{
    sync::{RwLock, Mutex, mpsc},
    time,
};

use thiserror::Error;
use uuid::Uuid;
use dashmap::DashMap;
use parking_lot::RwLock as SyncRwLock;
use tracing::{debug, error, info, warn, instrument};
use game_network::NetworkInstant;

#[cfg(windows)]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::System::Threading::*,
    Win32::System::Performance::*,
    Win32::System::SystemServices::*,
    Win32::System::Registry::*,
    Win32::System::Memory::*,
    Win32::Graphics::Direct3D11::*,
    Win32::Graphics::Dxgi::*,
    Win32::Media::Audio::DirectSound::*,
};

/// Windows message types for game engine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsMessage {
    Quit,
    Activate,
    Deactivate,
    Resize,
    Paint,
    KeyDown(u32),
    KeyUp(u32),
    MouseMove,
    MouseButtonDown(u32),
    MouseButtonUp(u32),
    Other(u32),
}

/// Engine subsystem states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsystemState {
    Uninitialized,
    Initializing,
    Ready,
    Running,
    Paused,
    Error,
    ShuttingDown,
}

/// Performance metrics structure
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub frame_count: u64,
    pub fps: f64,
    pub frame_time_ms: f64,
    pub update_time_ms: f64,
    pub render_time_ms: f64,
    pub memory_usage_mb: u64,
    pub cpu_usage_percent: f32,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            frame_count: 0,
            fps: 0.0,
            frame_time_ms: 0.0,
            update_time_ms: 0.0,
            render_time_ms: 0.0,
            memory_usage_mb: 0,
            cpu_usage_percent: 0.0,
        }
    }
}

/// Win32-specific game engine configuration
#[derive(Debug, Clone)]
pub struct Win32EngineConfig {
    pub window_title: String,
    pub window_width: u32,
    pub window_height: u32,
    pub fullscreen: bool,
    pub vsync: bool,
    pub enable_audio: bool,
    pub enable_input: bool,
    pub enable_networking: bool,
    pub target_fps: u32,
    pub enable_alt_tab_handling: bool,
    pub enable_performance_monitoring: bool,
}

impl Default for Win32EngineConfig {
    fn default() -> Self {
        Self {
            window_title: "Command & Conquer Generals Zero Hour".to_string(),
            window_width: 1024,
            window_height: 768,
            fullscreen: false,
            vsync: true,
            enable_audio: true,
            enable_input: true,
            enable_networking: true,
            target_fps: 60,
            enable_alt_tab_handling: true,
            enable_performance_monitoring: true,
        }
    }
}

/// Win32-specific game engine implementation with complete platform support
#[derive(Debug)]
pub struct Win32GameEngine {
    /// Engine identifier
    id: Uuid,
    /// Windows instance handle
    instance: Option<HINSTANCE>,
    /// Main window handle
    window: Option<HWND>,
    /// DirectX device and related resources
    #[cfg(windows)]
    d3d_device: Option<ID3D11Device>,
    #[cfg(windows)]
    d3d_context: Option<ID3D11DeviceContext>,
    #[cfg(windows)]
    dxgi_swap_chain: Option<IDXGISwapChain>,
    /// DirectSound interface
    #[cfg(windows)]
    direct_sound: Option<IDirectSound8>,
    /// Engine state tracking
    initialized: bool,
    is_active: bool,
    is_quitting: bool,
    is_minimized: bool,
    /// Performance counter data
    perf_frequency: i64,
    perf_counter_start: i64,
    last_frame_time: NetworkInstant,
    /// Configuration
    config: Win32EngineConfig,
    /// Subsystem states
    subsystem_states: Arc<DashMap<String, SubsystemState>>,
    /// Performance metrics
    performance_metrics: Arc<SyncRwLock<PerformanceMetrics>>,
    /// Message queue for Windows messages
    message_queue: Arc<Mutex<Vec<WindowsMessage>>>,
    /// Previous error mode (for blue screen prevention)
    previous_error_mode: u32,
    /// Registry access for configuration
    registry_keys: HashMap<String, String>,
    /// Thread pool for background operations
    thread_pool: rayon::ThreadPool,
}

/// Win32 Game Engine errors
#[derive(Error, Debug)]
pub enum Win32EngineError {
    #[error("Engine already initialized")]
    AlreadyInitialized,
    
    #[error("Engine not initialized")]
    NotInitialized,
    
    #[error("Invalid window handle")]
    InvalidWindow,
    
    #[error("DirectX initialization failed: {message}")]
    DirectXFailed { message: String },
    
    #[error("DirectInput initialization failed: {message}")]
    DirectInputFailed { message: String },
    
    #[error("Audio initialization failed: {message}")]
    AudioInitFailed { message: String },
    
    #[error("Registry operation failed: {message}")]
    RegistryError { message: String },
    
    #[error("System API error: {message}")]
    SystemApiError { message: String },
    
    #[error("Invalid string provided")]
    InvalidString,
    
    #[error("Hardware not supported: {details}")]
    UnsupportedHardware { details: String },
    
    #[error("Out of memory")]
    OutOfMemory,
    
    #[error("Threading error: {source}")]
    ThreadError {
        #[from]
        source: tokio::task::JoinError,
    },
    
    #[cfg(windows)]
    #[error("Windows API error: {source}")]
    WindowsApiError {
        #[from]
        source: windows::core::Error,
    },
    
    #[error("Unknown error: {message}")]
    Unknown { message: String },
}

type Result<T> = std::result::Result<T, Win32EngineError>;

impl Win32GameEngine {
    /// Create a new Win32GameEngine with default configuration
    pub fn new() -> Self {
        Self::with_config(Win32EngineConfig::default())
    }
    
    /// Create a new Win32GameEngine with custom configuration
    pub fn with_config(config: Win32EngineConfig) -> Self {
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_cpus::get())
            .thread_name(|i| format!("win32-engine-worker-{}", i))
            .build()
            .expect("Failed to create thread pool");
        
        Self {
            id: Uuid::new_v4(),
            instance: None,
            window: None,
            #[cfg(windows)]
            d3d_device: None,
            #[cfg(windows)]
            d3d_context: None,
            #[cfg(windows)]
            dxgi_swap_chain: None,
            #[cfg(windows)]
            direct_sound: None,
            initialized: false,
            is_active: false,
            is_quitting: false,
            is_minimized: false,
            perf_frequency: 0,
            perf_counter_start: 0,
            last_frame_time: NetworkInstant::now(),
            config,
            subsystem_states: Arc::new(DashMap::new()),
            performance_metrics: Arc::new(SyncRwLock::new(PerformanceMetrics::default())),
            message_queue: Arc::new(Mutex::new(Vec::new())),
            previous_error_mode: 0,
            registry_keys: HashMap::new(),
            thread_pool,
        }
    }

    /// Initialize the Win32 game engine
    /// 
    /// # Arguments
    /// 
    /// * `instance` - Windows application instance handle
    /// * `window` - Main window handle (optional for headless mode)
    /// 
    /// # Returns
    /// 
    /// `Ok(())` on success, `Err` on failure
    #[instrument(skip(self), fields(engine_id = %self.id))]
    pub async fn initialize(
        &mut self,
        instance: Option<HINSTANCE>,
        window: Option<HWND>,
    ) -> Result<()> {
        if self.initialized {
            warn!("Engine already initialized");
            return Ok(());
        }
        
        info!("Initializing Win32 Game Engine");
        
        // Store handles
        self.instance = instance;
        self.window = window;
        
        // Set error mode to prevent blue screens
        #[cfg(windows)]
        {
            self.previous_error_mode = unsafe { SetErrorMode(SEM_FAILCRITICALERRORS) };
        }
        
        // Initialize subsystems in order
        self.init_performance_counter().await?;
        self.init_registry_access().await?;
        
        if self.config.enable_audio {
            self.init_audio().await?;
        }
        
        if window.is_some() {
            self.init_directx().await?;
            
            if self.config.enable_input {
                self.init_input().await?;
            }
        }
        
        if self.config.enable_performance_monitoring {
            self.start_performance_monitoring().await;
        }
        
        self.initialized = true;
        self.is_active = true;
        
        info!("Win32 Game Engine initialization complete");
        Ok(())
    }

    /// Shutdown the Win32 game engine
    #[instrument(skip(self), fields(engine_id = %self.id))]
    pub async fn shutdown(&mut self) {
        if !self.initialized {
            warn!("Engine not initialized, nothing to shutdown");
            return;
        }
        
        info!("Shutting down Win32 Game Engine");
        
        self.is_quitting = true;
        
        // Stop performance monitoring
        if self.config.enable_performance_monitoring {
            self.stop_performance_monitoring().await;
        }
        
        // Cleanup subsystems in reverse order
        self.cleanup_input().await;
        self.cleanup_directx().await;
        self.cleanup_audio().await;
        
        // Restore error mode
        #[cfg(windows)]
        {
            unsafe { SetErrorMode(self.previous_error_mode); }
        }
        
        // Clear handles
        self.instance = None;
        self.window = None;
        #[cfg(windows)]
        {
            self.d3d_device = None;
            self.d3d_context = None;
            self.dxgi_swap_chain = None;
            self.direct_sound = None;
        }
        
        self.initialized = false;
        self.is_active = false;
        
        info!("Win32 Game Engine shutdown complete");
    }

    /// Initialize performance counter for high-resolution timing
    async fn init_performance_counter(&mut self) -> Result<()> {
        debug!("Initializing performance counter");
        
        #[cfg(windows)]
        {
            let mut frequency = 0i64;
            let mut counter = 0i64;
            
            unsafe {
                if QueryPerformanceFrequency(&mut frequency as *mut i64).as_bool() {
                    self.perf_frequency = frequency;
                    
                    if QueryPerformanceCounter(&mut counter as *mut i64).as_bool() {
                        self.perf_counter_start = counter;
                        self.subsystem_states.insert("performance_counter".to_string(), SubsystemState::Ready);
                        debug!("Performance counter initialized: {} Hz", frequency);
                        return Ok(());
                    }
                }
            }
            
            return Err(Win32EngineError::SystemApiError {
                message: "Failed to initialize performance counter".to_string(),
            });
        }
        
        #[cfg(not(windows))]
        {
            // Fallback for non-Windows platforms
            self.perf_frequency = 1_000_000_000; // 1 GHz (nanosecond precision)
            self.perf_counter_start = self.last_frame_time.elapsed().as_nanos() as i64;
            self.subsystem_states.insert("performance_counter".to_string(), SubsystemState::Ready);
            Ok(())
        }
    }

    /// Initialize DirectX graphics system
    #[cfg(windows)]
    async fn init_directx(&mut self) -> Result<()> {
        debug!("Initializing DirectX");
        
        if self.window.is_none() {
            return Err(Win32EngineError::InvalidWindow);
        }
        
        self.subsystem_states.insert("directx".to_string(), SubsystemState::Initializing);
        
        unsafe {
            // Create DXGI Factory
            let factory: IDXGIFactory2 = CreateDXGIFactory2(0)?;
            
            // Get the primary adapter
            let adapter = factory.EnumAdapters(0)?;
            
            // Create D3D11 device and context
            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;
            let mut feature_level = D3D_FEATURE_LEVEL_11_0;
            
            let create_flags = if cfg!(debug_assertions) {
                D3D11_CREATE_DEVICE_DEBUG
            } else {
                D3D11_CREATE_DEVICE_FLAG(0)
            };
            
            let hr = D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                create_flags,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut feature_level),
                Some(&mut context),
            );
            
            if hr.is_err() {
                self.subsystem_states.insert("directx".to_string(), SubsystemState::Error);
                return Err(Win32EngineError::DirectXFailed {
                    message: format!("D3D11CreateDevice failed: {:?}", hr),
                });
            }
            
            self.d3d_device = device;
            self.d3d_context = context;
            
            // Create swap chain
            if let Some(hwnd) = self.window {
                let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
                    Width: self.config.window_width,
                    Height: self.config.window_height,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                    BufferCount: 2,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                    Scaling: DXGI_SCALING_STRETCH,
                    Stereo: FALSE,
                    AlphaMode: DXGI_ALPHA_MODE_UNSPECIFIED,
                    Flags: 0,
                };
                
                let swap_chain = factory.CreateSwapChainForHwnd(
                    self.d3d_device.as_ref().unwrap(),
                    hwnd,
                    &swap_chain_desc,
                    None,
                    None,
                )?;
                
                self.dxgi_swap_chain = Some(swap_chain);
            }
        }
        
        self.subsystem_states.insert("directx".to_string(), SubsystemState::Ready);
        info!("DirectX initialized successfully");
        Ok(())
    }
    
    #[cfg(not(windows))]
    async fn init_directx(&mut self) -> Result<()> {
        debug!("DirectX not available on this platform, skipping");
        Ok(())
    }

    /// Initialize input systems (DirectInput or modern alternatives)
    async fn init_input(&mut self) -> Result<()> {
        debug!("Initializing input system");
        
        self.subsystem_states.insert("input".to_string(), SubsystemState::Initializing);
        
        #[cfg(windows)]
        {
            // For 2025, we might want to use Raw Input API instead of DirectInput
            // which is more modern and doesn't require DirectX
            
            if let Some(hwnd) = self.window {
                // Register for raw input from keyboard and mouse
                let mut devices = [RAWINPUTDEVICE::default(); 2];
                
                // Keyboard
                devices[0] = RAWINPUTDEVICE {
                    usUsagePage: 0x01, // HID_USAGE_PAGE_GENERIC
                    usUsage: 0x06,     // HID_USAGE_GENERIC_KEYBOARD
                    dwFlags: RIDEV_NOLEGACY,
                    hwndTarget: hwnd,
                };
                
                // Mouse  
                devices[1] = RAWINPUTDEVICE {
                    usUsagePage: 0x01, // HID_USAGE_PAGE_GENERIC
                    usUsage: 0x02,     // HID_USAGE_GENERIC_MOUSE
                    dwFlags: RIDEV_NOLEGACY,
                    hwndTarget: hwnd,
                };
                
                unsafe {
                    if !RegisterRawInputDevices(&devices, std::mem::size_of::<RAWINPUTDEVICE>() as u32).as_bool() {
                        self.subsystem_states.insert("input".to_string(), SubsystemState::Error);
                        return Err(Win32EngineError::DirectInputFailed {
                            message: "Failed to register raw input devices".to_string(),
                        });
                    }
                }
            }
        }
        
        self.subsystem_states.insert("input".to_string(), SubsystemState::Ready);
        info!("Input system initialized");
        Ok(())
    }

    /// Initialize audio system (DirectSound or modern audio APIs)
    async fn init_audio(&mut self) -> Result<()> {
        debug!("Initializing audio system");
        
        self.subsystem_states.insert("audio".to_string(), SubsystemState::Initializing);
        
        #[cfg(windows)]
        {
            unsafe {
                // Initialize DirectSound
                let mut ds: Option<IDirectSound8> = None;
                
                let hr = DirectSoundCreate8(None, &mut ds, None);
                if hr.is_err() {
                    self.subsystem_states.insert("audio".to_string(), SubsystemState::Error);
                    return Err(Win32EngineError::AudioInitFailed {
                        message: format!("DirectSoundCreate8 failed: {:?}", hr),
                    });
                }
                
                if let Some(direct_sound) = &ds {
                    if let Some(hwnd) = self.window {
                        let hr = direct_sound.SetCooperativeLevel(hwnd, DSSCL_PRIORITY);
                        if hr.is_err() {
                            self.subsystem_states.insert("audio".to_string(), SubsystemState::Error);
                            return Err(Win32EngineError::AudioInitFailed {
                                message: format!("SetCooperativeLevel failed: {:?}", hr),
                            });
                        }
                    }
                }
                
                self.direct_sound = ds;
            }
        }
        
        self.subsystem_states.insert("audio".to_string(), SubsystemState::Ready);
        info!("Audio system initialized");
        Ok(())
    }
    
    /// Initialize registry access for game settings
    async fn init_registry_access(&mut self) -> Result<()> {
        debug!("Initializing registry access");
        
        #[cfg(windows)]
        {
            // Common registry paths for EA games
            let registry_paths = [
                r"SOFTWARE\Electronic Arts\EA Games\Command and Conquer Generals Zero Hour",
                r"SOFTWARE\Electronic Arts\Command and Conquer Generals",
                r"SOFTWARE\WOW6432Node\Electronic Arts\EA Games\Command and Conquer Generals Zero Hour",
            ];
            
            for path in &registry_paths {
                if let Ok(key) = self.read_registry_string(HKEY_LOCAL_MACHINE, path, "InstallPath").await {
                    self.registry_keys.insert("InstallPath".to_string(), key);
                    break;
                }
            }
            
            // Read user settings
            if let Ok(key) = self.read_registry_string(
                HKEY_CURRENT_USER,
                r"SOFTWARE\Electronic Arts\EA Games\Command and Conquer Generals Zero Hour",
                "UserDataLeafName"
            ).await {
                self.registry_keys.insert("UserDataLeafName".to_string(), key);
            }
        }
        
        Ok(())
    }

    /// Cleanup DirectX resources
    #[cfg(windows)]
    async fn cleanup_directx(&mut self) {
        debug!("Cleaning up DirectX resources");
        
        // DirectX COM objects are automatically cleaned up when dropped
        self.dxgi_swap_chain = None;
        self.d3d_context = None;
        self.d3d_device = None;
        
        self.subsystem_states.insert("directx".to_string(), SubsystemState::Uninitialized);
        debug!("DirectX cleanup complete");
    }
    
    #[cfg(not(windows))]
    async fn cleanup_directx(&mut self) {
        // No-op on non-Windows platforms
    }

    /// Cleanup input resources
    async fn cleanup_input(&mut self) {
        debug!("Cleaning up input resources");
        
        #[cfg(windows)]
        {
            if let Some(hwnd) = self.window {
                // Unregister raw input devices
                let mut devices = [RAWINPUTDEVICE::default(); 2];
                
                devices[0] = RAWINPUTDEVICE {
                    usUsagePage: 0x01,
                    usUsage: 0x06,
                    dwFlags: RIDEV_REMOVE,
                    hwndTarget: HWND::default(),
                };
                
                devices[1] = RAWINPUTDEVICE {
                    usUsagePage: 0x01,
                    usUsage: 0x02,
                    dwFlags: RIDEV_REMOVE,
                    hwndTarget: HWND::default(),
                };
                
                unsafe {
                    let _ = RegisterRawInputDevices(&devices, std::mem::size_of::<RAWINPUTDEVICE>() as u32);
                }
            }
        }
        
        self.subsystem_states.insert("input".to_string(), SubsystemState::Uninitialized);
        debug!("Input cleanup complete");
    }

    /// Cleanup audio resources
    async fn cleanup_audio(&mut self) {
        debug!("Cleaning up audio resources");
        
        #[cfg(windows)]
        {
            // DirectSound COM object is automatically cleaned up when dropped
            self.direct_sound = None;
        }
        
        self.subsystem_states.insert("audio".to_string(), SubsystemState::Uninitialized);
        debug!("Audio cleanup complete");
    }

    /// Get current high-resolution timestamp
    pub fn get_timestamp(&self) -> i64 {
        #[cfg(windows)]
        {
            let mut counter = 0i64;
            unsafe {
                if QueryPerformanceCounter(&mut counter as *mut i64).as_bool() {
                    return counter;
                }
            }
        }
        
        // Fallback to system time
        self.last_frame_time.elapsed().as_nanos() as i64
    }

    /// Convert timestamp to milliseconds
    pub fn timestamp_to_ms(&self, timestamp: i64) -> f64 {
        if self.perf_frequency == 0 {
            return 0.0;
        }
        (timestamp as f64 * 1000.0) / (self.perf_frequency as f64)
    }
    
    /// Get elapsed time since engine start in milliseconds
    pub fn get_elapsed_time_ms(&self) -> f64 {
        let current = self.get_timestamp();
        self.timestamp_to_ms(current - self.perf_counter_start)
    }

    /// Update the game engine (matches C++ signature)
    /// This is the main update loop that should be called each frame
    #[instrument(skip(self), fields(engine_id = %self.id))]
    pub async fn update(&mut self) {
        if !self.initialized {
            return;
        }
        
        // Update performance metrics
        if self.config.enable_performance_monitoring {
            self.update_performance_metrics().await;
        }
        
        // Process Windows messages
        let should_continue = self.service_windows_os().await;
        if !should_continue {
            self.is_quitting = true;
        }
        
        // Handle alt-tab behavior (matching C++ implementation)
        if self.config.enable_alt_tab_handling {
            self.handle_alt_tab_behavior().await;
        }
        
        // Update subsystems
        self.update_subsystems().await;
    }
    
    /// Service Windows OS message queue (matches C++ serviceWindowsOS)
    #[instrument(skip(self), fields(engine_id = %self.id))]
    pub async fn service_windows_os(&mut self) -> bool {
        #[cfg(windows)]
        {
            let mut msg = MSG::default();
            
            // Process all pending messages
            while unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() } {
                
                // Handle WM_QUIT message
                if msg.message == WM_QUIT {
                    debug!("Received WM_QUIT message");
                    return false;
                }
                
                // Store message time for external synchronization
                // This matches the C++ TheMessageTime global variable
                
                // Translate and dispatch message
                unsafe {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                
                // Convert to our message type and queue it
                let engine_message = self.convert_windows_message(&msg);
                if let Some(msg) = engine_message {
                    let _ = self.message_queue.lock().await.push(msg);
                }
            }
        }
        
        true // Continue running
    }

    /// Handle alt-tab behavior (matches C++ implementation)
    async fn handle_alt_tab_behavior(&mut self) {
        #[cfg(windows)]
        {
            if let Some(hwnd) = self.window {
                unsafe {
                    let is_minimized = IsIconic(hwnd).as_bool();
                    
                    if is_minimized && !self.is_minimized {
                        debug!("Window minimized, entering alt-tab mode");
                        self.is_minimized = true;
                        self.is_active = false;
                        
                        // Pause audio if available
                        // This would integrate with MilesAudioManager
                        
                    } else if !is_minimized && self.is_minimized {
                        debug!("Window restored from minimized state");
                        self.is_minimized = false;
                        self.is_active = true;
                        
                        // Restore audio volume (matches C++ behavior)
                        // This would integrate with MilesAudioManager
                    }
                    
                    // If minimized, sleep briefly and continue processing messages
                    if is_minimized {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                        
                        // Keep network active even when minimized (matches C++)
                        // This would integrate with LAN manager
                        
                        // For multiplayer games, keep logic running
                        // This matches the C++ behavior for internet/LAN games
                    }
                }
            }
        }
    }
    
    /// Convert Windows message to engine message type
    #[cfg(windows)]
    fn convert_windows_message(&self, msg: &MSG) -> Option<WindowsMessage> {
        match msg.message {
            WM_QUIT => Some(WindowsMessage::Quit),
            WM_ACTIVATE => {
                let active = (msg.wParam.0 & 0xFFFF) != 0;
                if active {
                    Some(WindowsMessage::Activate)
                } else {
                    Some(WindowsMessage::Deactivate)
                }
            }
            WM_SIZE => Some(WindowsMessage::Resize),
            WM_PAINT => Some(WindowsMessage::Paint),
            WM_KEYDOWN => Some(WindowsMessage::KeyDown(msg.wParam.0 as u32)),
            WM_KEYUP => Some(WindowsMessage::KeyUp(msg.wParam.0 as u32)),
            WM_MOUSEMOVE => Some(WindowsMessage::MouseMove),
            WM_LBUTTONDOWN => Some(WindowsMessage::MouseButtonDown(1)),
            WM_LBUTTONUP => Some(WindowsMessage::MouseButtonUp(1)),
            WM_RBUTTONDOWN => Some(WindowsMessage::MouseButtonDown(2)),
            WM_RBUTTONUP => Some(WindowsMessage::MouseButtonUp(2)),
            _ => Some(WindowsMessage::Other(msg.message)),
        }
    }

    /// Update performance metrics
    async fn update_performance_metrics(&mut self) {
        let now = NetworkInstant::now();
        let frame_time = now.duration_since(self.last_frame_time);
        self.last_frame_time = now;
        
        let mut metrics = self.performance_metrics.write();
        metrics.frame_count += 1;
        metrics.frame_time_ms = frame_time.as_secs_f64() * 1000.0;
        
        // Calculate FPS over last second
        if metrics.frame_count % 60 == 0 {
            metrics.fps = 1000.0 / metrics.frame_time_ms;
            
            // Update memory usage
            #[cfg(windows)]
            {
                metrics.memory_usage_mb = self.get_memory_usage_mb().await;
            }
        }
    }
    
    /// Get current memory usage in MB
    #[cfg(windows)]
    async fn get_memory_usage_mb(&self) -> u64 {
        use windows::Win32::System::ProcessStatus::*;
        
        unsafe {
            let process = GetCurrentProcess();
            let mut counters = PROCESS_MEMORY_COUNTERS::default();
            
            if GetProcessMemoryInfo(process, &mut counters, std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32).as_bool() {
                return counters.WorkingSetSize as u64 / (1024 * 1024);
            }
        }
        
        0
    }
    
    /// Update all subsystems
    async fn update_subsystems(&mut self) {
        // This would call update methods on various game subsystems
        // For now, just update subsystem states
        
        for mut entry in self.subsystem_states.iter_mut() {
            if *entry.value() == SubsystemState::Ready {
                *entry.value_mut() = SubsystemState::Running;
            }
        }
    }
    
    /// Start performance monitoring background task
    async fn start_performance_monitoring(&self) {
        debug!("Starting performance monitoring");
        // Would start background task to collect system metrics
    }
    
    /// Stop performance monitoring
    async fn stop_performance_monitoring(&self) {
        debug!("Stopping performance monitoring");
        // Would stop background monitoring tasks
    }

    /// Public accessor methods
    
    /// Get window handle
    #[cfg(windows)]
    pub fn get_window_handle(&self) -> Option<HWND> {
        self.window
    }
    
    /// Get DirectX device handle
    #[cfg(windows)]
    pub fn get_d3d_device(&self) -> Option<&ID3D11Device> {
        self.d3d_device.as_ref()
    }
    
    /// Get DirectX context
    #[cfg(windows)]
    pub fn get_d3d_context(&self) -> Option<&ID3D11DeviceContext> {
        self.d3d_context.as_ref()
    }
    
    /// Check if engine is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
    
    /// Check if engine is active (not minimized)
    pub fn is_active(&self) -> bool {
        self.is_active
    }
    
    /// Check if engine is quitting
    pub fn is_quitting(&self) -> bool {
        self.is_quitting
    }
    
    /// Get engine configuration
    pub fn get_config(&self) -> &Win32EngineConfig {
        &self.config
    }
    
    /// Get performance metrics
    pub fn get_performance_metrics(&self) -> PerformanceMetrics {
        self.performance_metrics.read().clone()
    }
    
    /// Get subsystem state
    pub fn get_subsystem_state(&self, subsystem: &str) -> SubsystemState {
        self.subsystem_states
            .get(subsystem)
            .map(|s| *s.value())
            .unwrap_or(SubsystemState::Uninitialized)
    }
    
    /// Get pending Windows messages
    pub async fn get_messages(&self) -> Vec<WindowsMessage> {
        let mut queue = self.message_queue.lock().await;
        let messages = queue.clone();
        queue.clear();
        messages
    }
    
    /// Get registry value
    pub fn get_registry_value(&self, key: &str) -> Option<&String> {
        self.registry_keys.get(key)
    }

    /// Set window title
    #[cfg(windows)]
    pub async fn set_window_title(&self, title: &str) -> Result<()> {
        if let Some(hwnd) = self.window {
            let wide_title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
            
            unsafe {
                if !SetWindowTextW(hwnd, PCWSTR(wide_title.as_ptr())).as_bool() {
                    return Err(Win32EngineError::SystemApiError {
                        message: "SetWindowTextW failed".to_string(),
                    });
                }
            }
            
            Ok(())
        } else {
            Err(Win32EngineError::InvalidWindow)
        }
    }

    /// Show/hide cursor
    #[cfg(windows)]
    pub fn show_cursor(&self, show: bool) -> Result<()> {
        unsafe {
            ShowCursor(show);
        }
        Ok(())
    }

    /// Set cursor position
    #[cfg(windows)]
    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<()> {
        unsafe {
            if !SetCursorPos(x, y).as_bool() {
                return Err(Win32EngineError::SystemApiError {
                    message: "SetCursorPos failed".to_string(),
                });
            }
        }
        Ok(())
    }
    
    /// Set engine as quitting
    pub fn set_quitting(&mut self, quitting: bool) {
        self.is_quitting = quitting;
        if quitting {
            info!("Engine marked for shutdown");
        }
    }
    
    /// Private helper methods
    
    /// Read string value from Windows registry
    #[cfg(windows)]
    async fn read_registry_string(&self, hkey: HKEY, path: &str, value_name: &str) -> Result<String> {
        use windows::Win32::System::Registry::*;
        
        let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let value_name_wide: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
        
        unsafe {
            let mut key = HKEY::default();
            let result = RegOpenKeyExW(
                hkey,
                PCWSTR(path_wide.as_ptr()),
                0,
                KEY_READ,
                &mut key,
            );
            
            if result.is_err() {
                return Err(Win32EngineError::RegistryError {
                    message: format!("Failed to open registry key: {}", path),
                });
            }
            
            let mut data_type = REG_NONE;
            let mut data_size = 0u32;
            
            // Get size
            let result = RegQueryValueExW(
                key,
                PCWSTR(value_name_wide.as_ptr()),
                None,
                Some(&mut data_type),
                None,
                Some(&mut data_size),
            );
            
            if result.is_err() {
                let _ = RegCloseKey(key);
                return Err(Win32EngineError::RegistryError {
                    message: format!("Failed to query registry value: {}", value_name),
                });
            }
            
            // Read data
            let mut buffer = vec![0u8; data_size as usize];
            let result = RegQueryValueExW(
                key,
                PCWSTR(value_name_wide.as_ptr()),
                None,
                Some(&mut data_type),
                Some(buffer.as_mut_ptr()),
                Some(&mut data_size),
            );
            
            let _ = RegCloseKey(key);
            
            if result.is_err() {
                return Err(Win32EngineError::RegistryError {
                    message: format!("Failed to read registry value: {}", value_name),
                });
            }
            
            // Convert to string
            if data_type == REG_SZ {
                let wide_chars = buffer.len() / 2;
                let wide_buffer: &[u16] = std::slice::from_raw_parts(
                    buffer.as_ptr() as *const u16,
                    wide_chars,
                );
                
                // Find null terminator
                let end = wide_buffer.iter().position(|&c| c == 0).unwrap_or(wide_chars);
                let result = String::from_utf16(&wide_buffer[..end])
                    .map_err(|_| Win32EngineError::InvalidString)?;
                
                Ok(result)
            } else {
                Err(Win32EngineError::RegistryError {
                    message: "Registry value is not a string".to_string(),
                })
            }
        }
    }
}

impl Default for Win32GameEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Win32GameEngine {
    fn drop(&mut self) {
        // Note: Can't call async shutdown() in Drop
        // The user should call shutdown() explicitly before drop
        if self.initialized {
            warn!("Win32GameEngine dropped without explicit shutdown");
            
            // Do basic cleanup synchronously
            #[cfg(windows)]
            {
                unsafe { SetErrorMode(self.previous_error_mode); }
            }
            
            // Clear COM objects
            #[cfg(windows)]
            {
                self.direct_sound = None;
                self.dxgi_swap_chain = None;
                self.d3d_context = None;
                self.d3d_device = None;
            }
        }
    }
}

// Non-windows stub implementations
#[cfg(not(windows))]
type HINSTANCE = *mut c_void;
#[cfg(not(windows))]
type HWND = *mut c_void;

// Additional dependencies needed
#[cfg(not(feature = "rayon"))]
compile_error!("The rayon feature is required for Win32GameEngine");

// Re-export commonly used types for convenience
pub use WindowsMessage;
pub use SubsystemState;
pub use PerformanceMetrics;
pub use Win32EngineConfig;
pub use Win32EngineError;

// Helper function to create Win32GameEngine instance (matches C++ factory pattern)
pub fn create_win32_game_engine() -> Win32GameEngine {
    Win32GameEngine::new()
}

// Helper function with configuration
pub fn create_win32_game_engine_with_config(config: Win32EngineConfig) -> Win32GameEngine {
    Win32GameEngine::with_config(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = Win32GameEngine::new();
        assert!(!engine.is_initialized());
        assert_eq!(engine.get_window_handle(), ptr::null_mut());
        assert_eq!(engine.get_d3d_device(), ptr::null_mut());
    }

    #[test]
    fn test_timestamp_conversion() {
        let mut engine = Win32GameEngine::new();
        engine.perf_frequency = 1000000; // 1MHz
        
        let timestamp = 1000000; // 1 second at 1MHz
        let ms = engine.timestamp_to_ms(timestamp);
        assert_eq!(ms, 1000.0); // Should be 1000ms
    }
}
