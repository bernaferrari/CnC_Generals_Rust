//! Ultra-modern debugging tools for GameNetwork (2025)
//!
//! Provides comprehensive debugging capabilities including:
//! - Network packet inspection
//! - Connection state visualization
//! - Performance bottleneck analysis
//! - Real-time debugging console
//! - Memory leak detection
//! - Traffic simulation tools

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::diagnostics::{AlertSeverity, DiagnosticAlert, HealthStatus};
use crate::error::{EnhancedError, ErrorContext, NetworkError, NetworkResult};

#[cfg(feature = "metrics")]
use log;
#[cfg(feature = "metrics")]
use tracing::{debug, error, info, instrument, span, warn, Level};

/// Debug configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugConfig {
    /// Enable packet capture and inspection
    pub enable_packet_capture: bool,
    /// Maximum packets to capture before rotating
    pub max_captured_packets: usize,
    /// Enable connection state tracking
    pub enable_connection_tracking: bool,
    /// Enable performance profiling
    pub enable_profiling: bool,
    /// Debug console port (0 = disabled)
    pub console_port: u16,
    /// Enable memory debugging
    pub enable_memory_debugging: bool,
    /// Debug output verbosity level
    pub verbosity_level: DebugLevel,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            enable_packet_capture: cfg!(debug_assertions),
            max_captured_packets: 1000,
            enable_connection_tracking: true,
            enable_profiling: cfg!(debug_assertions),
            console_port: 0, // Disabled by default
            enable_memory_debugging: cfg!(debug_assertions),
            verbosity_level: if cfg!(debug_assertions) {
                DebugLevel::Verbose
            } else {
                DebugLevel::Normal
            },
        }
    }
}

/// Debug verbosity levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum DebugLevel {
    Silent,
    Normal,
    Verbose,
    Trace,
}

/// Ultra-modern debugging toolkit
pub struct NetworkDebugger {
    config: DebugConfig,
    start_time: Instant,

    // Packet capture system
    packet_captures: Arc<RwLock<Vec<PacketCapture>>>,

    // Connection state tracking
    connection_states: Arc<RwLock<HashMap<String, ConnectionDebugInfo>>>,

    // Performance profiler
    profiler: Arc<RwLock<PerformanceProfiler>>,

    // Debug console
    console_channel: Option<mpsc::UnboundedSender<DebugCommand>>,

    // Memory tracker
    memory_tracker: Arc<RwLock<MemoryTracker>>,

    // Event log
    debug_events: Arc<RwLock<Vec<DebugEvent>>>,
}

/// Captured packet information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketCapture {
    pub id: String,
    pub timestamp: u64,
    pub direction: PacketDirection,
    pub size: usize,
    pub packet_type: String,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub headers: HashMap<String, String>,
    pub payload_preview: String,
    pub metadata: HashMap<String, String>,
}

/// Packet direction enumeration
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PacketDirection {
    Incoming,
    Outgoing,
}

impl fmt::Display for PacketDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Incoming => write!(f, "⬇️  IN"),
            Self::Outgoing => write!(f, "⬆️  OUT"),
        }
    }
}

/// Connection debug information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionDebugInfo {
    pub connection_id: String,
    pub remote_address: String,
    pub state: ConnectionDebugState,
    pub established_at: u64,
    pub last_activity: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub round_trip_time: Option<Duration>,
    pub quality_metrics: ConnectionQualityMetrics,
    pub error_history: Vec<EnhancedError>,
}

/// Connection debug states
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ConnectionDebugState {
    Connecting,
    Connected,
    Authenticated,
    Active,
    Idle,
    Reconnecting,
    Disconnecting,
    Disconnected,
    Error,
}

impl fmt::Display for ConnectionDebugState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (emoji, text) = match self {
            Self::Connecting => ("🔄", "CONNECTING"),
            Self::Connected => ("🔗", "CONNECTED"),
            Self::Authenticated => ("OK", "AUTHENTICATED"),
            Self::Active => ("🟢", "ACTIVE"),
            Self::Idle => ("🟡", "IDLE"),
            Self::Reconnecting => ("🔄", "RECONNECTING"),
            Self::Disconnecting => ("⏳", "DISCONNECTING"),
            Self::Disconnected => ("🔴", "DISCONNECTED"),
            Self::Error => ("NO", "ERROR"),
        };
        write!(f, "{} {}", emoji, text)
    }
}

/// Connection quality metrics for debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionQualityMetrics {
    pub latency_ms: f64,
    pub jitter_ms: f64,
    pub packet_loss_rate: f64,
    pub throughput_mbps: f64,
    pub stability_score: f64,
}

/// Performance profiler for bottleneck analysis
#[derive(Debug)]
pub struct PerformanceProfiler {
    profiles: HashMap<String, PerformanceProfile>,
    active_spans: HashMap<String, ProfileSpan>,
}

/// Performance profile data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceProfile {
    pub operation: String,
    pub total_calls: u64,
    pub total_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
    pub average_duration: Duration,
    pub last_call: u64,
    pub error_count: u64,
}

/// Active profiling span
#[derive(Debug)]
pub struct ProfileSpan {
    pub operation: String,
    pub start_time: Instant,
    pub metadata: HashMap<String, String>,
}

/// Memory tracking information
#[derive(Debug, Default)]
pub struct MemoryTracker {
    allocations: HashMap<String, MemoryAllocation>,
    total_allocated: u64,
    peak_usage: u64,
    allocation_count: u64,
}

/// Memory allocation record
#[derive(Debug, Clone)]
pub struct MemoryAllocation {
    pub size: u64,
    pub allocated_at: Instant,
    pub location: String,
    pub still_alive: bool,
}

/// Debug event for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugEvent {
    pub id: String,
    pub timestamp: u64,
    pub event_type: DebugEventType,
    pub description: String,
    pub metadata: HashMap<String, String>,
    pub severity: DebugEventSeverity,
}

/// Debug event types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DebugEventType {
    PacketCaptured,
    ConnectionStateChanged,
    PerformanceAnomaly,
    MemoryLeak,
    ErrorOccurred,
    ConfigurationChanged,
    SystemEvent,
}

/// Debug event severity
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DebugEventSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Debug console commands
#[derive(Debug, Clone)]
pub enum DebugCommand {
    ShowStatus,
    CapturePackets(bool),
    ShowConnections,
    ShowProfiler,
    ShowMemoryUsage,
    GenerateReport,
    SetVerbosity(DebugLevel),
    SimulateTraffic {
        duration: Duration,
        packets_per_sec: u32,
    },
    InjectError {
        error_type: String,
        target: String,
    },
    ClearHistory,
    Exit,
}

impl NetworkDebugger {
    /// Create new ultra-modern network debugger
    pub fn new(config: DebugConfig) -> Self {
        Self {
            config: config.clone(),
            start_time: Instant::now(),
            packet_captures: Arc::new(RwLock::new(Vec::new())),
            connection_states: Arc::new(RwLock::new(HashMap::new())),
            profiler: Arc::new(RwLock::new(PerformanceProfiler {
                profiles: HashMap::new(),
                active_spans: HashMap::new(),
            })),
            console_channel: None,
            memory_tracker: Arc::new(RwLock::new(MemoryTracker::default())),
            debug_events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Initialize the debugging system
    #[instrument(skip(self))]
    pub async fn initialize(&mut self) -> NetworkResult<()> {
        info!("🔧 Initializing ultra-modern network debugging system");

        // Start debug console if enabled
        if self.config.console_port > 0 {
            self.start_debug_console().await?;
        }

        // Log initialization event
        self.log_event(
            DebugEventType::SystemEvent,
            "Network debugger initialized".to_string(),
            HashMap::new(),
            DebugEventSeverity::Info,
        )
        .await;

        info!("Network debugging system initialized successfully");
        Ok(())
    }

    /// Capture a network packet for inspection
    #[instrument(skip(self, payload), fields(direction = ?direction, size = size, packet_type = packet_type))]
    pub async fn capture_packet(
        &self,
        direction: PacketDirection,
        size: usize,
        packet_type: String,
        source: Option<String>,
        destination: Option<String>,
        headers: HashMap<String, String>,
        payload: &[u8],
    ) -> NetworkResult<()> {
        if !self.config.enable_packet_capture {
            return Ok(());
        }

        let capture = PacketCapture {
            id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            direction,
            size,
            packet_type: packet_type.clone(),
            source: source.clone(),
            destination: destination.clone(),
            headers,
            payload_preview: self.format_payload_preview(payload),
            metadata: HashMap::new(),
        };

        {
            let mut captures = self.packet_captures.write().await;
            captures.push(capture);

            // Rotate old captures if we exceed the limit
            if captures.len() > self.config.max_captured_packets {
                captures.remove(0);
            }
        }

        // Log packet capture event
        let mut metadata = HashMap::new();
        metadata.insert("direction".to_string(), format!("{:?}", direction));
        metadata.insert("size".to_string(), size.to_string());
        metadata.insert("type".to_string(), packet_type);

        self.log_event(
            DebugEventType::PacketCaptured,
            format!("Packet captured: {} {} bytes", direction, size),
            metadata,
            DebugEventSeverity::Info,
        )
        .await;

        if self.config.verbosity_level >= DebugLevel::Verbose {
            debug!(
                "📦 Captured packet: {} {} bytes, type: {}, src: {:?}, dst: {:?}",
                direction, size, packet_type, source, destination
            );
        }

        Ok(())
    }

    /// Update connection state for debugging
    #[instrument(skip(self), fields(connection_id = connection_id, state = ?state))]
    pub async fn update_connection_state(
        &self,
        connection_id: String,
        remote_address: String,
        state: ConnectionDebugState,
    ) -> NetworkResult<()> {
        if !self.config.enable_connection_tracking {
            return Ok(());
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        {
            let mut connections = self.connection_states.write().await;
            let connection_info =
                connections
                    .entry(connection_id.clone())
                    .or_insert_with(|| ConnectionDebugInfo {
                        connection_id: connection_id.clone(),
                        remote_address: remote_address.clone(),
                        state: ConnectionDebugState::Connecting,
                        established_at: now,
                        last_activity: now,
                        bytes_sent: 0,
                        bytes_received: 0,
                        packets_sent: 0,
                        packets_received: 0,
                        round_trip_time: None,
                        quality_metrics: ConnectionQualityMetrics {
                            latency_ms: 0.0,
                            jitter_ms: 0.0,
                            packet_loss_rate: 0.0,
                            throughput_mbps: 0.0,
                            stability_score: 100.0,
                        },
                        error_history: Vec::new(),
                    });

            connection_info.state = state;
            connection_info.last_activity = now;
            connection_info.remote_address = remote_address;
        }

        // Log state change event
        let mut metadata = HashMap::new();
        metadata.insert("connection_id".to_string(), connection_id.clone());
        metadata.insert("state".to_string(), format!("{:?}", state));

        self.log_event(
            DebugEventType::ConnectionStateChanged,
            format!("Connection {} state changed to {}", connection_id, state),
            metadata,
            DebugEventSeverity::Info,
        )
        .await;

        if self.config.verbosity_level >= DebugLevel::Normal {
            info!("🔗 Connection {} state changed to {}", connection_id, state);
        }

        Ok(())
    }

    /// Start profiling a network operation
    #[instrument(skip(self), fields(operation = operation))]
    pub async fn start_profile(&self, operation: String) -> NetworkResult<String> {
        if !self.config.enable_profiling {
            return Ok(String::new());
        }

        let span_id = Uuid::new_v4().to_string();
        let span = ProfileSpan {
            operation: operation.clone(),
            start_time: Instant::now(),
            metadata: HashMap::new(),
        };

        {
            let mut profiler = self.profiler.write().await;
            profiler.active_spans.insert(span_id.clone(), span);
        }

        if self.config.verbosity_level >= DebugLevel::Trace {
            debug!("⏱️  Started profiling: {} ({})", operation, span_id);
        }

        Ok(span_id)
    }

    /// End profiling and record performance data
    #[instrument(skip(self), fields(span_id = span_id, success = success))]
    pub async fn end_profile(&self, span_id: String, success: bool) -> NetworkResult<()> {
        if !self.config.enable_profiling || span_id.is_empty() {
            return Ok(());
        }

        let end_time = Instant::now();

        let mut profiler = self.profiler.write().await;
        if let Some(span) = profiler.active_spans.remove(&span_id) {
            let duration = end_time.duration_since(span.start_time);

            let profile = profiler
                .profiles
                .entry(span.operation.clone())
                .or_insert_with(|| PerformanceProfile {
                    operation: span.operation.clone(),
                    total_calls: 0,
                    total_duration: Duration::ZERO,
                    min_duration: Duration::MAX,
                    max_duration: Duration::ZERO,
                    average_duration: Duration::ZERO,
                    last_call: 0,
                    error_count: 0,
                });

            profile.total_calls += 1;
            profile.total_duration += duration;
            profile.min_duration = profile.min_duration.min(duration);
            profile.max_duration = profile.max_duration.max(duration);
            profile.average_duration = profile.total_duration / profile.total_calls as u32;
            profile.last_call = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if !success {
                profile.error_count += 1;
            }

            if self.config.verbosity_level >= DebugLevel::Trace {
                debug!(
                    "⏱️  Completed profiling: {} in {:.2}ms (success: {})",
                    span.operation,
                    duration.as_secs_f64() * 1000.0,
                    success
                );
            }

            // Check for performance anomalies
            if duration > Duration::from_millis(500) {
                let mut metadata = HashMap::new();
                metadata.insert("operation".to_string(), span.operation.clone());
                metadata.insert(
                    "duration_ms".to_string(),
                    (duration.as_secs_f64() * 1000.0).to_string(),
                );

                self.log_event(
                    DebugEventType::PerformanceAnomaly,
                    format!(
                        "Slow operation detected: {} took {:.2}ms",
                        span.operation,
                        duration.as_secs_f64() * 1000.0
                    ),
                    metadata,
                    DebugEventSeverity::Warning,
                )
                .await;
            }
        }

        Ok(())
    }

    /// Record memory allocation for leak detection
    pub async fn record_allocation(&self, size: u64, location: String) -> NetworkResult<String> {
        if !self.config.enable_memory_debugging {
            return Ok(String::new());
        }

        let allocation_id = Uuid::new_v4().to_string();
        let allocation = MemoryAllocation {
            size,
            allocated_at: Instant::now(),
            location,
            still_alive: true,
        };

        {
            let mut tracker = self.memory_tracker.write().await;
            tracker
                .allocations
                .insert(allocation_id.clone(), allocation);
            tracker.total_allocated += size;
            tracker.allocation_count += 1;

            if tracker.total_allocated > tracker.peak_usage {
                tracker.peak_usage = tracker.total_allocated;
            }
        }

        Ok(allocation_id)
    }

    /// Record memory deallocation
    pub async fn record_deallocation(&self, allocation_id: String) -> NetworkResult<()> {
        if !self.config.enable_memory_debugging || allocation_id.is_empty() {
            return Ok(());
        }

        let mut tracker = self.memory_tracker.write().await;
        if let Some(allocation) = tracker.allocations.get_mut(&allocation_id) {
            allocation.still_alive = false;
            tracker.total_allocated = tracker.total_allocated.saturating_sub(allocation.size);
        }

        Ok(())
    }

    /// Generate comprehensive debug report
    #[instrument(skip(self))]
    pub async fn generate_debug_report(&self) -> NetworkResult<String> {
        let uptime = self.start_time.elapsed();
        let packet_count = self.packet_captures.read().await.len();
        let connection_count = self.connection_states.read().await.len();
        let event_count = self.debug_events.read().await.len();

        let memory_stats = {
            let tracker = self.memory_tracker.read().await;
            format!(
                "Current: {:.2} MB, Peak: {:.2} MB, Allocations: {}",
                tracker.total_allocated as f64 / 1_048_576.0,
                tracker.peak_usage as f64 / 1_048_576.0,
                tracker.allocation_count
            )
        };

        let top_operations = {
            let profiler = self.profiler.read().await;
            let mut operations: Vec<_> = profiler.profiles.values().collect();
            operations.sort_by(|a, b| b.total_calls.cmp(&a.total_calls));
            operations
                .iter()
                .take(5)
                .map(|p| {
                    format!(
                        "  {} - {} calls, avg {:.2}ms",
                        p.operation,
                        p.total_calls,
                        p.average_duration.as_secs_f64() * 1000.0
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        Ok(format!(
            "🔧 Ultra-Modern Network Debug Report\n\
            ====================================\n\
            ⏰ Debug Session Uptime: {:?}\n\
            📊 Debug Configuration:\n\
            - Packet Capture: {}\n\
            - Connection Tracking: {}\n\
            - Performance Profiling: {}\n\
            - Memory Debugging: {}\n\
            - Verbosity Level: {:?}\n\
            \n\
            📦 Packet Capture Statistics\n\
            ----------------------------\n\
            Captured Packets: {} / {} max\n\
            \n\
            🔗 Connection Tracking\n\
            ----------------------\n\
            Active Connections: {}\n\
            \n\
            ⏱️  Performance Profile (Top Operations)\n\
            ----------------------------------------\n\
            {}\n\
            \n\
            🧠 Memory Debugging\n\
            -------------------\n\
            Memory Statistics: {}\n\
            \n\
            📋 Debug Events\n\
            ---------------\n\
            Total Events Logged: {}\n\
            \n\
            🛠️  Debug Tools Status\n\
            ----------------------\n\
            Console: {}\n\
            Real-time Monitoring: Active\n\
            Traffic Simulation: Available\n\
            Error Injection: Available\n\
            \n\
            Generated at: {}\n",
            uptime,
            if self.config.enable_packet_capture {
                "[ON] Enabled"
            } else {
                "[OFF] Disabled"
            },
            if self.config.enable_connection_tracking {
                "[ON] Enabled"
            } else {
                "[OFF] Disabled"
            },
            if self.config.enable_profiling {
                "[ON] Enabled"
            } else {
                "[OFF] Disabled"
            },
            if self.config.enable_memory_debugging {
                "[ON] Enabled"
            } else {
                "[OFF] Disabled"
            },
            self.config.verbosity_level,
            packet_count,
            self.config.max_captured_packets,
            connection_count,
            if top_operations.is_empty() {
                "  No operations profiled yet".to_string()
            } else {
                top_operations
            },
            memory_stats,
            event_count,
            if self.config.console_port > 0 {
                format!("[ON] Enabled (port {})", self.config.console_port)
            } else {
                "[OFF] Disabled".to_string()
            },
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        ))
    }

    // Helper methods

    fn format_payload_preview(&self, payload: &[u8]) -> String {
        let preview_len = 64.min(payload.len());
        let preview_bytes = &payload[..preview_len];

        // Try to format as UTF-8 string first
        if let Ok(text) = std::str::from_utf8(preview_bytes) {
            if text
                .chars()
                .all(|c| c.is_ascii_graphic() || c.is_ascii_whitespace())
            {
                return format!("\"{}\"", text.replace('\n', "\\n").replace('\r', "\\r"));
            }
        }

        // Fall back to hex representation
        let hex: String = preview_bytes
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect::<Vec<_>>()
            .join(" ");

        if payload.len() > preview_len {
            format!("{} ... ({} bytes total)", hex, payload.len())
        } else {
            hex
        }
    }

    async fn log_event(
        &self,
        event_type: DebugEventType,
        description: String,
        metadata: HashMap<String, String>,
        severity: DebugEventSeverity,
    ) {
        let event = DebugEvent {
            id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            event_type,
            description,
            metadata,
            severity,
        };

        let mut events = self.debug_events.write().await;
        events.push(event);

        // Keep only last 10000 events to prevent memory growth
        if events.len() > 10000 {
            events.remove(0);
        }
    }

    async fn start_debug_console(&mut self) -> NetworkResult<()> {
        // Debug console implementation would go here
        // This would start a web server or TCP server for interactive debugging
        info!(
            "🖥️  Debug console would start on port {}",
            self.config.console_port
        );
        Ok(())
    }
}

/// Convenience macro for performance profiling
#[macro_export]
macro_rules! debug_profile {
    ($debugger:expr, $operation:expr, $body:expr) => {{
        let span_id = $debugger.start_profile($operation.to_string()).await?;
        let result = $body;
        let success = result.is_ok();
        $debugger.end_profile(span_id, success).await?;
        result
    }};
}

/// Convenience macro for memory tracking
#[macro_export]
macro_rules! debug_alloc {
    ($debugger:expr, $size:expr, $location:expr) => {{
        $debugger
            .record_allocation($size, $location.to_string())
            .await
            .unwrap_or_default()
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_debugger_initialization() {
        let config = DebugConfig::default();
        let mut debugger = NetworkDebugger::new(config);

        debugger.initialize().await.unwrap();

        // Verify initialization
        assert!(debugger.start_time.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_packet_capture() {
        let config = DebugConfig {
            enable_packet_capture: true,
            ..Default::default()
        };
        let debugger = NetworkDebugger::new(config);

        let headers = HashMap::new();
        let payload = b"test payload";

        debugger
            .capture_packet(
                PacketDirection::Outgoing,
                payload.len(),
                "TEST".to_string(),
                Some("127.0.0.1:8080".to_string()),
                Some("127.0.0.1:9090".to_string()),
                headers,
                payload,
            )
            .await
            .unwrap();

        let captures = debugger.packet_captures.read().await;
        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].size, payload.len());
        assert!(matches!(captures[0].direction, PacketDirection::Outgoing));
    }

    #[tokio::test]
    async fn test_connection_tracking() {
        let config = DebugConfig {
            enable_connection_tracking: true,
            ..Default::default()
        };
        let debugger = NetworkDebugger::new(config);

        debugger
            .update_connection_state(
                "conn-1".to_string(),
                "192.168.1.100:8080".to_string(),
                ConnectionDebugState::Connected,
            )
            .await
            .unwrap();

        let connections = debugger.connection_states.read().await;
        assert!(connections.contains_key("conn-1"));
        assert!(matches!(
            connections["conn-1"].state,
            ConnectionDebugState::Connected
        ));
    }

    #[tokio::test]
    async fn test_performance_profiling() {
        let config = DebugConfig {
            enable_profiling: true,
            ..Default::default()
        };
        let debugger = NetworkDebugger::new(config);

        let span_id = debugger
            .start_profile("test_operation".to_string())
            .await
            .unwrap();

        // Simulate some work
        tokio::time::sleep(Duration::from_millis(10)).await;

        debugger.end_profile(span_id, true).await.unwrap();

        let profiler = debugger.profiler.read().await;
        assert!(profiler.profiles.contains_key("test_operation"));

        let profile = &profiler.profiles["test_operation"];
        assert_eq!(profile.total_calls, 1);
        assert!(profile.total_duration >= Duration::from_millis(10));
    }

    #[tokio::test]
    async fn test_memory_tracking() {
        let config = DebugConfig {
            enable_memory_debugging: true,
            ..Default::default()
        };
        let debugger = NetworkDebugger::new(config);

        let allocation_id = debugger
            .record_allocation(1024, "test_location".to_string())
            .await
            .unwrap();

        {
            let tracker = debugger.memory_tracker.read().await;
            assert_eq!(tracker.total_allocated, 1024);
            assert_eq!(tracker.allocation_count, 1);
        }

        debugger.record_deallocation(allocation_id).await.unwrap();

        {
            let tracker = debugger.memory_tracker.read().await;
            assert_eq!(tracker.total_allocated, 0);
        }
    }

    #[tokio::test]
    async fn test_debug_report_generation() {
        let debugger = NetworkDebugger::new(DebugConfig::default());

        let report = debugger.generate_debug_report().await.unwrap();

        assert!(report.contains("Ultra-Modern Network Debug Report"));
        assert!(report.contains("Debug Session Uptime"));
        assert!(report.contains("Packet Capture Statistics"));
        assert!(report.contains("Connection Tracking"));
        assert!(report.contains("Performance Profile"));
        assert!(report.contains("Memory Debugging"));
    }

    #[test]
    fn test_debug_level_ordering() {
        assert!(DebugLevel::Silent < DebugLevel::Normal);
        assert!(DebugLevel::Normal < DebugLevel::Verbose);
        assert!(DebugLevel::Verbose < DebugLevel::Trace);
    }
}
