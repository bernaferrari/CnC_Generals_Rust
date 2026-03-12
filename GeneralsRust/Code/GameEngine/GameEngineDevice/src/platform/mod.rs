//! # Platform-Specific Device Interfaces
//!
//! This module provides platform-specific device implementations and abstractions,
//! enabling the GameEngineDevice to work seamlessly across different operating systems.

pub mod device_interface;

#[cfg(target_os = "windows")]
pub mod win32_device;

#[cfg(target_os = "linux")]
pub mod linux_device;

#[cfg(target_os = "macos")]
pub mod macos_device;

// Re-exports
pub use device_interface::{DeviceInterface, PlatformCapabilities, SystemInfo};

#[cfg(target_os = "windows")]
pub use win32_device::Win32Device;

#[cfg(target_os = "linux")]
pub use linux_device::LinuxDevice;

#[cfg(target_os = "macos")]
pub use macos_device::MacOsDevice;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Platform-specific errors
#[derive(Error, Debug)]
pub enum PlatformError {
    /// System call failed
    #[error("System call failed: {0}")]
    SystemCallFailed(String),

    /// Device not supported on this platform
    #[error("Device not supported on platform: {0}")]
    DeviceNotSupported(String),

    /// Platform API error
    #[error("Platform API error: {0}")]
    PlatformApiError(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Resource unavailable
    #[error("Resource unavailable: {0}")]
    ResourceUnavailable(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Driver error
    #[error("Driver error: {0}")]
    DriverError(String),
}

/// Result type for platform operations
pub type Result<T> = std::result::Result<T, PlatformError>;

/// Platform types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    /// Windows operating system
    Windows,
    /// Linux operating system
    Linux,
    /// macOS operating system
    MacOS,
    /// Other/Unknown platform
    Other,
}

impl Platform {
    /// Get the current platform
    pub fn current() -> Self {
        #[cfg(target_os = "windows")]
        return Self::Windows;

        #[cfg(target_os = "linux")]
        return Self::Linux;

        #[cfg(target_os = "macos")]
        return Self::MacOS;

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        Self::Other
    }

    /// Get platform name as string
    pub fn name(self) -> &'static str {
        match self {
            Self::Windows => "Windows",
            Self::Linux => "Linux",
            Self::MacOS => "macOS",
            Self::Other => "Other",
        }
    }

    /// Check if platform supports a specific feature
    pub fn supports_feature(self, feature: PlatformFeature) -> bool {
        match (self, feature) {
            (Self::Windows, PlatformFeature::DirectSound) => true,
            (Self::Windows, PlatformFeature::Wasapi) => true,
            (Self::Windows, PlatformFeature::DirectX) => true,
            (Self::Linux, PlatformFeature::Alsa) => true,
            (Self::Linux, PlatformFeature::PulseAudio) => true,
            (Self::Linux, PlatformFeature::Vulkan) => true,
            (Self::MacOS, PlatformFeature::CoreAudio) => true,
            (Self::MacOS, PlatformFeature::Metal) => true,
            (_, PlatformFeature::OpenGL) => true,
            (_, PlatformFeature::OpenAL) => true,
            _ => false,
        }
    }
}

/// Platform-specific features
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformFeature {
    // Audio features
    DirectSound,
    Wasapi,
    Alsa,
    PulseAudio,
    CoreAudio,
    OpenAL,

    // Graphics features
    DirectX,
    Vulkan,
    Metal,
    OpenGL,

    // System features
    FileMapping,
    SharedMemory,
    ThreadPriority,
    RealTimeScheduling,

    // Hardware features
    SimdInstructions,
    MultiCore,
    GpuAcceleration,
}

/// CPU architecture types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CpuArchitecture {
    /// x86-64 / AMD64
    X86_64,
    /// ARM64 / AArch64
    Aarch64,
    /// x86 32-bit
    X86,
    /// ARM 32-bit
    Arm,
    /// Unknown architecture
    Unknown,
}

impl CpuArchitecture {
    /// Get the current CPU architecture
    pub fn current() -> Self {
        #[cfg(target_arch = "x86_64")]
        return Self::X86_64;

        #[cfg(target_arch = "aarch64")]
        return Self::Aarch64;

        #[cfg(target_arch = "x86")]
        return Self::X86;

        #[cfg(target_arch = "arm")]
        return Self::Arm;

        #[cfg(not(any(
            target_arch = "x86_64",
            target_arch = "aarch64",
            target_arch = "x86",
            target_arch = "arm"
        )))]
        Self::Unknown
    }

    /// Get architecture name as string
    pub fn name(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
            Self::X86 => "x86",
            Self::Arm => "arm",
            Self::Unknown => "unknown",
        }
    }

    /// Check if architecture supports SIMD instructions
    pub fn supports_simd(self) -> bool {
        match self {
            Self::X86_64 | Self::X86 => true,  // SSE, AVX
            Self::Aarch64 | Self::Arm => true, // NEON
            Self::Unknown => false,
        }
    }
}

/// Hardware information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    /// CPU information
    pub cpu: CpuInfo,
    /// Memory information
    pub memory: MemoryInfo,
    /// GPU information
    pub gpu: Vec<GpuInfo>,
    /// Storage information
    pub storage: Vec<StorageInfo>,
}

/// CPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    /// CPU name
    pub name: String,
    /// CPU vendor
    pub vendor: String,
    /// CPU architecture
    pub architecture: CpuArchitecture,
    /// Number of physical cores
    pub physical_cores: u32,
    /// Number of logical cores
    pub logical_cores: u32,
    /// CPU frequency in MHz
    pub base_frequency_mhz: u32,
    /// Maximum frequency in MHz
    pub max_frequency_mhz: u32,
    /// L1 cache size in KB
    pub l1_cache_kb: u32,
    /// L2 cache size in KB
    pub l2_cache_kb: u32,
    /// L3 cache size in KB
    pub l3_cache_kb: u32,
    /// Supported instruction sets
    pub instruction_sets: Vec<String>,
}

/// Memory information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    /// Total physical memory in bytes
    pub total_physical: u64,
    /// Available physical memory in bytes
    pub available_physical: u64,
    /// Total virtual memory in bytes
    pub total_virtual: u64,
    /// Available virtual memory in bytes
    pub available_virtual: u64,
    /// Page size in bytes
    pub page_size: u32,
}

/// GPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// GPU name
    pub name: String,
    /// GPU vendor
    pub vendor: String,
    /// Device ID
    pub device_id: u32,
    /// Vendor ID
    pub vendor_id: u32,
    /// Dedicated video memory in bytes
    pub dedicated_memory: u64,
    /// Shared memory in bytes
    pub shared_memory: u64,
    /// Is primary GPU
    pub is_primary: bool,
}

/// Storage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    /// Device name
    pub name: String,
    /// Mount point or drive letter
    pub mount_point: String,
    /// Total capacity in bytes
    pub total_capacity: u64,
    /// Free space in bytes
    pub free_space: u64,
    /// File system type
    pub filesystem: String,
    /// Is SSD
    pub is_ssd: bool,
}

impl Default for HardwareInfo {
    fn default() -> Self {
        Self {
            cpu: CpuInfo {
                name: "Unknown CPU".to_string(),
                vendor: "Unknown".to_string(),
                architecture: CpuArchitecture::current(),
                physical_cores: 1,
                logical_cores: 1,
                base_frequency_mhz: 2000,
                max_frequency_mhz: 2000,
                l1_cache_kb: 32,
                l2_cache_kb: 256,
                l3_cache_kb: 2048,
                instruction_sets: vec!["x86".to_string()],
            },
            memory: MemoryInfo {
                total_physical: 8 * 1024 * 1024 * 1024,     // 8GB
                available_physical: 4 * 1024 * 1024 * 1024, // 4GB
                total_virtual: 16 * 1024 * 1024 * 1024,     // 16GB
                available_virtual: 8 * 1024 * 1024 * 1024,  // 8GB
                page_size: 4096,
            },
            gpu: vec![GpuInfo {
                name: "Unknown GPU".to_string(),
                vendor: "Unknown".to_string(),
                device_id: 0,
                vendor_id: 0,
                dedicated_memory: 1024 * 1024 * 1024, // 1GB
                shared_memory: 0,
                is_primary: true,
            }],
            storage: vec![StorageInfo {
                name: "Primary Storage".to_string(),
                mount_point: "/".to_string(),
                total_capacity: 500 * 1024 * 1024 * 1024, // 500GB
                free_space: 100 * 1024 * 1024 * 1024,     // 100GB
                filesystem: "ext4".to_string(),
                is_ssd: true,
            }],
        }
    }
}
