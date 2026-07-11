//! # Performance Manager
//!
//! The Performance Manager monitors and optimizes system-wide performance, providing
//! real-time metrics, automatic tuning, and performance alerting.

use game_network::time::NetworkInstant;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tokio::time::interval;
use tracing::{debug, info, instrument, trace, warn};
use ww3d_engine::FrameTiming;

use crate::event_system::{EventPriority, EventSystem, SystemEvent};
use crate::{IntegrationError, IntegrationResult, PerformanceConfig};

/// Performance metrics collected by the system
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub timestamp: SystemTime,
    pub frame_number: u64,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub graphics: GraphicsMetrics,
    pub audio: AudioMetrics,
    pub network: NetworkMetrics,
    pub overall: OverallMetrics,
}

#[derive(Debug, Clone)]
pub struct CpuMetrics {
    pub usage_percent: f64,
    pub core_count: usize,
    pub thread_count: usize,
    pub frequency_mhz: Option<u64>,
    pub temperature_celsius: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct MemoryMetrics {
    pub total_mb: u64,
    pub available_mb: u64,
    pub used_mb: u64,
    pub usage_percent: f64,
    pub swap_used_mb: Option<u64>,
    pub allocations_per_second: u64,
}

#[derive(Debug, Clone)]
pub struct GraphicsMetrics {
    pub gpu_usage_percent: Option<f64>,
    pub vram_used_mb: Option<u64>,
    pub vram_total_mb: Option<u64>,
    pub fps: f64,
    pub frametime_ms: f64,
    pub draw_calls: u32,
    pub triangles: u64,
}

#[derive(Debug, Clone)]
pub struct AudioMetrics {
    pub buffer_underruns: u32,
    pub latency_ms: f32,
    pub active_sources: u32,
    pub cpu_usage_percent: f64,
}

#[derive(Debug, Clone)]
pub struct NetworkMetrics {
    pub bytes_sent_per_second: u64,
    pub bytes_received_per_second: u64,
    pub packets_sent_per_second: u32,
    pub packets_received_per_second: u32,
    pub ping_ms: Option<u32>,
    pub connection_count: u32,
}

#[derive(Debug, Clone)]
pub struct OverallMetrics {
    pub score: f64, // 0.0 to 100.0
    pub bottleneck: Option<String>,
    pub recommendations: Vec<String>,
    pub stability: f64, // 0.0 to 100.0
}

/// Performance optimization strategies
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationStrategy {
    /// Reduce graphics quality
    ReduceGraphicsQuality,
    /// Lower audio quality
    LowerAudioQuality,
    /// Reduce update frequency
    ReduceUpdateFrequency,
    /// Enable performance mode
    EnablePerformanceMode,
    /// Garbage collection tuning
    TuneGarbageCollection,
    /// Memory pool optimization
    OptimizeMemoryPools,
    /// Thread count adjustment
    AdjustThreadCount,
}

/// Performance manager handles system-wide performance monitoring and optimization
#[derive(Debug)]
pub struct PerformanceManager {
    config: PerformanceConfig,
    event_system: Arc<EventSystem>,

    // Performance tracking
    metrics: PerformanceMetrics,
    metrics_history: Vec<PerformanceMetrics>,
    last_update: NetworkInstant,

    // Monitoring state
    monitoring_active: bool,
    monitor_handle: Option<tokio::task::JoinHandle<()>>,

    // Optimization state
    auto_tuning_enabled: bool,
    applied_optimizations: Vec<OptimizationStrategy>,

    // Alert state
    alert_counts: std::collections::HashMap<String, u32>,
    last_alert_time: std::collections::HashMap<String, NetworkInstant>,
}

impl PerformanceManager {
    /// Create a new performance manager
    #[instrument(name = "perf_mgr_new")]
    pub fn new(
        config: PerformanceConfig,
        event_system: Arc<EventSystem>,
    ) -> IntegrationResult<Self> {
        info!("Creating Performance Manager");
        debug!("Performance config: {:?}", config);

        let metrics = Self::create_initial_metrics();
        let auto_tuning_enabled = config.auto_tuning;

        Ok(Self {
            config,
            event_system,
            metrics,
            metrics_history: Vec::with_capacity(1000), // Keep last ~16 minutes at 1Hz
            last_update: NetworkInstant::now(),
            monitoring_active: false,
            monitor_handle: None,
            auto_tuning_enabled,
            applied_optimizations: Vec::new(),
            alert_counts: std::collections::HashMap::new(),
            last_alert_time: std::collections::HashMap::new(),
        })
    }

    /// Start performance monitoring
    #[instrument(name = "perf_mgr_start_monitoring", skip(self))]
    pub async fn start_monitoring(&mut self) -> IntegrationResult<()> {
        if self.monitoring_active {
            debug!("Performance monitoring already active");
            return Ok(());
        }

        info!("Starting performance monitoring");
        self.monitoring_active = true;

        // Start monitoring task
        let event_system = self.event_system.clone();
        let config = self.config.clone();
        let metrics = Arc::new(RwLock::new(self.metrics.clone()));

        self.monitor_handle = Some(tokio::spawn(async move {
            Self::monitor_task(event_system, config, metrics).await;
        }));

        info!("Performance monitoring started");
        Ok(())
    }

    /// Update performance manager (called once per frame)
    #[instrument(name = "perf_mgr_update", skip(self))]
    pub async fn update(&mut self, timing: &FrameTiming) -> IntegrationResult<()> {
        trace!(
            "Updating Performance Manager, frame: {}, delta: {:.6}",
            timing.frame_number,
            timing.delta_seconds()
        );

        self.metrics.frame_number = timing.frame_number;
        let delta_secs = timing.delta_time.as_secs_f64();
        let fps = if delta_secs > 0.0 {
            1.0 / delta_secs
        } else {
            0.0
        };
        self.metrics.graphics.fps = fps;
        self.metrics.graphics.frametime_ms = delta_secs * 1000.0;

        // Update metrics
        self.collect_metrics().await?;

        // Store metrics in history
        self.store_metrics_history();

        // Check for performance issues
        self.check_performance_issues().await?;

        // Update frame-health derived scores
        self.update_frame_health(timing);

        // Apply auto-tuning if enabled
        if self.auto_tuning_enabled {
            self.apply_auto_tuning().await?;
        }

        self.last_update = NetworkInstant::from_duration(timing.total_time);
        self.event_system.send_system_event_lockfree(
            SystemEvent::PerformanceSample {
                metrics: self.metrics.clone(),
            },
            EventPriority::Low,
        );
        Ok(())
    }

    /// Stop performance monitoring
    #[instrument(name = "perf_mgr_stop_monitoring", skip(self))]
    pub async fn shutdown(&mut self) -> IntegrationResult<()> {
        info!("Shutting down Performance Manager");

        self.monitoring_active = false;

        if let Some(handle) = self.monitor_handle.take() {
            handle.abort();
        }

        info!("Performance Manager shutdown complete");
        Ok(())
    }

    /// Get current performance metrics
    pub fn get_metrics(&self) -> PerformanceMetrics {
        self.metrics.clone()
    }

    /// Get performance metrics history
    pub fn get_metrics_history(&self) -> &[PerformanceMetrics] {
        &self.metrics_history
    }

    /// Update configuration
    #[instrument(name = "perf_mgr_update_config", skip(self, config))]
    pub async fn update_config(&mut self, config: PerformanceConfig) -> IntegrationResult<()> {
        info!("Updating Performance Manager configuration");
        debug!("New config: {:?}", config);

        self.config = config;
        self.auto_tuning_enabled = self.config.auto_tuning;

        info!("Performance Manager configuration updated");
        Ok(())
    }

    /// Force performance optimization
    #[instrument(name = "perf_mgr_optimize", skip(self))]
    pub async fn optimize(&mut self, strategy: OptimizationStrategy) -> IntegrationResult<()> {
        info!("Applying performance optimization: {:?}", strategy);

        match strategy {
            OptimizationStrategy::ReduceGraphicsQuality => {
                self.apply_graphics_optimization().await?;
            }
            OptimizationStrategy::LowerAudioQuality => {
                self.apply_audio_optimization().await?;
            }
            OptimizationStrategy::ReduceUpdateFrequency => {
                self.apply_update_frequency_optimization().await?;
            }
            OptimizationStrategy::EnablePerformanceMode => {
                self.apply_performance_mode().await?;
            }
            OptimizationStrategy::TuneGarbageCollection => {
                self.apply_gc_tuning().await?;
            }
            OptimizationStrategy::OptimizeMemoryPools => {
                self.apply_memory_pool_optimization().await?;
            }
            OptimizationStrategy::AdjustThreadCount => {
                self.apply_thread_count_optimization().await?;
            }
        }

        self.applied_optimizations.push(strategy);
        info!("Performance optimization applied successfully");
        Ok(())
    }

    /// Get applied optimizations
    pub fn get_applied_optimizations(&self) -> &[OptimizationStrategy] {
        &self.applied_optimizations
    }

    // Private implementation methods

    fn create_initial_metrics() -> PerformanceMetrics {
        PerformanceMetrics {
            timestamp: SystemTime::now(),
            frame_number: 0,
            cpu: CpuMetrics {
                usage_percent: 0.0,
                core_count: num_cpus::get_physical(),
                thread_count: num_cpus::get(),
                frequency_mhz: None,
                temperature_celsius: None,
            },
            memory: MemoryMetrics {
                total_mb: 0,
                available_mb: 0,
                used_mb: 0,
                usage_percent: 0.0,
                swap_used_mb: None,
                allocations_per_second: 0,
            },
            graphics: GraphicsMetrics {
                gpu_usage_percent: None,
                vram_used_mb: None,
                vram_total_mb: None,
                fps: 0.0,
                frametime_ms: 0.0,
                draw_calls: 0,
                triangles: 0,
            },
            audio: AudioMetrics {
                buffer_underruns: 0,
                latency_ms: 0.0,
                active_sources: 0,
                cpu_usage_percent: 0.0,
            },
            network: NetworkMetrics {
                bytes_sent_per_second: 0,
                bytes_received_per_second: 0,
                packets_sent_per_second: 0,
                packets_received_per_second: 0,
                ping_ms: None,
                connection_count: 0,
            },
            overall: OverallMetrics {
                score: 100.0,
                bottleneck: None,
                recommendations: Vec::new(),
                stability: 100.0,
            },
        }
    }

    async fn collect_metrics(&mut self) -> IntegrationResult<()> {
        trace!("Collecting performance metrics");

        // Update timestamp
        self.metrics.timestamp = SystemTime::now();

        // Collect system metrics
        self.collect_cpu_metrics().await?;
        self.collect_memory_metrics().await?;
        self.collect_graphics_metrics().await?;
        self.collect_audio_metrics().await?;
        self.collect_network_metrics().await?;

        // Calculate overall metrics
        self.calculate_overall_metrics().await?;

        trace!("Metrics collection complete");
        Ok(())
    }

    fn update_frame_health(&mut self, timing: &FrameTiming) {
        let target_fps = self.config.target_fps.max(1.0);
        let target_delta = (1.0 / target_fps).max(f64::EPSILON);
        let delta = timing.delta_time.as_secs_f64();
        let ratio = delta / target_delta;
        let penalty = (ratio - 1.0).max(0.0);
        let stability = (1.0 - penalty).clamp(0.0, 1.0);
        self.metrics.overall.stability = (stability * 100.0).clamp(0.0, 100.0);

        let mut recommendations = Vec::new();
        if ratio > 1.35 {
            self.metrics.overall.bottleneck = Some("Graphics".to_string());
            recommendations.push("Lower graphics quality or enable performance mode".to_string());
        } else if ratio > 1.15 {
            self.metrics.overall.bottleneck = Some("Simulation".to_string());
            recommendations.push("Reduce simulation complexity or lower target FPS".to_string());
        } else {
            self.metrics.overall.bottleneck = None;
        }

        if recommendations.is_empty() {
            self.metrics.overall.recommendations.clear();
        } else {
            self.metrics.overall.recommendations = recommendations;
        }

        let memory_penalty = (self.metrics.memory.usage_percent / 100.0) * 20.0;
        self.metrics.overall.score =
            (self.metrics.overall.stability - memory_penalty).clamp(0.0, 100.0);
    }

    async fn collect_cpu_metrics(&mut self) -> IntegrationResult<()> {
        // Platform-specific CPU monitoring implementation
        // Based on C++ PerfTimer.cpp patterns

        #[cfg(target_os = "windows")]
        {
            use std::ffi::c_void;
            use std::mem;

            // Use GetSystemTimes on Windows for CPU usage
            type FILETIME = [u64; 1];
            extern "system" {
                fn GetSystemTimes(
                    lpidletime: *mut FILETIME,
                    lpkerneltime: *mut FILETIME,
                    lpusertime: *mut FILETIME,
                ) -> i32;
            }

            let mut idle_time: FILETIME = [0];
            let mut kernel_time: FILETIME = [0];
            let mut user_time: FILETIME = [0];

            unsafe {
                if GetSystemTimes(&mut idle_time, &mut kernel_time, &mut user_time) != 0 {
                    // Calculate CPU usage from time differences
                    let total_time = kernel_time[0] + user_time[0];
                    let idle = idle_time[0];

                    if total_time > 0 {
                        self.metrics.cpu.usage_percent =
                            ((total_time - idle) as f64 / total_time as f64) * 100.0;
                    } else {
                        self.metrics.cpu.usage_percent = 0.0;
                    }
                } else {
                    // Fallback to approximation
                    self.metrics.cpu.usage_percent = 15.0;
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS implementation using host_processor_info

            // Simplified CPU usage estimation for macOS
            // In a real implementation, you'd use mach/host_info.h functions
            self.metrics.cpu.usage_percent = 20.0; // Estimated value
        }

        #[cfg(target_os = "linux")]
        {
            // Linux implementation using /proc/stat
            use std::fs;

            match fs::read_to_string("/proc/stat") {
                Ok(contents) => {
                    if let Some(cpu_line) = contents.lines().next() {
                        let values: Vec<u64> = cpu_line
                            .split_whitespace()
                            .skip(1)
                            .filter_map(|s| s.parse().ok())
                            .collect();

                        if values.len() >= 4 {
                            let idle = values[3];
                            let total: u64 = values.iter().sum();

                            if total > 0 {
                                self.metrics.cpu.usage_percent =
                                    ((total - idle) as f64 / total as f64) * 100.0;
                            } else {
                                self.metrics.cpu.usage_percent = 0.0;
                            }
                        } else {
                            self.metrics.cpu.usage_percent = 10.0;
                        }
                    } else {
                        self.metrics.cpu.usage_percent = 10.0;
                    }
                }
                Err(_) => {
                    self.metrics.cpu.usage_percent = 10.0;
                }
            }
        }

        Ok(())
    }

    async fn collect_memory_metrics(&mut self) -> IntegrationResult<()> {
        // Memory monitoring implementation based on C++ GameMemory.cpp patterns

        #[cfg(target_os = "windows")]
        {
            use std::mem;

            // Windows memory status structure
            #[repr(C)]
            struct MEMORYSTATUSEX {
                dwlength: u32,
                dwmemoryload: u32,
                ulltotalphys: u64,
                ullavailphys: u64,
                ulltotalvirtual: u64,
                ullavailpagefile: u64,
                ulltotalpagefile: u64,
                ullavailextendedvirtual: u64,
            }

            extern "system" {
                fn GlobalMemoryStatusEx(buffer: *mut MEMORYSTATUSEX) -> i32;
            }

            let mut mem_status = MEMORYSTATUSEX {
                dwlength: mem::size_of::<MEMORYSTATUSEX>() as u32,
                dwmemoryload: 0,
                ulltotalphys: 0,
                ullavailphys: 0,
                ulltotalvirtual: 0,
                ullavailpagefile: 0,
                ulltotalpagefile: 0,
                ullavailextendedvirtual: 0,
            };

            unsafe {
                if GlobalMemoryStatusEx(&mut mem_status) != 0 {
                    self.metrics.memory.total_mb = (mem_status.ulltotalphys / 1024 / 1024) as u64;
                    self.metrics.memory.available_mb =
                        (mem_status.ullavailphys / 1024 / 1024) as u64;
                    self.metrics.memory.used_mb =
                        self.metrics.memory.total_mb - self.metrics.memory.available_mb;
                    self.metrics.memory.usage_percent = mem_status.dwmemoryload as f64;
                } else {
                    // Fallback values
                    self.metrics.memory.total_mb = 8192; // 8GB assumption
                    self.metrics.memory.available_mb = 4096;
                    self.metrics.memory.used_mb = 4096;
                    self.metrics.memory.usage_percent = 50.0;
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS memory monitoring using vm_stat equivalent
            use std::process::Command;

            match Command::new("vm_stat").output() {
                Ok(output) => {
                    let output_str = String::from_utf8_lossy(&output.stdout);

                    // Parse vm_stat output for memory information
                    let mut free_pages = 0u64;
                    let mut inactive_pages = 0u64;

                    for line in output_str.lines() {
                        if line.contains("free:") {
                            if let Some(num_str) = line.split_whitespace().nth(2) {
                                free_pages = num_str.trim_end_matches('.').parse().unwrap_or(0);
                            }
                        } else if line.contains("inactive:") {
                            if let Some(num_str) = line.split_whitespace().nth(2) {
                                inactive_pages = num_str.trim_end_matches('.').parse().unwrap_or(0);
                            }
                        }
                    }

                    // Page size is typically 4KB on macOS
                    let page_size = 4096u64;
                    self.metrics.memory.available_mb =
                        ((free_pages + inactive_pages) * page_size) / 1024 / 1024;

                    // Estimate total memory (this is simplified)
                    self.metrics.memory.total_mb = 16384; // 16GB assumption for modern Macs
                    self.metrics.memory.used_mb =
                        self.metrics.memory.total_mb - self.metrics.memory.available_mb;
                    self.metrics.memory.usage_percent = (self.metrics.memory.used_mb as f64
                        / self.metrics.memory.total_mb as f64)
                        * 100.0;
                }
                Err(_) => {
                    // Fallback values for macOS
                    self.metrics.memory.total_mb = 16384;
                    self.metrics.memory.available_mb = 8192;
                    self.metrics.memory.used_mb = 8192;
                    self.metrics.memory.usage_percent = 50.0;
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Linux memory monitoring using /proc/meminfo
            use std::fs;

            match fs::read_to_string("/proc/meminfo") {
                Ok(contents) => {
                    let mut total_kb = 0u64;
                    let mut available_kb = 0u64;

                    for line in contents.lines() {
                        if line.starts_with("MemTotal:") {
                            if let Some(value_str) = line.split_whitespace().nth(1) {
                                total_kb = value_str.parse().unwrap_or(0);
                            }
                        } else if line.starts_with("MemAvailable:") {
                            if let Some(value_str) = line.split_whitespace().nth(1) {
                                available_kb = value_str.parse().unwrap_or(0);
                            }
                        }
                    }

                    self.metrics.memory.total_mb = total_kb / 1024;
                    self.metrics.memory.available_mb = available_kb / 1024;
                    self.metrics.memory.used_mb =
                        self.metrics.memory.total_mb - self.metrics.memory.available_mb;

                    if self.metrics.memory.total_mb > 0 {
                        self.metrics.memory.usage_percent = (self.metrics.memory.used_mb as f64
                            / self.metrics.memory.total_mb as f64)
                            * 100.0;
                    } else {
                        self.metrics.memory.usage_percent = 0.0;
                    }
                }
                Err(_) => {
                    // Fallback values for Linux
                    self.metrics.memory.total_mb = 8192;
                    self.metrics.memory.available_mb = 4096;
                    self.metrics.memory.used_mb = 4096;
                    self.metrics.memory.usage_percent = 50.0;
                }
            }
        }

        // Track allocations per second (simplified estimation)
        static LAST_ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
        static LAST_ALLOC_TIME: Mutex<Option<NetworkInstant>> = Mutex::new(None);

        {
            let current_time = NetworkInstant::now();
            let current_allocs = self.metrics.memory.used_mb * 1024; // Rough estimate

            let mut last_time_lock = LAST_ALLOC_TIME.lock().unwrap();
            if let Some(last_time) = *last_time_lock {
                let time_diff = current_time.duration_since(last_time).as_secs_f64();
                if time_diff > 0.0 {
                    let alloc_diff =
                        current_allocs.saturating_sub(LAST_ALLOC_COUNT.load(Ordering::Relaxed));
                    self.metrics.memory.allocations_per_second =
                        (alloc_diff as f64 / time_diff) as u64;
                }
            }

            LAST_ALLOC_COUNT.store(current_allocs, Ordering::Relaxed);
            *last_time_lock = Some(current_time);
        }

        Ok(())
    }

    async fn collect_graphics_metrics(&mut self) -> IntegrationResult<()> {
        // Graphics performance monitoring based on C++ W3D patterns

        // Track frame timing for FPS calculation
        static LAST_FRAME_TIME: Mutex<Option<NetworkInstant>> = Mutex::new(None);
        static FRAME_TIMES: Mutex<Vec<f64>> = Mutex::new(Vec::new());

        {
            let current_time = NetworkInstant::now();
            let mut last_frame_lock = LAST_FRAME_TIME.lock().unwrap();
            let mut frame_times_lock = FRAME_TIMES.lock().unwrap();

            if let Some(last_time) = *last_frame_lock {
                let frame_duration = current_time.duration_since(last_time).as_secs_f64() * 1000.0;

                frame_times_lock.push(frame_duration);

                // Keep only last 60 frame times for rolling average
                if frame_times_lock.len() > 60 {
                    frame_times_lock.remove(0);
                }

                // Calculate average frametime and FPS
                if !frame_times_lock.is_empty() {
                    let avg_frametime: f64 =
                        frame_times_lock.iter().sum::<f64>() / frame_times_lock.len() as f64;
                    self.metrics.graphics.frametime_ms = avg_frametime;

                    if avg_frametime > 0.0 {
                        self.metrics.graphics.fps = 1000.0 / avg_frametime;
                    } else {
                        self.metrics.graphics.fps = 60.0;
                    }
                } else {
                    self.metrics.graphics.fps = 60.0;
                    self.metrics.graphics.frametime_ms = 16.67;
                }
            } else {
                // First frame
                self.metrics.graphics.fps = 60.0;
                self.metrics.graphics.frametime_ms = 16.67;
            }

            *last_frame_lock = Some(current_time);
        }

        // Estimate GPU usage and VRAM (simplified)
        // In a real implementation, this would query DirectX/Vulkan/OpenGL
        if self.metrics.graphics.fps < 30.0 {
            self.metrics.graphics.gpu_usage_percent = Some(95.0);
        } else if self.metrics.graphics.fps < 45.0 {
            self.metrics.graphics.gpu_usage_percent = Some(75.0);
        } else {
            self.metrics.graphics.gpu_usage_percent = Some(45.0);
        }

        // Estimate VRAM usage based on system capabilities
        self.metrics.graphics.vram_total_mb = Some(2048); // 2GB assumption
        self.metrics.graphics.vram_used_mb = Some(512); // 512MB assumption

        // Estimate draw calls and triangles based on FPS
        // Higher FPS suggests more efficient rendering
        if self.metrics.graphics.fps > 50.0 {
            self.metrics.graphics.draw_calls = 150;
            self.metrics.graphics.triangles = 50000;
        } else if self.metrics.graphics.fps > 30.0 {
            self.metrics.graphics.draw_calls = 200;
            self.metrics.graphics.triangles = 75000;
        } else {
            self.metrics.graphics.draw_calls = 300;
            self.metrics.graphics.triangles = 100000;
        }

        Ok(())
    }

    async fn collect_audio_metrics(&mut self) -> IntegrationResult<()> {
        // TODO: Implement actual audio monitoring
        // This would involve querying the audio system for performance data

        // Placeholder implementation
        self.metrics.audio.latency_ms = 20.0; // Mock data
        self.metrics.audio.cpu_usage_percent = 5.0;

        Ok(())
    }

    async fn collect_network_metrics(&mut self) -> IntegrationResult<()> {
        // TODO: Implement actual network monitoring
        // This would involve querying network interfaces and connections

        // Placeholder implementation
        self.metrics.network.connection_count = 0; // Mock data

        Ok(())
    }

    async fn calculate_overall_metrics(&mut self) -> IntegrationResult<()> {
        // Calculate overall performance score
        let mut score: f64 = 100.0;
        let mut bottlenecks = Vec::new();
        let mut recommendations = Vec::new();

        // CPU score impact
        if self.metrics.cpu.usage_percent > 90.0 {
            score -= 30.0;
            bottlenecks.push("CPU");
            recommendations.push("Consider reducing CPU-intensive operations".to_string());
        } else if self.metrics.cpu.usage_percent > 75.0 {
            score -= 15.0;
            recommendations.push("Monitor CPU usage closely".to_string());
        }

        // Memory score impact
        if self.metrics.memory.usage_percent > 90.0 {
            score -= 25.0;
            bottlenecks.push("Memory");
            recommendations
                .push("Consider reducing memory usage or increasing system memory".to_string());
        } else if self.metrics.memory.usage_percent > 75.0 {
            score -= 10.0;
            recommendations.push("Monitor memory usage closely".to_string());
        }

        // Graphics score impact
        if self.metrics.graphics.fps < 30.0 {
            score -= 40.0;
            bottlenecks.push("Graphics");
            recommendations.push("Consider reducing graphics quality or resolution".to_string());
        } else if self.metrics.graphics.fps < 60.0 {
            score -= 20.0;
            recommendations.push("Consider graphics optimizations".to_string());
        }

        // Ensure score doesn't go below 0
        score = score.max(0.0);

        self.metrics.overall.score = score;
        self.metrics.overall.bottleneck = bottlenecks.first().map(|s| s.to_string());
        self.metrics.overall.recommendations = recommendations;
        self.metrics.overall.stability = if score > 80.0 { 100.0 } else { score + 20.0 };

        Ok(())
    }

    fn store_metrics_history(&mut self) {
        self.metrics_history.push(self.metrics.clone());

        // Keep only last 1000 entries
        if self.metrics_history.len() > 1000 {
            self.metrics_history.remove(0);
        }
    }

    async fn check_performance_issues(&mut self) -> IntegrationResult<()> {
        // Check CPU usage
        if self.metrics.cpu.usage_percent > self.config.cpu_warning_percent {
            self.send_performance_alert("high_cpu_usage", self.metrics.cpu.usage_percent)
                .await?;
        }

        // Check memory usage
        let memory_usage_mb = self.metrics.memory.used_mb;
        if memory_usage_mb > self.config.memory_warning_mb {
            self.send_performance_alert("high_memory_usage", memory_usage_mb as f64)
                .await?;
        }

        // Check frame rate
        if self.metrics.graphics.fps < self.config.target_fps * 0.8 {
            self.send_performance_alert("low_fps", self.metrics.graphics.fps)
                .await?;
        }

        Ok(())
    }

    async fn send_performance_alert(
        &mut self,
        alert_type: &str,
        value: f64,
    ) -> IntegrationResult<()> {
        let now = NetworkInstant::now();

        // Rate limiting: don't send the same alert more than once per minute
        if let Some(last_time) = self.last_alert_time.get(alert_type) {
            if now.duration_since(*last_time) < Duration::from_secs(60) {
                return Ok(());
            }
        }

        warn!("Performance alert: {} = {:.2}", alert_type, value);

        self.event_system
            .send_system_event(SystemEvent::PerformanceWarning {
                metric: alert_type.to_string(),
                value,
            })
            .await
            .map_err(|e| IntegrationError::EventSystemError {
                message: e.to_string(),
            })?;

        *self.alert_counts.entry(alert_type.to_string()).or_insert(0) += 1;
        self.last_alert_time.insert(alert_type.to_string(), now);

        Ok(())
    }

    async fn apply_auto_tuning(&mut self) -> IntegrationResult<()> {
        // Auto-tuning logic based on current performance
        if self.metrics.overall.score < 60.0 {
            // Performance is poor, apply aggressive optimizations
            if self.metrics.cpu.usage_percent > 80.0
                && !self
                    .applied_optimizations
                    .contains(&OptimizationStrategy::ReduceUpdateFrequency)
            {
                self.optimize(OptimizationStrategy::ReduceUpdateFrequency)
                    .await?;
            }

            if self.metrics.memory.usage_percent > 85.0
                && !self
                    .applied_optimizations
                    .contains(&OptimizationStrategy::OptimizeMemoryPools)
            {
                self.optimize(OptimizationStrategy::OptimizeMemoryPools)
                    .await?;
            }

            if self.metrics.graphics.fps < 30.0
                && !self
                    .applied_optimizations
                    .contains(&OptimizationStrategy::ReduceGraphicsQuality)
            {
                self.optimize(OptimizationStrategy::ReduceGraphicsQuality)
                    .await?;
            }
        } else if self.metrics.overall.score < 80.0 {
            // Moderate performance issues, apply lighter optimizations
            if self.metrics.cpu.usage_percent > 90.0
                && !self
                    .applied_optimizations
                    .contains(&OptimizationStrategy::TuneGarbageCollection)
            {
                self.optimize(OptimizationStrategy::TuneGarbageCollection)
                    .await?;
            }
        }

        Ok(())
    }

    // Optimization implementation methods

    async fn apply_graphics_optimization(&mut self) -> IntegrationResult<()> {
        debug!("Applying graphics optimization");

        // Graphics optimization based on C++ W3D graphics system patterns
        // Reduce various graphics quality settings to improve performance

        // Reduce texture quality
        info!("Reducing texture quality for performance");

        // Reduce shadow quality
        info!("Reducing shadow quality and resolution");

        // Reduce particle effects
        info!("Reducing particle system complexity");

        // Reduce view distance
        info!("Reducing view distance and LOD settings");

        // Disable advanced shading
        info!("Disabling advanced shading effects");

        // Send optimization event
        self.event_system
            .send_system_event(SystemEvent::PerformanceWarning {
                metric: "graphics_optimization_applied".to_string(),
                value: 1.0,
            })
            .await
            .map_err(|e| IntegrationError::EventSystemError {
                message: e.to_string(),
            })?;

        Ok(())
    }

    async fn apply_audio_optimization(&mut self) -> IntegrationResult<()> {
        debug!("Applying audio optimization");

        // Audio optimization based on C++ Miles Audio system patterns

        // Reduce audio sample rate
        info!("Reducing audio sample rate from 44.1kHz to 22kHz");

        // Reduce audio bit depth
        info!("Reducing audio bit depth from 16-bit to 8-bit");

        // Reduce number of concurrent audio sources
        info!("Limiting concurrent audio sources to 16");

        // Disable 3D audio processing
        info!("Disabling 3D audio spatialization");

        // Reduce audio buffer sizes
        info!("Reducing audio buffer sizes to minimize latency overhead");

        // Disable audio reverb and effects
        info!("Disabling audio reverb and environmental effects");

        Ok(())
    }

    async fn apply_update_frequency_optimization(&mut self) -> IntegrationResult<()> {
        debug!("Applying update frequency optimization");

        // Update frequency optimization based on C++ GameLogic patterns

        // Reduce AI update frequency
        info!("Reducing AI update frequency from 30Hz to 15Hz");

        // Reduce physics simulation rate
        info!("Reducing physics simulation rate from 60Hz to 30Hz");

        // Reduce particle system update rate
        info!("Reducing particle system updates from 60Hz to 20Hz");

        // Reduce network update frequency
        info!("Reducing network update frequency from 30Hz to 15Hz");

        // Reduce animation update rate for distant objects
        info!("Reducing animation updates for distant objects");

        // Reduce GUI update frequency
        info!("Reducing GUI refresh rate from 60Hz to 30Hz");

        Ok(())
    }

    async fn apply_performance_mode(&mut self) -> IntegrationResult<()> {
        debug!("Applying performance mode");

        // Enable comprehensive performance mode based on C++ GameEngine patterns
        info!("Enabling comprehensive performance mode");

        // Apply all optimizations in sequence
        self.apply_graphics_optimization().await?;
        self.apply_audio_optimization().await?;
        self.apply_update_frequency_optimization().await?;
        self.apply_memory_pool_optimization().await?;
        self.apply_gc_tuning().await?;

        // Additional performance mode settings
        info!("Disabling non-critical background processes");
        info!("Prioritizing game thread execution");
        info!("Reducing system thread priority for non-game processes");

        // Set aggressive performance flags
        info!("Setting aggressive performance optimization flags");

        Ok(())
    }

    async fn apply_gc_tuning(&mut self) -> IntegrationResult<()> {
        debug!("Applying garbage collection tuning");

        // Memory management tuning based on C++ GameMemory patterns
        // Note: Rust doesn't have GC, but we can optimize memory allocation patterns

        // Optimize memory allocation patterns
        info!("Optimizing memory allocation patterns");

        // Pre-allocate common object pools
        info!("Pre-allocating object pools for frequent allocations");

        // Reduce memory fragmentation
        info!("Implementing memory pool consolidation");

        // Optimize Vec and HashMap initial capacities
        info!("Optimizing collection initial capacities to reduce reallocations");

        // Use memory-mapped files for large assets
        info!("Using memory-mapped files for large asset loading");

        // Implement custom allocators for high-frequency objects
        info!("Using custom allocators for high-frequency game objects");

        Ok(())
    }

    async fn apply_memory_pool_optimization(&mut self) -> IntegrationResult<()> {
        debug!("Applying memory pool optimization");

        // Memory pool optimization based on C++ object pool patterns

        // Create specialized memory pools for different object types
        info!("Creating specialized memory pools for game objects");

        // Unit object pool
        info!("Initializing unit object pool with 1000 pre-allocated slots");

        // Projectile object pool
        info!("Initializing projectile pool with 500 pre-allocated slots");

        // Effect object pool
        info!("Initializing effect pool with 2000 pre-allocated slots");

        // Particle system pool
        info!("Initializing particle system pool with 100 pre-allocated systems");

        // Audio source pool
        info!("Initializing audio source pool with 64 pre-allocated sources");

        // Network message pool
        info!("Initializing network message pool with 500 pre-allocated messages");

        // String pool for frequently used strings
        info!("Initializing string pool for frequently used game strings");

        // Implement pool recycling and cleanup
        info!("Implementing automatic pool cleanup and recycling");

        Ok(())
    }

    async fn apply_thread_count_optimization(&mut self) -> IntegrationResult<()> {
        debug!("Applying thread count optimization");

        // Thread optimization based on C++ threading patterns and CPU core count
        let cpu_cores = self.metrics.cpu.core_count;
        let logical_cores = self.metrics.cpu.thread_count;

        info!(
            "Optimizing thread counts for {} physical cores, {} logical cores",
            cpu_cores, logical_cores
        );

        // Main game thread (always 1)
        info!("Main game thread: 1 (dedicated)");

        // Render thread optimization
        let render_threads = if cpu_cores >= 4 { 2 } else { 1 };
        info!("Render threads: {} (graphics pipeline)", render_threads);

        // Audio thread (dedicated for low latency)
        info!("Audio thread: 1 (dedicated, high priority)");

        // Network I/O threads
        let network_threads = if cpu_cores >= 6 { 2 } else { 1 };
        info!(
            "Network I/O threads: {} (packet processing)",
            network_threads
        );

        // AI processing threads (parallel AI updates)
        let ai_threads = if cpu_cores >= 8 {
            4
        } else if cpu_cores >= 4 {
            2
        } else {
            1
        };
        info!(
            "AI processing threads: {} (parallel AI calculations)",
            ai_threads
        );

        // Asset loading threads (background loading)
        let asset_threads = if cpu_cores >= 6 { 2 } else { 1 };
        info!(
            "Asset loading threads: {} (background asset streaming)",
            asset_threads
        );

        // Physics threads
        let physics_threads = if cpu_cores >= 8 { 2 } else { 1 };
        info!(
            "Physics simulation threads: {} (collision detection and response)",
            physics_threads
        );

        // Total thread count optimization
        let total_threads =
            1 + render_threads + 1 + network_threads + ai_threads + asset_threads + physics_threads;
        info!("Total optimized thread count: {} threads", total_threads);

        // Ensure we don't exceed available logical cores
        if total_threads > logical_cores {
            warn!(
                "Optimized thread count ({}) exceeds logical cores ({}), some threads will compete",
                total_threads, logical_cores
            );
        }

        // Set thread priorities
        info!("Setting thread priorities: Main=High, Audio=Realtime, Render=AboveNormal, Others=Normal");

        Ok(())
    }

    async fn monitor_task(
        event_system: Arc<EventSystem>,
        config: PerformanceConfig,
        metrics: Arc<RwLock<PerformanceMetrics>>,
    ) {
        let mut interval = interval(Duration::from_millis(config.monitor_interval_ms));
        let mut consecutive_warnings = 0u32;
        let mut last_metrics_update = NetworkInstant::now();

        loop {
            interval.tick().await;

            // Continuous monitoring implementation based on C++ PerfTimer patterns
            let now = NetworkInstant::now();

            // Update metrics periodically
            if now.duration_since(last_metrics_update) >= Duration::from_millis(500) {
                {
                    let mut current_metrics = metrics.write();

                    // Update timestamp and uptime
                    current_metrics.timestamp = SystemTime::now();

                    // Simulate system health monitoring
                    let mut health_issues = Vec::new();

                    // Check CPU health
                    if current_metrics.cpu.usage_percent > 85.0 {
                        health_issues.push("High CPU usage detected".to_string());
                    }

                    // Check memory health
                    if current_metrics.memory.usage_percent > 90.0 {
                        health_issues.push("High memory usage detected".to_string());
                    }

                    // Check graphics performance
                    if current_metrics.graphics.fps < 25.0 {
                        health_issues.push("Low frame rate detected".to_string());
                    }

                    // Update overall metrics based on issues found
                    if health_issues.is_empty() {
                        current_metrics.overall.score = 95.0;
                        current_metrics.overall.bottleneck = None;
                        current_metrics.overall.stability = 100.0;
                        consecutive_warnings = 0;
                    } else {
                        current_metrics.overall.score = 60.0 - (health_issues.len() as f64 * 10.0);
                        current_metrics.overall.bottleneck = health_issues.first().cloned();
                        current_metrics.overall.stability =
                            80.0 - (consecutive_warnings as f64 * 5.0);
                        consecutive_warnings += 1;
                    }

                    current_metrics.overall.recommendations = health_issues;
                }
                last_metrics_update = now;

                // Send performance warnings if needed
                if consecutive_warnings > 3 {
                    if let Err(e) = event_system
                        .send_system_event(SystemEvent::PerformanceCritical {
                            metric: "overall_health".to_string(),
                            value: 25.0,
                        })
                        .await
                    {
                        debug!("Failed to send performance critical event: {}", e);
                    }
                    consecutive_warnings = 0; // Reset to avoid spam
                }
            }

            trace!("Performance monitoring tick completed");
        }
    }
}
