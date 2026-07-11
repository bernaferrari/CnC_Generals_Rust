//! # Master Integration Library
//!
//! This library provides the master integration system for the C&C Generals Zero Hour Rust engine.
//! It coordinates all subsystems, manages performance optimization, and provides system-wide services.
//!
//! ## Features
//!
//! - **Engine Coordination**: Orchestrates all game engine subsystems
//! - **Performance Management**: System-wide performance optimization and monitoring
//! - **Resource Management**: Global resource management and pooling
//! - **Event System**: Master event coordination across all systems
//! - **Diagnostics**: Comprehensive system diagnostics and profiling
//! - **Memory Safety**: Zero-allocation hot paths with memory safety guarantees
//! - **SIMD Acceleration**: Hardware-accelerated operations where possible
//!
//! ## Architecture
//!
//! The integration system follows a hierarchical coordinator pattern:
//!
//! ```text
//! EngineCoordinator (Master)
//! ├── GameClient (Rendering & UI)
//! ├── GameLogic (Game State & AI)
//! ├── GameNetwork (Multiplayer)
//! ├── AudioSystem (Sound & Music)
//! └── ResourceSystem (Assets & Files)
//! ```

use game_network::NetworkClock;
use parking_lot::RwLock;
use std::sync::Arc;
use tracing::{debug, info, instrument, trace};
use ww3d_engine::FrameTiming;

// Re-export all major components for easy access
pub use game_client;
pub use game_engine;
pub use game_engine_device;
pub use game_logic;
pub use game_network;

// Re-export WWVegas libraries
pub use math_utilities;
pub use ww_save_load;
pub use wwaudio;
pub use wwdebug;
pub use wwdownload;
pub use wwlib_rust;
pub use wwshade_rust;

// Re-export base types
pub use base_types;
pub use ini_parser;
pub use memory_system;
pub use string_system;

// Internal modules
pub mod diagnostics;
pub mod engine_coordinator;
pub mod event_system;
pub mod performance_manager;
pub mod resource_manager;

// Re-export main integration components
pub use diagnostics::DiagnosticsSystem;
pub use engine_coordinator::EngineCoordinator;
pub use event_system::{EventSystem, SystemEvent};
pub use performance_manager::PerformanceManager;
pub use resource_manager::ResourceManager;

/// Integration result type for error handling
pub type IntegrationResult<T> = anyhow::Result<T>;

/// Integration error types
#[derive(thiserror::Error, Debug)]
pub enum IntegrationError {
    #[error("Subsystem initialization failed: {subsystem}")]
    SubsystemInitFailed { subsystem: String },

    #[error("Performance degradation detected: {metric}: {value}")]
    PerformanceDegradation { metric: String, value: f64 },

    #[error("Resource exhaustion: {resource_type}")]
    ResourceExhaustion { resource_type: String },

    #[error("Event system error: {message}")]
    EventSystemError { message: String },

    #[error("Diagnostics error: {message}")]
    DiagnosticsError { message: String },

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },
}

/// Main integration configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IntegrationConfig {
    /// Performance monitoring configuration
    pub performance: PerformanceConfig,

    /// Resource management configuration
    pub resources: ResourceConfig,

    /// Event system configuration
    pub events: EventConfig,

    /// Diagnostics configuration
    pub diagnostics: DiagnosticsConfig,

    /// SIMD optimization settings
    pub simd: SimdConfig,

    /// Memory management settings
    pub memory: MemoryConfig,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PerformanceConfig {
    /// Target frame rate
    pub target_fps: f64,

    /// Performance monitoring interval (ms)
    pub monitor_interval_ms: u64,

    /// Memory usage warning threshold (MB)
    pub memory_warning_mb: u64,

    /// CPU usage warning threshold (%)
    pub cpu_warning_percent: f64,

    /// Enable automatic performance tuning
    pub auto_tuning: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResourceConfig {
    /// Maximum texture memory (MB)
    pub max_texture_memory_mb: u64,

    /// Maximum audio buffer size (MB)
    pub max_audio_buffer_mb: u64,

    /// Resource cache size (MB)
    pub cache_size_mb: u64,

    /// Enable resource compression
    pub compression: bool,

    /// Enable resource streaming
    pub streaming: bool,

    /// Maximum texture cache (MB)
    pub max_texture_cache_mb: u64,

    /// Maximum audio cache (MB)
    pub max_audio_cache_mb: u64,

    /// Maximum model cache (MB)
    pub max_model_cache_mb: u64,

    /// Maximum data cache (MB)
    pub max_data_cache_mb: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventConfig {
    /// Maximum events per frame
    pub max_events_per_frame: usize,

    /// Event queue capacity
    pub queue_capacity: usize,

    /// Enable event logging
    pub logging: bool,

    /// Enable event profiling
    pub profiling: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiagnosticsConfig {
    /// Enable performance profiling
    pub profiling: bool,

    /// Enable memory tracking
    pub memory_tracking: bool,

    /// Enable system monitoring
    pub system_monitoring: bool,

    /// Diagnostics update interval (ms)
    pub update_interval_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimdConfig {
    /// Enable SIMD optimizations
    pub enabled: bool,

    /// Force specific instruction set (AVX2, SSE4.1, etc.)
    pub force_instruction_set: Option<String>,

    /// Enable automatic SIMD detection
    pub auto_detect: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryConfig {
    /// Enable memory pooling
    pub pooling: bool,

    /// Memory pool size (MB)
    pub pool_size_mb: u64,

    /// Enable memory compression
    pub compression: bool,

    /// Memory allocation strategy
    pub allocation_strategy: String,

    /// Enable memory debugging
    pub debugging: bool,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            performance: PerformanceConfig {
                target_fps: 60.0,
                monitor_interval_ms: 1000,
                memory_warning_mb: 2048,
                cpu_warning_percent: 90.0,
                auto_tuning: true,
            },
            resources: ResourceConfig {
                max_texture_memory_mb: 1024,
                max_audio_buffer_mb: 256,
                cache_size_mb: 512,
                compression: true,
                streaming: true,
                max_texture_cache_mb: 512,
                max_audio_cache_mb: 256,
                max_model_cache_mb: 256,
                max_data_cache_mb: 256,
            },
            events: EventConfig {
                max_events_per_frame: 10000,
                queue_capacity: 50000,
                logging: false,
                profiling: false,
            },
            diagnostics: DiagnosticsConfig {
                profiling: true,
                memory_tracking: true,
                system_monitoring: true,
                update_interval_ms: 100,
            },
            simd: SimdConfig {
                enabled: true,
                force_instruction_set: None,
                auto_detect: true,
            },
            memory: MemoryConfig {
                pooling: true,
                pool_size_mb: 256,
                compression: false,
                allocation_strategy: "bump_allocator".to_string(),
                debugging: false,
            },
        }
    }
}

/// Master integration system
#[derive(Debug)]
pub struct IntegrationSystem {
    config: IntegrationConfig,
    coordinator: Arc<RwLock<EngineCoordinator>>,
    performance: Arc<RwLock<PerformanceManager>>,
    resources: Arc<RwLock<ResourceManager>>,
    events: Arc<EventSystem>,
    diagnostics: Arc<RwLock<DiagnosticsSystem>>,
}

impl IntegrationSystem {
    /// Create a new integration system with default configuration
    #[instrument(name = "integration_new")]
    pub async fn new() -> IntegrationResult<Self> {
        Self::with_config(IntegrationConfig::default()).await
    }

    /// Create a new integration system with custom configuration
    #[instrument(name = "integration_new_with_config", skip(config))]
    pub async fn with_config(config: IntegrationConfig) -> IntegrationResult<Self> {
        info!("Initializing Integration System with configuration");
        debug!("Config: {:?}", config);

        // Initialize event system first as other components depend on it
        let events = Arc::new(EventSystem::new(config.events.clone())?);

        // Initialize diagnostics system early for monitoring
        let diagnostics = Arc::new(RwLock::new(DiagnosticsSystem::new(
            config.diagnostics.clone(),
            events.clone(),
        )?));

        // Initialize performance manager
        let performance = Arc::new(RwLock::new(PerformanceManager::new(
            config.performance.clone(),
            events.clone(),
        )?));

        // Initialize resource manager
        let resources = Arc::new(RwLock::new(ResourceManager::new(
            config.resources.clone(),
            events.clone(),
        )?));

        // Initialize engine coordinator (orchestrates all subsystems)
        let coordinator = Arc::new(RwLock::new(EngineCoordinator::new(
            performance.clone(),
            resources.clone(),
            events.clone(),
            diagnostics.clone(),
        )?));

        let system = Self {
            config,
            coordinator,
            performance,
            resources,
            events,
            diagnostics,
        };

        info!("Integration System initialized successfully");
        Ok(system)
    }

    /// Initialize all subsystems
    #[instrument(name = "integration_initialize", skip(self))]
    pub async fn initialize(&mut self) -> IntegrationResult<()> {
        info!("Initializing all subsystems");

        // Start diagnostics monitoring
        self.diagnostics.write().start_monitoring().await?;

        // Start performance monitoring
        self.performance.write().start_monitoring().await?;

        // Initialize resource pools
        self.resources.write().initialize_pools().await?;

        // Start event system
        self.events.start().await?;

        // Initialize engine coordinator (this starts all game subsystems)
        self.coordinator.write().initialize().await?;

        // Send initialization complete event
        self.events
            .send_system_event(SystemEvent::IntegrationInitialized)
            .await?;

        info!("All subsystems initialized successfully");
        Ok(())
    }

    /// Update all systems (called once per frame)
    #[instrument(name = "integration_update", skip(self, timing))]
    pub async fn update(&mut self, timing: &FrameTiming) -> IntegrationResult<()> {
        NetworkClock::override_with_duration(timing.total_time);
        trace!(
            "Updating integration system, frame: {}, delta: {:.6}",
            timing.frame_number,
            timing.delta_seconds()
        );

        // Update diagnostics first to monitor system health
        self.diagnostics.write().update(timing).await?;

        // Update performance manager
        self.performance.write().update(timing).await?;

        // Update resource manager
        self.resources.write().update(timing).await?;

        // Process events
        self.events.process_events().await?;

        // Update engine coordinator (updates all game systems)
        self.coordinator.write().update(timing).await?;

        Ok(())
    }

    /// Shutdown all systems gracefully
    #[instrument(name = "integration_shutdown", skip(self))]
    pub async fn shutdown(&mut self) -> IntegrationResult<()> {
        info!("Shutting down integration system");

        // Send shutdown event
        self.events
            .send_system_event(SystemEvent::IntegrationShutdown)
            .await?;

        // Shutdown in reverse order of initialization
        self.coordinator.write().shutdown().await?;
        self.events.shutdown().await?;
        self.resources.write().shutdown().await?;
        self.performance.write().shutdown().await?;
        self.diagnostics.write().shutdown().await?;

        info!("Integration system shutdown complete");
        Ok(())
    }

    /// Get system performance metrics
    pub fn get_performance_metrics(&self) -> performance_manager::PerformanceMetrics {
        self.performance.read().get_metrics()
    }

    /// Latest performance sample emitted on the event bus (if any).
    pub fn latest_performance_sample(&self) -> Option<performance_manager::PerformanceMetrics> {
        self.events.latest_performance_sample()
    }

    /// Latest resource usage sample emitted on the event bus (if any).
    pub fn latest_resource_usage(&self) -> Option<resource_manager::ResourceUsage> {
        self.events.latest_resource_usage()
    }

    /// Latest diagnostics snapshot, if any.
    pub fn latest_diagnostics(&self) -> Option<diagnostics::SystemDiagnostics> {
        self.events.latest_diagnostics()
    }

    /// Get system resource usage
    pub fn get_resource_usage(&self) -> resource_manager::ResourceUsage {
        self.resources.read().get_usage()
    }

    /// Get system diagnostics
    pub fn get_diagnostics(&self) -> diagnostics::SystemDiagnostics {
        self.diagnostics.read().get_diagnostics()
    }

    /// Get configuration
    pub fn get_config(&self) -> &IntegrationConfig {
        &self.config
    }

    /// Update configuration
    #[instrument(name = "integration_update_config", skip(self, config))]
    pub async fn update_config(&mut self, config: IntegrationConfig) -> IntegrationResult<()> {
        info!("Updating integration system configuration");

        // Update subsystem configurations
        self.performance
            .write()
            .update_config(config.performance.clone())
            .await?;
        self.resources
            .write()
            .update_config(config.resources.clone())
            .await?;
        self.events.update_config(config.events.clone()).await?;
        self.diagnostics
            .write()
            .update_config(config.diagnostics.clone())
            .await?;

        self.config = config;

        info!("Configuration updated successfully");
        Ok(())
    }
}

/// Initialize tracing subscriber for logging
pub fn init_logging() -> IntegrationResult<()> {
    use tracing_subscriber::{filter::EnvFilter, fmt::format::FmtSpan, prelude::*};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,integration=debug"));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .with_ansi(true);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();

    info!("Logging initialized");
    Ok(())
}

/// Hardware capability detection
pub mod hardware {
    use parking_lot::RwLock;
    use std::sync::Once;

    static INIT: Once = Once::new();
    static CAPABILITIES: RwLock<Option<HardwareCapabilities>> = RwLock::new(None);

    #[derive(Debug, Clone)]
    pub struct HardwareCapabilities {
        pub cpu_features: CpuFeatures,
        pub memory_info: MemoryInfo,
        pub gpu_info: Option<GpuInfo>,
    }

    #[derive(Debug, Clone)]
    pub struct CpuFeatures {
        pub sse41: bool,
        pub sse42: bool,
        pub avx: bool,
        pub avx2: bool,
        pub avx512f: bool,
        pub cores: usize,
        pub threads: usize,
    }

    #[derive(Debug, Clone)]
    pub struct MemoryInfo {
        pub total_mb: u64,
        pub available_mb: u64,
        pub page_size: usize,
    }

    #[derive(Debug, Clone)]
    pub struct GpuInfo {
        pub name: String,
        pub memory_mb: u64,
        pub compute_units: u32,
    }

    /// Detect hardware capabilities
    pub fn detect_capabilities() -> HardwareCapabilities {
        INIT.call_once(|| {
            let caps = detect_hardware_capabilities();
            *CAPABILITIES.write() = Some(caps);
        });

        CAPABILITIES.read().as_ref().unwrap().clone()
    }

    fn detect_hardware_capabilities() -> HardwareCapabilities {
        #[cfg(target_arch = "x86_64")]
        let cpu_features = {
            use std::arch;
            CpuFeatures {
                sse41: arch::x86_64::__cpuid(1).ecx & (1 << 19) != 0,
                sse42: arch::x86_64::__cpuid(1).ecx & (1 << 20) != 0,
                avx: arch::x86_64::__cpuid(1).ecx & (1 << 28) != 0,
                avx2: arch::x86_64::__cpuid_count(7, 0).ebx & (1 << 5) != 0,
                avx512f: arch::x86_64::__cpuid_count(7, 0).ebx & (1 << 16) != 0,
                cores: num_cpus::get_physical(),
                threads: num_cpus::get(),
            }
        };
        #[cfg(not(target_arch = "x86_64"))]
        let cpu_features = CpuFeatures {
            sse41: false,
            sse42: false,
            avx: false,
            avx2: false,
            avx512f: false,
            cores: num_cpus::get_physical(),
            threads: num_cpus::get(),
        };

        // Memory information
        let memory_info = get_memory_info();

        // GPU information (optional, would require GPU API integration)
        let gpu_info = None;

        HardwareCapabilities {
            cpu_features,
            memory_info,
            gpu_info,
        }
    }

    #[cfg(target_os = "windows")]
    fn get_memory_info() -> MemoryInfo {
        use winapi::um::sysinfoapi::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..unsafe { std::mem::zeroed() }
        };

        unsafe {
            GlobalMemoryStatusEx(&mut status);
        }

        MemoryInfo {
            total_mb: status.ullTotalPhys / 1024 / 1024,
            available_mb: status.ullAvailPhys / 1024 / 1024,
            page_size: 4096, // Standard Windows page size
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn get_memory_info() -> MemoryInfo {
        #[cfg(target_os = "linux")]
        {
            use libc::{sysconf, _SC_AVPHYS_PAGES, _SC_PAGESIZE, _SC_PHYS_PAGES};

            let page_size = unsafe { sysconf(_SC_PAGESIZE) } as usize;
            let total_pages = unsafe { sysconf(_SC_PHYS_PAGES) } as u64;
            let available_pages = unsafe { sysconf(_SC_AVPHYS_PAGES) } as u64;

            return MemoryInfo {
                total_mb: (total_pages * page_size as u64) / 1024 / 1024,
                available_mb: (available_pages * page_size as u64) / 1024 / 1024,
                page_size,
            };
        }

        // Fallback for other platforms where sysconf constants are unavailable.
        MemoryInfo {
            total_mb: 0,
            available_mb: 0,
            page_size: 0,
        }
    }
}
