//! # Cross-Platform Device Interface
//!
//! Provides a unified interface for platform-specific device operations.

use super::{CpuArchitecture, HardwareInfo, Platform, PlatformError, PlatformFeature, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Platform capabilities information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCapabilities {
    /// Current platform
    pub platform: Platform,
    /// CPU architecture
    pub architecture: CpuArchitecture,
    /// Supported features
    pub supported_features: Vec<PlatformFeature>,
    /// Feature-specific capabilities
    pub feature_capabilities: HashMap<String, serde_json::Value>,
    /// Performance characteristics
    pub performance_profile: PerformanceProfile,
}

/// Performance profile for the platform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceProfile {
    /// CPU performance tier (1-10, 10 being highest)
    pub cpu_tier: u8,
    /// GPU performance tier (1-10, 10 being highest)
    pub gpu_tier: u8,
    /// Memory performance tier (1-10, 10 being highest)
    pub memory_tier: u8,
    /// Storage performance tier (1-10, 10 being highest)
    pub storage_tier: u8,
    /// Overall performance tier
    pub overall_tier: u8,
    /// Power profile (battery, balanced, performance)
    pub power_profile: PowerProfile,
}

/// Power management profiles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerProfile {
    /// Battery saving mode
    Battery,
    /// Balanced performance and power
    Balanced,
    /// Maximum performance
    Performance,
}

/// System information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// Operating system name
    pub os_name: String,
    /// OS version
    pub os_version: String,
    /// Kernel version
    pub kernel_version: String,
    /// System uptime in seconds
    pub uptime_seconds: u64,
    /// Hardware information
    pub hardware: HardwareInfo,
    /// Environment variables
    pub environment: HashMap<String, String>,
}

/// Cross-platform device interface
pub struct DeviceInterface {
    /// Platform capabilities
    capabilities: PlatformCapabilities,
    /// System information
    system_info: SystemInfo,
    /// Platform-specific device handle
    #[cfg(target_os = "windows")]
    win32_device: Option<super::Win32Device>,
    #[cfg(target_os = "linux")]
    linux_device: Option<super::LinuxDevice>,
    #[cfg(target_os = "macos")]
    macos_device: Option<super::MacOsDevice>,
}

impl DeviceInterface {
    /// Create a new device interface
    pub async fn new() -> Result<Self> {
        let capabilities = Self::detect_capabilities().await?;
        let system_info = Self::gather_system_info().await?;

        let mut interface = Self {
            capabilities,
            system_info,
            #[cfg(target_os = "windows")]
            win32_device: None,
            #[cfg(target_os = "linux")]
            linux_device: None,
            #[cfg(target_os = "macos")]
            macos_device: None,
        };

        // Initialize platform-specific device
        interface.initialize_platform_device().await?;

        Ok(interface)
    }

    /// Get platform capabilities
    #[must_use]
    pub fn get_capabilities(&self) -> &PlatformCapabilities {
        &self.capabilities
    }

    /// Get system information
    #[must_use]
    pub fn get_system_info(&self) -> &SystemInfo {
        &self.system_info
    }

    /// Check if a feature is supported
    #[must_use]
    pub fn supports_feature(&self, feature: PlatformFeature) -> bool {
        self.capabilities.supported_features.contains(&feature)
    }

    /// Get feature-specific capabilities
    #[must_use]
    pub fn get_feature_capabilities(&self, feature: &str) -> Option<&serde_json::Value> {
        self.capabilities.feature_capabilities.get(feature)
    }

    /// Set thread priority
    pub async fn set_thread_priority(&self, priority: ThreadPriority) -> Result<()> {
        match Platform::current() {
            #[cfg(target_os = "windows")]
            Platform::Windows => {
                if let Some(device) = &self.win32_device {
                    device.set_thread_priority(priority).await
                } else {
                    Err(PlatformError::DeviceNotSupported(
                        "Win32 device not initialized".to_string(),
                    ))
                }
            }
            #[cfg(target_os = "linux")]
            Platform::Linux => {
                if let Some(device) = &self.linux_device {
                    device.set_thread_priority(priority).await
                } else {
                    Err(PlatformError::DeviceNotSupported(
                        "Linux device not initialized".to_string(),
                    ))
                }
            }
            #[cfg(target_os = "macos")]
            Platform::MacOS => {
                if let Some(device) = &self.macos_device {
                    device.set_thread_priority(priority).await
                } else {
                    Err(PlatformError::DeviceNotSupported(
                        "macOS device not initialized".to_string(),
                    ))
                }
            }
            _ => Err(PlatformError::DeviceNotSupported(
                "Unsupported platform".to_string(),
            )),
        }
    }

    /// Get current CPU usage
    pub async fn get_cpu_usage(&self) -> Result<f32> {
        // Platform-specific CPU usage implementation
        match Platform::current() {
            #[cfg(target_os = "windows")]
            Platform::Windows => {
                if let Some(device) = &self.win32_device {
                    device.get_cpu_usage().await
                } else {
                    Ok(0.0)
                }
            }
            #[cfg(target_os = "linux")]
            Platform::Linux => {
                if let Some(device) = &self.linux_device {
                    device.get_cpu_usage().await
                } else {
                    Ok(0.0)
                }
            }
            #[cfg(target_os = "macos")]
            Platform::MacOS => {
                if let Some(device) = &self.macos_device {
                    device.get_cpu_usage().await
                } else {
                    Ok(0.0)
                }
            }
            _ => Ok(0.0),
        }
    }

    /// Get memory usage
    pub async fn get_memory_usage(&self) -> Result<MemoryUsage> {
        // Platform-specific memory usage implementation
        let default_usage = MemoryUsage {
            physical_used: 0,
            physical_total: self.system_info.hardware.memory.total_physical,
            virtual_used: 0,
            virtual_total: self.system_info.hardware.memory.total_virtual,
        };

        match Platform::current() {
            #[cfg(target_os = "windows")]
            Platform::Windows => {
                if let Some(device) = &self.win32_device {
                    device.get_memory_usage().await
                } else {
                    Ok(default_usage)
                }
            }
            #[cfg(target_os = "linux")]
            Platform::Linux => {
                if let Some(device) = &self.linux_device {
                    device.get_memory_usage().await
                } else {
                    Ok(default_usage)
                }
            }
            #[cfg(target_os = "macos")]
            Platform::MacOS => {
                if let Some(device) = &self.macos_device {
                    device.get_memory_usage().await
                } else {
                    Ok(default_usage)
                }
            }
            _ => Ok(default_usage),
        }
    }

    /// Initialize platform-specific device
    async fn initialize_platform_device(&mut self) -> Result<()> {
        match Platform::current() {
            #[cfg(target_os = "windows")]
            Platform::Windows => {
                let device = super::Win32Device::new().await?;
                self.win32_device = Some(device);
            }
            #[cfg(target_os = "linux")]
            Platform::Linux => {
                let device = super::LinuxDevice::new().await?;
                self.linux_device = Some(device);
            }
            #[cfg(target_os = "macos")]
            Platform::MacOS => {
                let device = super::MacOsDevice::new().await?;
                self.macos_device = Some(device);
            }
            _ => {
                return Err(PlatformError::DeviceNotSupported(
                    "Unsupported platform".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Detect platform capabilities
    async fn detect_capabilities() -> Result<PlatformCapabilities> {
        let platform = Platform::current();
        let architecture = CpuArchitecture::current();

        let mut supported_features = Vec::new();
        let mut feature_capabilities = HashMap::new();

        // Add platform-specific features
        for feature in [
            PlatformFeature::DirectSound,
            PlatformFeature::Wasapi,
            PlatformFeature::Alsa,
            PlatformFeature::PulseAudio,
            PlatformFeature::CoreAudio,
            PlatformFeature::OpenAL,
            PlatformFeature::DirectX,
            PlatformFeature::Vulkan,
            PlatformFeature::Metal,
            PlatformFeature::OpenGL,
            PlatformFeature::FileMapping,
            PlatformFeature::SharedMemory,
            PlatformFeature::ThreadPriority,
            PlatformFeature::RealTimeScheduling,
            PlatformFeature::SimdInstructions,
            PlatformFeature::MultiCore,
            PlatformFeature::GpuAcceleration,
        ] {
            if platform.supports_feature(feature) {
                supported_features.push(feature);
            }
        }

        // Add SIMD capabilities
        if architecture.supports_simd() {
            feature_capabilities.insert(
                "simd".to_string(),
                serde_json::json!({
                    "supported": true,
                    "instruction_sets": match architecture {
                        CpuArchitecture::X86_64 | CpuArchitecture::X86 => vec!["SSE", "SSE2", "AVX"],
                        CpuArchitecture::Aarch64 | CpuArchitecture::Arm => vec!["NEON"],
                        _ => vec!["None"],
                    }
                }),
            );
        }

        let performance_profile = Self::detect_performance_profile().await;

        Ok(PlatformCapabilities {
            platform,
            architecture,
            supported_features,
            feature_capabilities,
            performance_profile,
        })
    }

    /// Detect performance profile
    async fn detect_performance_profile() -> PerformanceProfile {
        // Simplified performance detection - real implementation would benchmark
        let cpu_cores = std::thread::available_parallelism()
            .map_or(1, std::num::NonZero::get) as u32;

        let cpu_tier = match cpu_cores {
            1..=2 => 3,
            3..=4 => 5,
            5..=8 => 7,
            9..=16 => 9,
            _ => 10,
        } as u8;

        PerformanceProfile {
            cpu_tier,
            gpu_tier: 5, // Default assumption
            memory_tier: 6,
            storage_tier: 7,
            overall_tier: (cpu_tier + 5 + 6 + 7) / 4,
            power_profile: PowerProfile::Balanced,
        }
    }

    /// Gather system information
    async fn gather_system_info() -> Result<SystemInfo> {
        let os_name = std::env::consts::OS.to_string();
        let _arch = std::env::consts::ARCH.to_string();

        Ok(SystemInfo {
            os_name: os_name.clone(),
            os_version: "Unknown".to_string(), // Would query actual version
            kernel_version: "Unknown".to_string(),
            uptime_seconds: 0, // Would query actual uptime
            hardware: HardwareInfo::default(),
            environment: std::env::vars().collect(),
        })
    }
}

/// Thread priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadPriority {
    /// Lowest priority
    Lowest,
    /// Below normal priority
    BelowNormal,
    /// Normal priority
    Normal,
    /// Above normal priority
    AboveNormal,
    /// Highest priority
    Highest,
    /// Real-time priority
    RealTime,
}

/// Memory usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsage {
    /// Physical memory used in bytes
    pub physical_used: u64,
    /// Total physical memory in bytes
    pub physical_total: u64,
    /// Virtual memory used in bytes
    pub virtual_used: u64,
    /// Total virtual memory in bytes
    pub virtual_total: u64,
}

impl MemoryUsage {
    /// Get physical memory usage percentage
    #[must_use]
    pub fn physical_usage_percent(&self) -> f32 {
        if self.physical_total > 0 {
            (self.physical_used as f64 / self.physical_total as f64 * 100.0) as f32
        } else {
            0.0
        }
    }

    /// Get virtual memory usage percentage
    #[must_use]
    pub fn virtual_usage_percent(&self) -> f32 {
        if self.virtual_total > 0 {
            (self.virtual_used as f64 / self.virtual_total as f64 * 100.0) as f32
        } else {
            0.0
        }
    }
}
