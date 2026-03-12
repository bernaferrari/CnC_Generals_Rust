//! # GPU Performance Monitor
//!
//! Advanced GPU performance monitoring and memory management system.

use super::{Result, VideoDeviceError};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

#[cfg(feature = "video")]
use wgpu::{Buffer, BufferDescriptor, BufferUsages, Device, QuerySet, QueryType, Queue};

#[cfg(feature = "video")]
use wgpu_profiler::{GpuProfiler, GpuProfilerSettings, GpuTimerQueryResult};

/// GPU performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuPerformanceMetrics {
    /// GPU utilization percentage (0.0 - 100.0)
    pub gpu_utilization: f32,
    /// GPU memory usage in bytes
    pub memory_used: u64,
    /// Total GPU memory in bytes
    pub memory_total: u64,
    /// GPU temperature in Celsius (if available)
    pub temperature: f32,
    /// GPU power consumption in watts (if available)
    pub power_consumption: f32,
    /// GPU clock speed in MHz
    pub core_clock: u32,
    /// Memory clock speed in MHz
    pub memory_clock: u32,
    /// Frame time in milliseconds
    pub frame_time_ms: f32,
    /// Frames per second
    pub fps: f32,
    /// Draw calls per frame
    pub draw_calls_per_frame: u32,
    /// Triangles per frame
    pub triangles_per_frame: u64,
    /// Vertex shader invocations
    pub vertex_shader_invocations: u64,
    /// Fragment shader invocations
    pub fragment_shader_invocations: u64,
    /// Compute shader invocations
    pub compute_shader_invocations: u64,
}

impl Default for GpuPerformanceMetrics {
    fn default() -> Self {
        Self {
            gpu_utilization: 0.0,
            memory_used: 0,
            memory_total: 0,
            temperature: 0.0,
            power_consumption: 0.0,
            core_clock: 0,
            memory_clock: 0,
            frame_time_ms: 16.67, // 60 FPS
            fps: 60.0,
            draw_calls_per_frame: 0,
            triangles_per_frame: 0,
            vertex_shader_invocations: 0,
            fragment_shader_invocations: 0,
            compute_shader_invocations: 0,
        }
    }
}

/// Memory allocation tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAllocation {
    /// Allocation ID
    pub id: u64,
    /// Size in bytes
    pub size: u64,
    /// Allocation type
    pub allocation_type: MemoryType,
    /// Timestamp when allocated
    pub allocated_at: SystemTime,
    /// Last access timestamp
    pub last_accessed: SystemTime,
    /// Number of times accessed
    pub access_count: u64,
    /// Associated resource name/description
    pub description: String,
}

/// Types of GPU memory allocations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryType {
    /// Vertex buffer
    VertexBuffer,
    /// Index buffer
    IndexBuffer,
    /// Uniform buffer
    UniformBuffer,
    /// Storage buffer
    StorageBuffer,
    /// Texture 2D
    Texture2D,
    /// Texture 3D
    Texture3D,
    /// Texture cube
    TextureCube,
    /// Render target
    RenderTarget,
    /// Depth buffer
    DepthBuffer,
    /// Query buffer
    QueryBuffer,
    /// Other/Unknown
    Other,
}

/// Performance event for profiling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceEvent {
    /// Event name/label
    pub name: String,
    /// Start time
    #[serde(skip, default = "instant_now")]
    pub start_time: Instant,
    /// Duration in microseconds
    pub duration_us: u64,
    /// Event type
    pub event_type: EventType,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Performance event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    /// Frame rendering
    Frame,
    /// Draw call
    Draw,
    /// Compute dispatch
    Compute,
    /// Buffer upload
    BufferUpload,
    /// Texture upload
    TextureUpload,
    /// Render pass
    RenderPass,
    /// Shader compilation
    ShaderCompilation,
    /// Pipeline state change
    PipelineChange,
    /// Memory allocation
    MemoryAllocation,
    /// Memory deallocation
    MemoryDeallocation,
}

fn instant_now() -> Instant {
    Instant::now()
}

/// Advanced performance monitor
pub struct PerformanceMonitor {
    /// Configuration
    config: PerformanceMonitorConfig,

    /// Current metrics
    current_metrics: Arc<RwLock<GpuPerformanceMetrics>>,

    /// Historical metrics (circular buffer)
    metrics_history: Arc<RwLock<VecDeque<(Instant, GpuPerformanceMetrics)>>>,

    /// Memory allocations tracker
    memory_allocations: Arc<RwLock<HashMap<u64, MemoryAllocation>>>,

    /// Performance events
    performance_events: Arc<RwLock<VecDeque<PerformanceEvent>>>,

    /// GPU profiler (if available)
    #[cfg(feature = "video")]
    gpu_profiler: Option<Arc<RwLock<GpuProfiler>>>,

    /// Timestamp query sets
    #[cfg(feature = "video")]
    timestamp_queries: Option<QuerySet>,

    /// Query buffers for readback
    #[cfg(feature = "video")]
    query_buffers: Vec<Buffer>,

    /// Frame timing
    last_frame_time: Arc<Mutex<Instant>>,
    frame_count: Arc<Mutex<u64>>,

    /// Next allocation ID
    next_allocation_id: Arc<Mutex<u64>>,

    /// Total memory usage tracking
    total_memory_used: Arc<Mutex<u64>>,
}

/// Configuration for performance monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMonitorConfig {
    /// Maximum number of historical metrics to keep
    pub max_history_entries: usize,
    /// Maximum number of performance events to keep
    pub max_performance_events: usize,
    /// Enable GPU timestamp queries
    pub enable_gpu_timing: bool,
    /// Enable memory tracking
    pub enable_memory_tracking: bool,
    /// Metrics collection interval in milliseconds
    pub metrics_interval_ms: u64,
    /// Enable detailed profiling
    pub enable_detailed_profiling: bool,
    /// Profile CPU events
    pub profile_cpu_events: bool,
    /// Profile GPU events
    pub profile_gpu_events: bool,
}

impl Default for PerformanceMonitorConfig {
    fn default() -> Self {
        Self {
            max_history_entries: 1000,
            max_performance_events: 10000,
            enable_gpu_timing: true,
            enable_memory_tracking: true,
            metrics_interval_ms: 100, // 10Hz
            enable_detailed_profiling: true,
            profile_cpu_events: true,
            profile_gpu_events: true,
        }
    }
}

impl PerformanceMonitor {
    /// Create a new performance monitor
    pub fn new(device: &Device, config: PerformanceMonitorConfig) -> Result<Self> {
        let max_history_entries = config.max_history_entries;
        let max_performance_events = config.max_performance_events;

        #[cfg(feature = "video")]
        let (gpu_profiler, timestamp_queries) = {
            let profiler = if config.enable_gpu_timing {
                let settings = GpuProfilerSettings {
                    max_num_pending_frames: 4,
                    ..GpuProfilerSettings::default()
                };
                Some(Arc::new(RwLock::new(
                    GpuProfiler::new(device, settings).map_err(|e| {
                        VideoDeviceError::InitializationFailed(format!(
                            "GPU profiler init failed: {:?}",
                            e
                        ))
                    })?,
                )))
            } else {
                None
            };

            let queries = if device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
                Some(device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("Performance Monitor Timestamps"),
                    ty: QueryType::Timestamp,
                    count: 256, // Max queries per frame
                }))
            } else {
                None
            };

            (profiler, queries)
        };

        Ok(Self {
            config,
            current_metrics: Arc::new(RwLock::new(GpuPerformanceMetrics::default())),
            metrics_history: Arc::new(RwLock::new(VecDeque::with_capacity(max_history_entries))),
            memory_allocations: Arc::new(RwLock::new(HashMap::new())),
            performance_events: Arc::new(RwLock::new(VecDeque::with_capacity(
                max_performance_events,
            ))),

            #[cfg(feature = "video")]
            gpu_profiler,
            #[cfg(feature = "video")]
            timestamp_queries,
            #[cfg(feature = "video")]
            query_buffers: Vec::new(),

            last_frame_time: Arc::new(Mutex::new(Instant::now())),
            frame_count: Arc::new(Mutex::new(0)),
            next_allocation_id: Arc::new(Mutex::new(1)),
            total_memory_used: Arc::new(Mutex::new(0)),
        })
    }

    /// Start frame profiling
    pub fn begin_frame(&self) {
        let now = Instant::now();
        let mut last_time = self.last_frame_time.lock();
        let frame_time = now.duration_since(*last_time);
        *last_time = now;

        let mut frame_count = self.frame_count.lock();
        *frame_count += 1;

        // Update frame metrics
        let mut metrics = self.current_metrics.write();
        metrics.frame_time_ms = frame_time.as_secs_f32() * 1000.0;
        metrics.fps = 1000.0 / metrics.frame_time_ms.max(0.001);

        // Add performance event
        if self.config.enable_detailed_profiling {
            self.add_event("Frame", EventType::Frame, HashMap::new());
        }
    }

    /// End frame profiling
    pub fn end_frame(&self) {
        // Collect metrics and add to history
        let current_metrics = self.current_metrics.read().clone();
        let mut history = self.metrics_history.write();

        history.push_back((Instant::now(), current_metrics));

        // Keep history size within limits
        while history.len() > self.config.max_history_entries {
            history.pop_front();
        }

        // Reset per-frame counters
        let mut metrics = self.current_metrics.write();
        metrics.draw_calls_per_frame = 0;
        metrics.triangles_per_frame = 0;
    }

    /// Record draw call
    pub fn record_draw_call(&self, triangle_count: u64) {
        let mut metrics = self.current_metrics.write();
        metrics.draw_calls_per_frame += 1;
        metrics.triangles_per_frame += triangle_count;
    }

    /// Record memory allocation
    pub fn record_allocation(
        &self,
        size: u64,
        allocation_type: MemoryType,
        description: String,
    ) -> u64 {
        if !self.config.enable_memory_tracking {
            return 0;
        }

        let mut next_id = self.next_allocation_id.lock();
        let id = *next_id;
        *next_id += 1;

        let allocation = MemoryAllocation {
            id,
            size,
            allocation_type,
            allocated_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
            access_count: 0,
            description,
        };

        self.memory_allocations.write().insert(id, allocation);

        // Update total memory usage
        *self.total_memory_used.lock() += size;

        // Update metrics
        let mut metrics = self.current_metrics.write();
        metrics.memory_used = *self.total_memory_used.lock();

        // Add event
        if self.config.enable_detailed_profiling {
            let mut metadata = HashMap::new();
            metadata.insert("size".to_string(), size.to_string());
            metadata.insert("type".to_string(), format!("{:?}", allocation_type));
            self.add_event("Memory Allocation", EventType::MemoryAllocation, metadata);
        }

        id
    }

    /// Record memory deallocation
    pub fn record_deallocation(&self, allocation_id: u64) {
        if !self.config.enable_memory_tracking {
            return;
        }

        if let Some(allocation) = self.memory_allocations.write().remove(&allocation_id) {
            // Update total memory usage
            *self.total_memory_used.lock() -= allocation.size;

            // Update metrics
            let mut metrics = self.current_metrics.write();
            metrics.memory_used = *self.total_memory_used.lock();

            // Add event
            if self.config.enable_detailed_profiling {
                let mut metadata = HashMap::new();
                metadata.insert("size".to_string(), allocation.size.to_string());
                metadata.insert("id".to_string(), allocation_id.to_string());
                self.add_event(
                    "Memory Deallocation",
                    EventType::MemoryDeallocation,
                    metadata,
                );
            }
        }
    }

    /// Get current performance metrics
    pub fn get_current_metrics(&self) -> GpuPerformanceMetrics {
        self.current_metrics.read().clone()
    }

    /// Get historical metrics
    pub fn get_metrics_history(&self) -> Vec<(Instant, GpuPerformanceMetrics)> {
        self.metrics_history.read().iter().cloned().collect()
    }

    /// Get memory allocations
    pub fn get_memory_allocations(&self) -> Vec<MemoryAllocation> {
        self.memory_allocations.read().values().cloned().collect()
    }

    /// Get performance events
    pub fn get_performance_events(&self) -> Vec<PerformanceEvent> {
        self.performance_events.read().iter().cloned().collect()
    }

    /// Get total memory usage by type
    pub fn get_memory_usage_by_type(&self) -> HashMap<MemoryType, u64> {
        let allocations = self.memory_allocations.read();
        let mut usage_by_type = HashMap::new();

        for allocation in allocations.values() {
            *usage_by_type.entry(allocation.allocation_type).or_insert(0) += allocation.size;
        }

        usage_by_type
    }

    /// Get average FPS over time period
    pub fn get_average_fps(&self, duration: Duration) -> f32 {
        let now = Instant::now();
        let cutoff = now - duration;

        let history = self.metrics_history.read();
        let relevant_metrics: Vec<_> = history
            .iter()
            .filter(|(timestamp, _)| *timestamp >= cutoff)
            .collect();

        if relevant_metrics.is_empty() {
            return 0.0;
        }

        let total_fps: f32 = relevant_metrics
            .iter()
            .map(|(_, metrics)| metrics.fps)
            .sum();
        total_fps / relevant_metrics.len() as f32
    }

    /// Get memory allocation statistics
    pub fn get_memory_statistics(&self) -> MemoryStatistics {
        let allocations = self.memory_allocations.read();
        let total_allocations = allocations.len() as u64;
        let total_size = allocations.values().map(|a| a.size).sum::<u64>();

        let mut by_type = HashMap::new();
        for allocation in allocations.values() {
            let entry = by_type
                .entry(allocation.allocation_type)
                .or_insert((0u64, 0u64));
            entry.0 += 1; // Count
            entry.1 += allocation.size; // Size
        }

        MemoryStatistics {
            total_allocations,
            total_size,
            allocations_by_type: by_type,
            fragmentation_ratio: self.calculate_fragmentation_ratio(),
        }
    }

    /// Clear performance history
    pub fn clear_history(&self) {
        self.metrics_history.write().clear();
        self.performance_events.write().clear();
    }

    /// Set GPU utilization (external monitoring)
    pub fn set_gpu_utilization(&self, utilization: f32) {
        self.current_metrics.write().gpu_utilization = utilization.clamp(0.0, 100.0);
    }

    /// Set GPU temperature (external monitoring)
    pub fn set_gpu_temperature(&self, temperature: f32) {
        self.current_metrics.write().temperature = temperature;
    }

    /// Set GPU power consumption (external monitoring)
    pub fn set_power_consumption(&self, power_watts: f32) {
        self.current_metrics.write().power_consumption = power_watts;
    }

    // Private helper methods

    fn add_event(&self, name: &str, event_type: EventType, metadata: HashMap<String, String>) {
        let event = PerformanceEvent {
            name: name.to_string(),
            start_time: Instant::now(),
            duration_us: 0, // Will be set when event completes
            event_type,
            metadata,
        };

        let mut events = self.performance_events.write();
        events.push_back(event);

        // Keep events within limit
        while events.len() > self.config.max_performance_events {
            events.pop_front();
        }
    }

    fn calculate_fragmentation_ratio(&self) -> f32 {
        // Simplified fragmentation calculation
        // In practice, this would analyze memory layout
        let allocations = self.memory_allocations.read();
        let total_allocations = allocations.len() as f32;

        if total_allocations < 2.0 {
            return 0.0;
        }

        // Simple heuristic: more allocations = more potential fragmentation
        (total_allocations / 100.0).min(1.0)
    }
}

/// Memory usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStatistics {
    /// Total number of allocations
    pub total_allocations: u64,
    /// Total memory allocated in bytes
    pub total_size: u64,
    /// Allocations by type (count, size)
    pub allocations_by_type: HashMap<MemoryType, (u64, u64)>,
    /// Memory fragmentation ratio (0.0 - 1.0)
    pub fragmentation_ratio: f32,
}

/// Scoped performance timer for automatic event recording
pub struct ScopedTimer<'a> {
    monitor: &'a PerformanceMonitor,
    event_name: String,
    event_type: EventType,
    start_time: Instant,
    metadata: HashMap<String, String>,
}

impl<'a> ScopedTimer<'a> {
    pub fn new(monitor: &'a PerformanceMonitor, name: String, event_type: EventType) -> Self {
        Self {
            monitor,
            event_name: name,
            event_type,
            start_time: Instant::now(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

impl<'a> Drop for ScopedTimer<'a> {
    fn drop(&mut self) {
        if self.monitor.config.enable_detailed_profiling {
            let duration = self.start_time.elapsed();
            let event = PerformanceEvent {
                name: self.event_name.clone(),
                start_time: self.start_time,
                duration_us: duration.as_micros() as u64,
                event_type: self.event_type,
                metadata: self.metadata.clone(),
            };

            let mut events = self.monitor.performance_events.write();
            events.push_back(event);

            while events.len() > self.monitor.config.max_performance_events {
                events.pop_front();
            }
        }
    }
}

// Helper macro for scoped timing
#[macro_export]
macro_rules! profile_scope {
    ($monitor:expr, $name:expr) => {
        let _timer = ScopedTimer::new($monitor, $name.to_string(), EventType::Frame);
    };
    ($monitor:expr, $name:expr, $event_type:expr) => {
        let _timer = ScopedTimer::new($monitor, $name.to_string(), $event_type);
    };
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryType::VertexBuffer => write!(f, "Vertex Buffer"),
            MemoryType::IndexBuffer => write!(f, "Index Buffer"),
            MemoryType::UniformBuffer => write!(f, "Uniform Buffer"),
            MemoryType::StorageBuffer => write!(f, "Storage Buffer"),
            MemoryType::Texture2D => write!(f, "Texture 2D"),
            MemoryType::Texture3D => write!(f, "Texture 3D"),
            MemoryType::TextureCube => write!(f, "Texture Cube"),
            MemoryType::RenderTarget => write!(f, "Render Target"),
            MemoryType::DepthBuffer => write!(f, "Depth Buffer"),
            MemoryType::QueryBuffer => write!(f, "Query Buffer"),
            MemoryType::Other => write!(f, "Other"),
        }
    }
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventType::Frame => write!(f, "Frame"),
            EventType::Draw => write!(f, "Draw"),
            EventType::Compute => write!(f, "Compute"),
            EventType::BufferUpload => write!(f, "Buffer Upload"),
            EventType::TextureUpload => write!(f, "Texture Upload"),
            EventType::RenderPass => write!(f, "Render Pass"),
            EventType::ShaderCompilation => write!(f, "Shader Compilation"),
            EventType::PipelineChange => write!(f, "Pipeline Change"),
            EventType::MemoryAllocation => write!(f, "Memory Allocation"),
            EventType::MemoryDeallocation => write!(f, "Memory Deallocation"),
        }
    }
}
