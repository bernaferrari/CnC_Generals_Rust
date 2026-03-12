//! # Multi-Monitor Support
//!
//! Advanced multi-monitor management with resolution switching and display configuration.

use super::{ColorFormat, DisplayMode, RefreshRate, Resolution, Result, VideoDeviceError};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "video")]
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    monitor::{MonitorHandle, VideoMode},
    window::{Fullscreen, Window},
};

/// Monitor information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Monitor {
    /// Unique monitor ID
    pub id: String,
    /// Monitor name (from system)
    pub name: String,
    /// Physical position relative to primary monitor
    pub position: (i32, i32),
    /// Physical size in millimeters
    pub physical_size_mm: (u32, u32),
    /// Scale factor for high DPI displays
    pub scale_factor: f64,
    /// Native resolution
    pub native_resolution: Resolution,
    /// Current resolution
    pub current_resolution: Resolution,
    /// Is this the primary monitor
    pub is_primary: bool,
    /// Available video modes
    pub video_modes: Vec<VideoModeInfo>,
    /// Color space capabilities
    pub color_capabilities: ColorCapabilities,
    /// HDR capabilities
    pub hdr_capabilities: HdrCapabilities,
    /// Connection type (if detectable)
    pub connection_type: ConnectionType,
    /// Monitor manufacturer
    pub manufacturer: Option<String>,
    /// Monitor model
    pub model: Option<String>,
    /// Year of manufacture (if available)
    pub year: Option<u16>,
}

/// Video mode information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoModeInfo {
    /// Resolution
    pub resolution: Resolution,
    /// Refresh rate
    pub refresh_rate: RefreshRate,
    /// Color depth in bits
    pub bit_depth: u8,
    /// Whether this mode is supported
    pub is_supported: bool,
    /// Whether this is the preferred mode
    pub is_preferred: bool,
}

/// Color capabilities of a monitor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorCapabilities {
    /// Supported color formats
    pub supported_formats: Vec<ColorFormat>,
    /// Color gamut coverage
    pub srgb_coverage: f32,
    pub adobe_rgb_coverage: f32,
    pub dci_p3_coverage: f32,
    pub rec2020_coverage: f32,
    /// Supports wide color gamut
    pub wide_color_gamut: bool,
    /// Maximum bits per color channel
    pub max_bits_per_channel: u8,
}

/// HDR capabilities of a monitor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HdrCapabilities {
    /// HDR10 support
    pub hdr10: bool,
    /// HDR10+ support
    pub hdr10_plus: bool,
    /// Dolby Vision support
    pub dolby_vision: bool,
    /// Maximum luminance in nits
    pub max_luminance: f32,
    /// Minimum luminance in nits
    pub min_luminance: f32,
    /// Maximum content light level
    pub max_cll: f32,
    /// Maximum frame-average light level
    pub max_fall: f32,
}

/// Monitor connection type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionType {
    /// HDMI connection
    HDMI,
    /// DisplayPort connection
    DisplayPort,
    /// DVI connection
    DVI,
    /// VGA connection (analog)
    VGA,
    /// USB-C/Thunderbolt
    USBC,
    /// Built-in display (laptop screen)
    Builtin,
    /// Wireless display
    Wireless,
    /// Unknown connection type
    Unknown,
}

/// Display configuration for multi-monitor setups
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfiguration {
    /// Configuration name
    pub name: String,
    /// Primary monitor ID
    pub primary_monitor_id: String,
    /// Monitor configurations
    pub monitor_configs: HashMap<String, MonitorConfig>,
    /// Global settings
    pub global_settings: GlobalDisplaySettings,
}

/// Configuration for individual monitor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    /// Monitor ID
    pub monitor_id: String,
    /// Display mode to use
    pub display_mode: DisplayMode,
    /// Position relative to primary monitor
    pub position: (i32, i32),
    /// Orientation
    pub orientation: MonitorOrientation,
    /// Whether this monitor is enabled
    pub enabled: bool,
    /// Color profile to use
    pub color_profile: Option<String>,
    /// HDR mode
    pub hdr_mode: HdrMode,
}

/// Monitor orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitorOrientation {
    /// Landscape (normal)
    Landscape,
    /// Portrait (90° clockwise)
    Portrait,
    /// Landscape flipped (180°)
    LandscapeFlipped,
    /// Portrait flipped (270° clockwise)
    PortraitFlipped,
}

/// HDR mode setting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HdrMode {
    /// HDR disabled
    Off,
    /// HDR10 enabled
    Hdr10,
    /// HDR10+ enabled (if supported)
    Hdr10Plus,
    /// Dolby Vision enabled (if supported)
    DolbyVision,
    /// Auto HDR (system managed)
    Auto,
}

/// Global display settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalDisplaySettings {
    /// Global DPI scaling mode
    pub dpi_scaling: DpiScaling,
    /// Night light/blue light filter
    pub night_light: NightLightSettings,
    /// Power management
    pub power_management: PowerManagementSettings,
}

/// DPI scaling mode
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DpiScaling {
    /// No scaling (100%)
    None,
    /// System automatic scaling
    Auto,
    /// Custom scaling factor
    Custom(f64),
    /// Per-monitor DPI awareness
    PerMonitor,
}

/// Night light filter settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NightLightSettings {
    /// Enable night light
    pub enabled: bool,
    /// Color temperature in Kelvin
    pub temperature: u32,
    /// Schedule (time-based or location-based)
    pub schedule: NightLightSchedule,
}

/// Night light schedule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NightLightSchedule {
    /// Manual on/off
    Manual,
    /// Sunset to sunrise (location-based)
    SunsetToSunrise,
    /// Custom time schedule
    Custom {
        start_hour: u8,
        start_minute: u8,
        end_hour: u8,
        end_minute: u8,
    },
}

/// Power management settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerManagementSettings {
    /// Turn off display after (minutes, 0 = never)
    pub display_timeout: u32,
    /// Enable adaptive brightness
    pub adaptive_brightness: bool,
    /// Minimum brightness percentage
    pub min_brightness: f32,
    /// Maximum brightness percentage
    pub max_brightness: f32,
}

/// Multi-monitor manager
pub struct MultiMonitorManager {
    /// List of detected monitors
    monitors: Arc<RwLock<HashMap<String, Monitor>>>,

    /// Current display configuration
    current_configuration: Arc<RwLock<DisplayConfiguration>>,

    /// Saved configurations
    saved_configurations: Arc<RwLock<HashMap<String, DisplayConfiguration>>>,

    /// Primary monitor ID
    primary_monitor_id: Arc<RwLock<String>>,

    /// Monitor change callbacks
    callbacks: Arc<RwLock<Vec<Box<dyn Fn(&[Monitor]) + Send + Sync>>>>,
}

impl MultiMonitorManager {
    /// Create a new multi-monitor manager
    pub fn new() -> Result<Self> {
        let manager = Self {
            monitors: Arc::new(RwLock::new(HashMap::new())),
            current_configuration: Arc::new(RwLock::new(Self::default_configuration())),
            saved_configurations: Arc::new(RwLock::new(HashMap::new())),
            primary_monitor_id: Arc::new(RwLock::new(String::new())),
            callbacks: Arc::new(RwLock::new(Vec::new())),
        };

        // Initial monitor detection
        manager.detect_monitors()?;

        Ok(manager)
    }

    /// Detect all connected monitors
    pub fn detect_monitors(&self) -> Result<()> {
        #[cfg(feature = "video")]
        {
            // Get all available monitors from winit
            // Note: This requires an event loop context in a real implementation
            // For now, we'll create mock monitors

            let mut monitors = HashMap::new();

            // Create primary monitor (mock data)
            let primary_monitor = Monitor {
                id: "monitor_0".to_string(),
                name: "Primary Display".to_string(),
                position: (0, 0),
                physical_size_mm: (600, 340),
                scale_factor: 1.0,
                native_resolution: Resolution::new(2560, 1440),
                current_resolution: Resolution::new(2560, 1440),
                is_primary: true,
                video_modes: vec![
                    VideoModeInfo {
                        resolution: Resolution::new(2560, 1440),
                        refresh_rate: RefreshRate::new(60.0),
                        bit_depth: 32,
                        is_supported: true,
                        is_preferred: true,
                    },
                    VideoModeInfo {
                        resolution: Resolution::new(2560, 1440),
                        refresh_rate: RefreshRate::new(144.0),
                        bit_depth: 32,
                        is_supported: true,
                        is_preferred: false,
                    },
                    VideoModeInfo {
                        resolution: Resolution::new(1920, 1080),
                        refresh_rate: RefreshRate::new(60.0),
                        bit_depth: 32,
                        is_supported: true,
                        is_preferred: false,
                    },
                ],
                color_capabilities: ColorCapabilities {
                    supported_formats: vec![
                        ColorFormat::Rgba8,
                        ColorFormat::Rgba16,
                        ColorFormat::Hdr10,
                    ],
                    srgb_coverage: 100.0,
                    adobe_rgb_coverage: 85.0,
                    dci_p3_coverage: 95.0,
                    rec2020_coverage: 72.0,
                    wide_color_gamut: true,
                    max_bits_per_channel: 10,
                },
                hdr_capabilities: HdrCapabilities {
                    hdr10: true,
                    hdr10_plus: false,
                    dolby_vision: false,
                    max_luminance: 1000.0,
                    min_luminance: 0.05,
                    max_cll: 1000.0,
                    max_fall: 400.0,
                },
                connection_type: ConnectionType::DisplayPort,
                manufacturer: Some("Generic".to_string()),
                model: Some("Gaming Monitor".to_string()),
                year: Some(2023),
            };

            monitors.insert("monitor_0".to_string(), primary_monitor);

            // Add secondary monitor if available (mock)
            let secondary_monitor = Monitor {
                id: "monitor_1".to_string(),
                name: "Secondary Display".to_string(),
                position: (2560, 0), // To the right of primary
                physical_size_mm: (480, 270),
                scale_factor: 1.0,
                native_resolution: Resolution::new(1920, 1080),
                current_resolution: Resolution::new(1920, 1080),
                is_primary: false,
                video_modes: vec![
                    VideoModeInfo {
                        resolution: Resolution::new(1920, 1080),
                        refresh_rate: RefreshRate::new(60.0),
                        bit_depth: 24,
                        is_supported: true,
                        is_preferred: true,
                    },
                    VideoModeInfo {
                        resolution: Resolution::new(1680, 1050),
                        refresh_rate: RefreshRate::new(60.0),
                        bit_depth: 24,
                        is_supported: true,
                        is_preferred: false,
                    },
                ],
                color_capabilities: ColorCapabilities {
                    supported_formats: vec![ColorFormat::Rgba8],
                    srgb_coverage: 99.0,
                    adobe_rgb_coverage: 72.0,
                    dci_p3_coverage: 75.0,
                    rec2020_coverage: 35.0,
                    wide_color_gamut: false,
                    max_bits_per_channel: 8,
                },
                hdr_capabilities: HdrCapabilities {
                    hdr10: false,
                    hdr10_plus: false,
                    dolby_vision: false,
                    max_luminance: 250.0,
                    min_luminance: 0.3,
                    max_cll: 250.0,
                    max_fall: 100.0,
                },
                connection_type: ConnectionType::HDMI,
                manufacturer: Some("Generic".to_string()),
                model: Some("Office Monitor".to_string()),
                year: Some(2021),
            };

            monitors.insert("monitor_1".to_string(), secondary_monitor);

            // Update stored monitors
            *self.monitors.write() = monitors;
            *self.primary_monitor_id.write() = "monitor_0".to_string();

            // Notify callbacks
            self.notify_monitor_change();
        }

        tracing::info!("Detected {} monitors", self.monitors.read().len());
        Ok(())
    }

    /// Get all detected monitors
    pub fn get_monitors(&self) -> Vec<Monitor> {
        self.monitors.read().values().cloned().collect()
    }

    /// Get monitor by ID
    pub fn get_monitor(&self, id: &str) -> Option<Monitor> {
        self.monitors.read().get(id).cloned()
    }

    /// Get primary monitor
    pub fn get_primary_monitor(&self) -> Option<Monitor> {
        let primary_id = self.primary_monitor_id.read().clone();
        self.get_monitor(&primary_id)
    }

    /// Set primary monitor
    pub fn set_primary_monitor(&self, monitor_id: &str) -> Result<()> {
        if self.monitors.read().contains_key(monitor_id) {
            *self.primary_monitor_id.write() = monitor_id.to_string();

            // Update monitors to reflect new primary status
            let mut monitors = self.monitors.write();
            for (id, monitor) in monitors.iter_mut() {
                monitor.is_primary = id == monitor_id;
            }

            tracing::info!("Primary monitor set to: {}", monitor_id);
            Ok(())
        } else {
            Err(VideoDeviceError::ResourceError(format!(
                "Monitor not found: {}",
                monitor_id
            )))
        }
    }

    /// Change monitor resolution
    pub fn change_monitor_resolution(
        &self,
        monitor_id: &str,
        resolution: Resolution,
        refresh_rate: RefreshRate,
    ) -> Result<()> {
        let mut monitors = self.monitors.write();

        if let Some(monitor) = monitors.get_mut(monitor_id) {
            // Check if the mode is supported
            let mode_supported = monitor.video_modes.iter().any(|mode| {
                mode.resolution == resolution
                    && mode.refresh_rate.hz == refresh_rate.hz
                    && mode.is_supported
            });

            if mode_supported {
                monitor.current_resolution = resolution;
                tracing::info!(
                    "Changed resolution for monitor {}: {}x{} @ {:.1}Hz",
                    monitor_id,
                    resolution.width,
                    resolution.height,
                    refresh_rate.hz
                );
                Ok(())
            } else {
                Err(VideoDeviceError::ApiNotSupported(format!(
                    "Resolution {}x{} @ {:.1}Hz not supported on monitor {}",
                    resolution.width, resolution.height, refresh_rate.hz, monitor_id
                )))
            }
        } else {
            Err(VideoDeviceError::ResourceError(format!(
                "Monitor not found: {}",
                monitor_id
            )))
        }
    }

    /// Apply display configuration
    pub fn apply_configuration(&self, config: &DisplayConfiguration) -> Result<()> {
        // Validate configuration
        for (monitor_id, monitor_config) in &config.monitor_configs {
            if !self.monitors.read().contains_key(monitor_id) {
                return Err(VideoDeviceError::ResourceError(format!(
                    "Monitor not found in configuration: {}",
                    monitor_id
                )));
            }

            if monitor_config.enabled {
                self.change_monitor_resolution(
                    monitor_id,
                    monitor_config.display_mode.resolution,
                    monitor_config.display_mode.refresh_rate,
                )?;
            }
        }

        // Set primary monitor
        self.set_primary_monitor(&config.primary_monitor_id)?;

        // Update current configuration
        *self.current_configuration.write() = config.clone();

        tracing::info!("Applied display configuration: {}", config.name);
        Ok(())
    }

    /// Save current configuration
    pub fn save_configuration(&self, name: &str) -> Result<()> {
        let config = self.current_configuration.read().clone();
        self.saved_configurations
            .write()
            .insert(name.to_string(), config);

        tracing::info!("Saved display configuration: {}", name);
        Ok(())
    }

    /// Load saved configuration
    pub fn load_configuration(&self, name: &str) -> Result<()> {
        if let Some(config) = self.saved_configurations.read().get(name).cloned() {
            self.apply_configuration(&config)?;
            Ok(())
        } else {
            Err(VideoDeviceError::ResourceError(format!(
                "Configuration not found: {}",
                name
            )))
        }
    }

    /// Get available configurations
    pub fn get_saved_configurations(&self) -> Vec<String> {
        self.saved_configurations.read().keys().cloned().collect()
    }

    /// Register monitor change callback
    pub fn register_monitor_change_callback<F>(&self, callback: F)
    where
        F: Fn(&[Monitor]) + Send + Sync + 'static,
    {
        self.callbacks.write().push(Box::new(callback));
    }

    /// Get best configuration for current monitors
    pub fn get_recommended_configuration(&self) -> DisplayConfiguration {
        let monitors = self.monitors.read();
        let mut monitor_configs = HashMap::new();

        let primary_id = self.primary_monitor_id.read().clone();

        for (id, monitor) in monitors.iter() {
            let preferred_mode = monitor
                .video_modes
                .iter()
                .find(|mode| mode.is_preferred)
                .or_else(|| monitor.video_modes.first())
                .map(|mode| DisplayMode::new(mode.resolution, mode.refresh_rate, mode.bit_depth))
                .unwrap_or_default();

            let position = if monitor.is_primary {
                (0, 0)
            } else {
                monitor.position
            };

            let config = MonitorConfig {
                monitor_id: id.clone(),
                display_mode: preferred_mode,
                position,
                orientation: MonitorOrientation::Landscape,
                enabled: true,
                color_profile: None,
                hdr_mode: if monitor.hdr_capabilities.hdr10 {
                    HdrMode::Auto
                } else {
                    HdrMode::Off
                },
            };

            monitor_configs.insert(id.clone(), config);
        }

        DisplayConfiguration {
            name: "Recommended".to_string(),
            primary_monitor_id: primary_id,
            monitor_configs,
            global_settings: GlobalDisplaySettings {
                dpi_scaling: DpiScaling::Auto,
                night_light: NightLightSettings {
                    enabled: false,
                    temperature: 4000,
                    schedule: NightLightSchedule::Manual,
                },
                power_management: PowerManagementSettings {
                    display_timeout: 10, // 10 minutes
                    adaptive_brightness: true,
                    min_brightness: 20.0,
                    max_brightness: 100.0,
                },
            },
        }
    }

    /// Clone monitor for specific window
    pub fn clone_monitor_to_window(&self, monitor_id: &str, window: &Window) -> Result<()> {
        #[cfg(feature = "video")]
        {
            if let Some(monitor) = self.get_monitor(monitor_id) {
                // Set window fullscreen on specific monitor
                // This is a simplified implementation
                tracing::info!("Cloning display to monitor: {}", monitor_id);
                Ok(())
            } else {
                Err(VideoDeviceError::ResourceError(format!(
                    "Monitor not found: {}",
                    monitor_id
                )))
            }
        }

        #[cfg(not(feature = "video"))]
        {
            Err(VideoDeviceError::InitializationFailed(
                "Video feature not enabled".to_string(),
            ))
        }
    }

    // Private helper methods

    fn notify_monitor_change(&self) {
        let monitors: Vec<Monitor> = self.monitors.read().values().cloned().collect();
        let callbacks = self.callbacks.read();

        for callback in callbacks.iter() {
            callback(&monitors);
        }
    }

    fn default_configuration() -> DisplayConfiguration {
        DisplayConfiguration {
            name: "Default".to_string(),
            primary_monitor_id: String::new(),
            monitor_configs: HashMap::new(),
            global_settings: GlobalDisplaySettings {
                dpi_scaling: DpiScaling::Auto,
                night_light: NightLightSettings {
                    enabled: false,
                    temperature: 4000,
                    schedule: NightLightSchedule::Manual,
                },
                power_management: PowerManagementSettings {
                    display_timeout: 0, // Never
                    adaptive_brightness: false,
                    min_brightness: 0.0,
                    max_brightness: 100.0,
                },
            },
        }
    }
}

impl Default for ColorCapabilities {
    fn default() -> Self {
        Self {
            supported_formats: vec![ColorFormat::Rgba8],
            srgb_coverage: 100.0,
            adobe_rgb_coverage: 72.0,
            dci_p3_coverage: 75.0,
            rec2020_coverage: 35.0,
            wide_color_gamut: false,
            max_bits_per_channel: 8,
        }
    }
}

impl Default for HdrCapabilities {
    fn default() -> Self {
        Self {
            hdr10: false,
            hdr10_plus: false,
            dolby_vision: false,
            max_luminance: 100.0,
            min_luminance: 0.3,
            max_cll: 100.0,
            max_fall: 50.0,
        }
    }
}

impl std::fmt::Display for ConnectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionType::HDMI => write!(f, "HDMI"),
            ConnectionType::DisplayPort => write!(f, "DisplayPort"),
            ConnectionType::DVI => write!(f, "DVI"),
            ConnectionType::VGA => write!(f, "VGA"),
            ConnectionType::USBC => write!(f, "USB-C"),
            ConnectionType::Builtin => write!(f, "Built-in"),
            ConnectionType::Wireless => write!(f, "Wireless"),
            ConnectionType::Unknown => write!(f, "Unknown"),
        }
    }
}
