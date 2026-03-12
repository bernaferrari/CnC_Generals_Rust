//! System Information Module
//!
//! Provides comprehensive system information detection:
//! - CPU information (model, cores, frequency)
//! - Memory information (total, available)
//! - GPU information (vendor, model, memory)
//! - Operating system details
//! - Display information

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// CPU vendor type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CpuVendor {
    Intel,
    AMD,
    ARM,
    Unknown,
}

/// CPU information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    /// CPU vendor
    pub vendor: CpuVendor,
    /// CPU model name
    pub model: String,
    /// Number of physical cores
    pub physical_cores: usize,
    /// Number of logical cores (with hyperthreading)
    pub logical_cores: usize,
    /// CPU frequency in MHz
    pub frequency_mhz: u64,
    /// CPU features (SSE, AVX, etc.)
    pub features: Vec<String>,
}

impl Default for CpuInfo {
    fn default() -> Self {
        Self {
            vendor: CpuVendor::Unknown,
            model: String::from("Unknown CPU"),
            physical_cores: 1,
            logical_cores: 1,
            frequency_mhz: 0,
            features: Vec::new(),
        }
    }
}

/// Memory information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    /// Total physical memory in bytes
    pub total_bytes: u64,
    /// Available physical memory in bytes
    pub available_bytes: u64,
    /// Total virtual memory in bytes
    pub total_virtual_bytes: u64,
    /// Available virtual memory in bytes
    pub available_virtual_bytes: u64,
}

impl Default for MemoryInfo {
    fn default() -> Self {
        Self {
            total_bytes: 0,
            available_bytes: 0,
            total_virtual_bytes: 0,
            available_virtual_bytes: 0,
        }
    }
}

impl MemoryInfo {
    /// Get total memory in megabytes
    pub fn total_mb(&self) -> u64 {
        self.total_bytes / (1024 * 1024)
    }

    /// Get available memory in megabytes
    pub fn available_mb(&self) -> u64 {
        self.available_bytes / (1024 * 1024)
    }

    /// Get memory usage percentage
    pub fn usage_percent(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        ((self.total_bytes - self.available_bytes) as f64 / self.total_bytes as f64) * 100.0
    }
}

/// GPU vendor type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuVendor {
    NVIDIA,
    AMD,
    Intel,
    Unknown,
}

/// GPU information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// GPU vendor
    pub vendor: GpuVendor,
    /// GPU model name
    pub model: String,
    /// GPU memory in bytes (VRAM)
    pub memory_bytes: u64,
    /// Driver version
    pub driver_version: String,
}

impl Default for GpuInfo {
    fn default() -> Self {
        Self {
            vendor: GpuVendor::Unknown,
            model: String::from("Unknown GPU"),
            memory_bytes: 0,
            driver_version: String::from("Unknown"),
        }
    }
}

impl GpuInfo {
    /// Get GPU memory in megabytes
    pub fn memory_mb(&self) -> u64 {
        self.memory_bytes / (1024 * 1024)
    }
}

/// Operating system type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OsType {
    Windows,
    Linux,
    MacOS,
    Unknown,
}

/// Operating system information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInfo {
    /// OS type
    pub os_type: OsType,
    /// OS name
    pub name: String,
    /// OS version
    pub version: String,
    /// OS build number
    pub build: String,
    /// Architecture (x86, x64, arm64, etc.)
    pub arch: String,
}

impl Default for OsInfo {
    fn default() -> Self {
        Self {
            os_type: OsType::Unknown,
            name: String::from("Unknown OS"),
            version: String::from("0.0"),
            build: String::from("0"),
            arch: String::from("unknown"),
        }
    }
}

/// Display information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayInfo {
    /// Primary display width in pixels
    pub width: u32,
    /// Primary display height in pixels
    pub height: u32,
    /// Refresh rate in Hz
    pub refresh_rate: u32,
    /// Bits per pixel
    pub bits_per_pixel: u32,
    /// Number of displays
    pub display_count: usize,
}

impl Default for DisplayInfo {
    fn default() -> Self {
        Self {
            width: 1024,
            height: 768,
            refresh_rate: 60,
            bits_per_pixel: 32,
            display_count: 1,
        }
    }
}

/// Comprehensive system information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// CPU information
    pub cpu: CpuInfo,
    /// Memory information
    pub memory: MemoryInfo,
    /// GPU information
    pub gpu: GpuInfo,
    /// Operating system information
    pub os: OsInfo,
    /// Display information
    pub display: DisplayInfo,
}

impl Default for SystemInfo {
    fn default() -> Self {
        Self {
            cpu: CpuInfo::default(),
            memory: MemoryInfo::default(),
            gpu: GpuInfo::default(),
            os: OsInfo::default(),
            display: DisplayInfo::default(),
        }
    }
}

impl SystemInfo {
    /// Detect and gather system information
    pub fn detect() -> Self {
        let mut info = SystemInfo::default();

        // Detect CPU information
        info.cpu = Self::detect_cpu();

        // Detect memory information
        info.memory = Self::detect_memory();

        // Detect GPU information
        info.gpu = Self::detect_gpu();

        // Detect OS information
        info.os = Self::detect_os();

        // Detect display information
        info.display = Self::detect_display();

        info
    }

    /// Detect CPU information
    fn detect_cpu() -> CpuInfo {
        let mut cpu = CpuInfo::default();

        // Get number of logical cores
        cpu.logical_cores = num_cpus::get();
        cpu.physical_cores = num_cpus::get_physical();

        // Try to get CPU info using sysinfo
        #[cfg(feature = "sysinfo")]
        {
            use sysinfo::{CpuExt, System, SystemExt};
            let mut sys = System::new_all();
            sys.refresh_cpu();

            if let Some(cpu_info) = sys.cpus().first() {
                cpu.model = cpu_info.brand().to_string();
                cpu.frequency_mhz = cpu_info.frequency();
            }
        }

        // Detect vendor from model string
        if cpu.model.to_lowercase().contains("intel") {
            cpu.vendor = CpuVendor::Intel;
        } else if cpu.model.to_lowercase().contains("amd") {
            cpu.vendor = CpuVendor::AMD;
        } else if cpu.model.to_lowercase().contains("arm") {
            cpu.vendor = CpuVendor::ARM;
        }

        // Detect CPU features (simplified)
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("sse") {
                cpu.features.push("SSE".to_string());
            }
            if is_x86_feature_detected!("sse2") {
                cpu.features.push("SSE2".to_string());
            }
            if is_x86_feature_detected!("sse3") {
                cpu.features.push("SSE3".to_string());
            }
            if is_x86_feature_detected!("avx") {
                cpu.features.push("AVX".to_string());
            }
            if is_x86_feature_detected!("avx2") {
                cpu.features.push("AVX2".to_string());
            }
        }

        cpu
    }

    /// Detect memory information
    fn detect_memory() -> MemoryInfo {
        #[allow(unused_mut)]
        let mut memory = MemoryInfo::default();

        #[cfg(feature = "sysinfo")]
        {
            use sysinfo::{System, SystemExt};
            let mut sys = System::new_all();
            sys.refresh_memory();

            memory.total_bytes = sys.total_memory() * 1024; // sysinfo returns KB
            memory.available_bytes = sys.available_memory() * 1024;
            memory.total_virtual_bytes = sys.total_swap() * 1024;
            memory.available_virtual_bytes = sys.free_swap() * 1024;
        }

        memory
    }

    /// Detect GPU information
    fn detect_gpu() -> GpuInfo {
        let mut gpu = GpuInfo::default();

        #[cfg(feature = "graphics")]
        {
            fn score_device_type(device_type: wgpu::DeviceType) -> u8 {
                match device_type {
                    wgpu::DeviceType::DiscreteGpu => 0,
                    wgpu::DeviceType::IntegratedGpu => 1,
                    wgpu::DeviceType::VirtualGpu => 2,
                    wgpu::DeviceType::Cpu => 3,
                    wgpu::DeviceType::Other => 4,
                }
            }

            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
            let mut selected_info: Option<wgpu::AdapterInfo> = None;

            for adapter in instance.enumerate_adapters(wgpu::Backends::all()) {
                let info = adapter.get_info();
                let should_select = selected_info
                    .as_ref()
                    .map(|current| {
                        score_device_type(info.device_type) < score_device_type(current.device_type)
                    })
                    .unwrap_or(true);

                if should_select {
                    selected_info = Some(info);
                }
            }

            if let Some(info) = selected_info {
                gpu.model = info.name;
                gpu.vendor = match info.vendor {
                    0x10DE => GpuVendor::NVIDIA,
                    0x1002 | 0x1022 => GpuVendor::AMD,
                    0x8086 => GpuVendor::Intel,
                    _ => GpuVendor::Unknown,
                };
                gpu.driver_version = format!("{:?}", info.backend);
            }
        }

        gpu
    }

    /// Detect operating system information
    fn detect_os() -> OsInfo {
        let mut os = OsInfo::default();

        // Detect OS type
        if cfg!(target_os = "windows") {
            os.os_type = OsType::Windows;
            os.name = String::from("Windows");
        } else if cfg!(target_os = "linux") {
            os.os_type = OsType::Linux;
            os.name = String::from("Linux");
        } else if cfg!(target_os = "macos") {
            os.os_type = OsType::MacOS;
            os.name = String::from("macOS");
        }

        // Get OS version
        #[cfg(feature = "sysinfo")]
        {
            use sysinfo::{System, SystemExt};
            let sys = System::new_all();
            if let Some(version) = sys.os_version() {
                os.version = version;
            }
            if let Some(name) = sys.name() {
                os.name = name;
            }
        }

        // Get architecture
        os.arch = std::env::consts::ARCH.to_string();

        os
    }

    /// Detect display information
    fn detect_display() -> DisplayInfo {
        let mut display = DisplayInfo::default();

        #[cfg(feature = "graphics")]
        {
            use winit::event_loop::EventLoopBuilder;

            if let Ok(event_loop) = EventLoopBuilder::new().build() {
                let primary = event_loop
                    .primary_monitor()
                    .or_else(|| event_loop.available_monitors().next());

                if let Some(primary) = primary {
                    let size = primary.size();
                    display.width = size.width.max(1);
                    display.height = size.height.max(1);
                    display.refresh_rate = primary
                        .refresh_rate_millihertz()
                        .map(|mhz| (mhz / 1000).max(1))
                        .unwrap_or(60);
                }

                let monitor_count = event_loop.available_monitors().count();
                if monitor_count > 0 {
                    display.display_count = monitor_count;
                }
            }
        }

        display
    }

    /// Generate a formatted system information report
    pub fn generate_report(&self) -> String {
        format!(
            "System Information Report\n\
            ========================\n\
            \n\
            CPU:\n\
            - Vendor: {:?}\n\
            - Model: {}\n\
            - Cores: {} physical, {} logical\n\
            - Frequency: {} MHz\n\
            - Features: {}\n\
            \n\
            Memory:\n\
            - Total: {} MB\n\
            - Available: {} MB\n\
            - Usage: {:.1}%\n\
            \n\
            GPU:\n\
            - Vendor: {:?}\n\
            - Model: {}\n\
            - Memory: {} MB\n\
            - Driver: {}\n\
            \n\
            Operating System:\n\
            - Type: {:?}\n\
            - Name: {}\n\
            - Version: {}\n\
            - Architecture: {}\n\
            \n\
            Display:\n\
            - Resolution: {}x{}\n\
            - Refresh Rate: {} Hz\n\
            - Bits Per Pixel: {}\n\
            - Display Count: {}\n",
            self.cpu.vendor,
            self.cpu.model,
            self.cpu.physical_cores,
            self.cpu.logical_cores,
            self.cpu.frequency_mhz,
            self.cpu.features.join(", "),
            self.memory.total_mb(),
            self.memory.available_mb(),
            self.memory.usage_percent(),
            self.gpu.vendor,
            self.gpu.model,
            self.gpu.memory_mb(),
            self.gpu.driver_version,
            self.os.os_type,
            self.os.name,
            self.os.version,
            self.os.arch,
            self.display.width,
            self.display.height,
            self.display.refresh_rate,
            self.display.bits_per_pixel,
            self.display.display_count
        )
    }
}

/// Global system information instance
static SYSTEM_INFO: OnceCell<Mutex<SystemInfo>> = OnceCell::new();

/// Get the global system information instance
pub fn get_system_info() -> SystemInfo {
    SYSTEM_INFO
        .get_or_init(|| Mutex::new(SystemInfo::detect()))
        .lock()
        .unwrap()
        .clone()
}

/// Refresh the global system information
pub fn refresh_system_info() {
    if let Some(info) = SYSTEM_INFO.get() {
        let mut guard = info.lock().unwrap();
        *guard = SystemInfo::detect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_info_detection() {
        let info = SystemInfo::detect();
        assert!(info.cpu.logical_cores > 0);
        assert!(info.cpu.physical_cores > 0);
    }

    #[test]
    fn test_memory_info_calculations() {
        let mut memory = MemoryInfo::default();
        memory.total_bytes = 16 * 1024 * 1024 * 1024; // 16 GB
        memory.available_bytes = 8 * 1024 * 1024 * 1024; // 8 GB

        assert_eq!(memory.total_mb(), 16384);
        assert_eq!(memory.available_mb(), 8192);
        assert!((memory.usage_percent() - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_gpu_info_memory() {
        let mut gpu = GpuInfo::default();
        gpu.memory_bytes = 8 * 1024 * 1024 * 1024; // 8 GB

        assert_eq!(gpu.memory_mb(), 8192);
    }

    #[test]
    fn test_system_info_report() {
        let info = SystemInfo::default();
        let report = info.generate_report();
        assert!(report.contains("System Information Report"));
        assert!(report.contains("CPU:"));
        assert!(report.contains("Memory:"));
    }

    #[test]
    fn test_global_system_info() {
        let info1 = get_system_info();
        let info2 = get_system_info();

        // Should return consistent information
        assert_eq!(info1.cpu.logical_cores, info2.cpu.logical_cores);
    }
}
