//! # Video Device Layer
//!
//! This module provides the complete video device abstraction layer, supporting:
//! - Multiple graphics APIs (Vulkan, DirectX 12, Metal, OpenGL)
//! - Display adapter enumeration and management
//! - Render device abstraction
//! - Window and surface management

pub mod cpp_bindings;
pub mod display_adapter;
pub mod multi_monitor;
pub mod performance_monitor;
pub mod render_device;
pub mod texture_manager;
pub mod video_device;

// Re-exports
pub use cpp_bindings::{CVertex, CVideoDevice, CVideoResult, CVideoStatistics};
pub use display_adapter::{
    AdapterCapabilities, AdapterDeviceType, AdapterFeatures, BackendType, ColorSpace,
    DisplayAdapter, DisplayInfo, DisplayOrientation, HdrSupport,
};
pub use multi_monitor::{
    ColorCapabilities, ConnectionType, DisplayConfiguration, HdrCapabilities, HdrMode, Monitor,
    MonitorConfig, MonitorOrientation, MultiMonitorManager, VideoModeInfo,
};
pub use performance_monitor::{
    EventType, GpuPerformanceMetrics, MemoryAllocation, MemoryStatistics, MemoryType,
    PerformanceEvent, PerformanceMonitor, PerformanceMonitorConfig, ScopedTimer,
};
pub use render_device::{
    BufferDesc, BufferMemoryLocation, BufferUsageFlags, CameraUniform, GraphicsApi, LightUniform,
    MaterialUniform, ModelUniform, RenderCapabilities, RenderContext, RenderDevice,
    RenderStatistics, RenderTarget, RenderTargetUsage, ShaderDesc, ShaderType, TextureDesc,
    TextureUsage, Vertex,
};
pub use texture_manager::{
    CachedTexture, StreamingPriority, TextureCompression, TextureFilter, TextureManager,
    TextureManagerConfig, TextureManagerStats, TextureMetadata, TextureWrap,
};
pub use video_device::{VideoDevice, VideoDeviceConfig, VideoStatistics};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "video")]
pub use wgpu::PowerPreference;

#[cfg(not(feature = "video"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerPreference {
    LowPower,
    HighPerformance,
}

#[cfg(not(feature = "video"))]
impl Default for PowerPreference {
    fn default() -> Self {
        Self::HighPerformance
    }
}

/// Video device error types
#[derive(Error, Debug)]
pub enum VideoDeviceError {
    /// Device initialization failed
    #[error("Video device initialization failed: {0}")]
    InitializationFailed(String),

    /// Display adapter not found
    #[error("Display adapter not found: {0}")]
    AdapterNotFound(String),

    /// Graphics API not supported
    #[error("Graphics API not supported: {0}")]
    ApiNotSupported(String),

    /// Surface creation failed
    #[error("Surface creation failed: {0}")]
    SurfaceCreationFailed(String),

    /// Render context error
    #[error("Render context error: {0}")]
    RenderContextError(String),

    /// Resource error
    #[error("Resource error: {0}")]
    ResourceError(String),

    /// Window system error
    #[error("Window system error: {0}")]
    WindowSystemError(String),

    /// Platform-specific error
    #[error("Platform error: {0}")]
    PlatformError(String),
}

/// Result type for video operations
pub type Result<T> = std::result::Result<T, VideoDeviceError>;

/// Video resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels  
    pub height: u32,
}

impl Resolution {
    /// Create a new resolution
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Common resolutions
    pub const fn hd_720p() -> Self {
        Self::new(1280, 720)
    }
    pub const fn hd_1080p() -> Self {
        Self::new(1920, 1080)
    }
    pub const fn uhd_4k() -> Self {
        Self::new(3840, 2160)
    }
    pub const fn uhd_8k() -> Self {
        Self::new(7680, 4320)
    }

    /// Get aspect ratio
    pub fn aspect_ratio(self) -> f32 {
        self.width as f32 / self.height as f32
    }

    /// Get total pixel count
    pub fn pixel_count(self) -> u32 {
        self.width * self.height
    }
}

impl Default for Resolution {
    fn default() -> Self {
        Self::hd_1080p()
    }
}

impl std::fmt::Display for Resolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// Display refresh rate
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RefreshRate {
    /// Rate in Hz
    pub hz: f32,
}

impl RefreshRate {
    /// Create a new refresh rate
    pub const fn new(hz: f32) -> Self {
        Self { hz }
    }

    /// Common refresh rates
    pub const fn rate_60hz() -> Self {
        Self::new(60.0)
    }
    pub const fn rate_120hz() -> Self {
        Self::new(120.0)
    }
    pub const fn rate_144hz() -> Self {
        Self::new(144.0)
    }
    pub const fn rate_240hz() -> Self {
        Self::new(240.0)
    }
}

impl Default for RefreshRate {
    fn default() -> Self {
        Self::rate_60hz()
    }
}

impl std::fmt::Display for RefreshRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.1}Hz", self.hz)
    }
}

/// Display mode combining resolution and refresh rate
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DisplayMode {
    /// Resolution
    pub resolution: Resolution,
    /// Refresh rate
    pub refresh_rate: RefreshRate,
    /// Color depth in bits
    pub color_depth: u8,
}

impl DisplayMode {
    /// Create a new display mode
    pub const fn new(resolution: Resolution, refresh_rate: RefreshRate, color_depth: u8) -> Self {
        Self {
            resolution,
            refresh_rate,
            color_depth,
        }
    }
}

impl Default for DisplayMode {
    fn default() -> Self {
        Self::new(Resolution::default(), RefreshRate::default(), 32)
    }
}

/// Color format for rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorFormat {
    /// 8-bit RGBA
    Rgba8,
    /// 8-bit BGRA  
    Bgra8,
    /// 16-bit RGBA
    Rgba16,
    /// 32-bit RGBA floating point
    Rgba32Float,
    /// 10-bit RGB with 2-bit alpha
    Rgb10A2,
    /// HDR formats
    Hdr10,
    /// Depth formats
    Depth24Stencil8,
    Depth32Float,
}

impl Default for ColorFormat {
    fn default() -> Self {
        Self::Rgba8
    }
}

/// Multi-sampling anti-aliasing settings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsaaSettings {
    /// Sample count (1 = disabled, 2/4/8/16 = enabled)
    pub sample_count: u32,
    /// Quality level (driver-dependent)
    pub quality: u32,
}

impl MsaaSettings {
    /// No anti-aliasing
    pub const fn none() -> Self {
        Self {
            sample_count: 1,
            quality: 0,
        }
    }

    /// 2x MSAA
    pub const fn msaa_2x() -> Self {
        Self {
            sample_count: 2,
            quality: 0,
        }
    }

    /// 4x MSAA
    pub const fn msaa_4x() -> Self {
        Self {
            sample_count: 4,
            quality: 0,
        }
    }

    /// 8x MSAA
    pub const fn msaa_8x() -> Self {
        Self {
            sample_count: 8,
            quality: 0,
        }
    }

    /// Check if MSAA is enabled
    pub const fn is_enabled(self) -> bool {
        self.sample_count > 1
    }
}

impl Default for MsaaSettings {
    fn default() -> Self {
        Self::msaa_4x()
    }
}

/// VSync (vertical synchronization) settings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VSync {
    /// VSync disabled - no frame rate limit
    Disabled,
    /// VSync enabled - limit to display refresh rate
    Enabled,
    /// Adaptive VSync - enable when above refresh rate, disable when below
    Adaptive,
    /// Fast VSync - half refresh rate when below, full when above
    Fast,
}

impl Default for VSync {
    fn default() -> Self {
        Self::Enabled
    }
}
