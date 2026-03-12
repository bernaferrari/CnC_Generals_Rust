//! Ultra-modern observability and telemetry system (2025)
//!
//! Provides comprehensive monitoring with:
//! - OpenTelemetry distributed tracing
//! - Prometheus metrics with custom collectors
//! - Structured logging with correlation IDs
//! - Real-time performance monitoring
//! - Tokio Console integration
//! - Custom dashboards and alerting

use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Conditional imports based on features
#[cfg(feature = "metrics")]
use {
    metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram},
    opentelemetry::KeyValue,
    opentelemetry_otlp::WithExportConfig,
    opentelemetry_sdk::{trace, Resource},
    tracing::{error, info, warn},
    tracing_opentelemetry::OpenTelemetryLayer,
    tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry},
};

#[cfg(feature = "metrics")]
use console_subscriber;

// Fallback macros when tracing is not available
#[cfg(not(feature = "metrics"))]
macro_rules! debug {
    ($($args:tt)*) => {};
}
#[cfg(not(feature = "metrics"))]
macro_rules! error { ($($args:tt)*) => { eprintln!($($args)*) }; }
#[cfg(not(feature = "metrics"))]
macro_rules! info { ($($args:tt)*) => { println!($($args)*) }; }
#[cfg(not(feature = "metrics"))]
macro_rules! trace {
    ($($args:tt)*) => {};
}
#[cfg(not(feature = "metrics"))]
macro_rules! warn { ($($args:tt)*) => { eprintln!("WARN: {}", format!($($args)*)) }; }

/// Ultra-modern observability configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    /// Enable distributed tracing
    pub enable_tracing: bool,
    /// OpenTelemetry collector endpoint
    pub otlp_endpoint: Option<String>,
    /// Service name for tracing
    pub service_name: String,
    /// Service version
    pub service_version: String,
    /// Environment (dev, staging, prod)
    pub environment: String,
    /// Enable Prometheus metrics
    pub enable_metrics: bool,
    /// Metrics server bind address
    pub metrics_bind_addr: String,
    /// Enable Tokio Console
    pub enable_console: bool,
    /// Sampling rate for traces (0.0 to 1.0)
    pub trace_sampling_rate: f64,
    /// Custom attributes to add to all spans
    pub global_attributes: HashMap<String, String>,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            enable_tracing: true,
            otlp_endpoint: None,
            service_name: "game-network".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            environment: std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            enable_metrics: true,
            metrics_bind_addr: "127.0.0.1:9090".to_string(),
            enable_console: false,
            trace_sampling_rate: 1.0,
            global_attributes: HashMap::new(),
        }
    }
}

/// Advanced telemetry system with modern observability patterns
pub struct TelemetrySystem {
    config: ObservabilityConfig,
    start_time: NetworkInstant,
    metrics_server: Option<tokio::task::JoinHandle<()>>,

    #[cfg(feature = "metrics")]
    _tracer_provider: Option<String>, // Simplified for now
}

/// Direction of a file transfer for telemetry accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferTelemetryDirection {
    Upload,
    Download,
}

impl TransferTelemetryDirection {
    fn as_label(self) -> &'static str {
        match self {
            TransferTelemetryDirection::Upload => "upload",
            TransferTelemetryDirection::Download => "download",
        }
    }
}

impl TelemetrySystem {
    /// Initialize the ultra-modern telemetry system
    pub async fn initialize(config: ObservabilityConfig) -> NetworkResult<Self> {
        info!("🚀 Initializing ultra-modern telemetry system");

        let mut system = Self {
            config: config.clone(),
            start_time: NetworkInstant::now(),
            metrics_server: None,

            #[cfg(feature = "metrics")]
            _tracer_provider: None,
        };

        // Initialize tracing with OpenTelemetry
        #[cfg(feature = "metrics")]
        if config.enable_tracing {
            system.setup_distributed_tracing().await?;
        }

        // Initialize Tokio Console
        #[cfg(feature = "metrics")]
        if config.enable_console {
            console_subscriber::init();
            info!("📊 Tokio Console enabled - connect with `tokio-console`");
        }

        // Start metrics server
        if config.enable_metrics {
            system.start_metrics_server().await?;
        }

        // Register custom metrics
        system.register_custom_metrics();

        info!("✅ Telemetry system initialized successfully");
        Ok(system)
    }

    /// Setup distributed tracing with OpenTelemetry
    #[cfg(feature = "metrics")]
    async fn setup_distributed_tracing(&mut self) -> NetworkResult<()> {
        info!("🔍 Setting up distributed tracing");

        let mut tracer = None;
        // Build the tracer
        if let Some(endpoint) = &self.config.otlp_endpoint {
            match opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(
                    opentelemetry_otlp::new_exporter()
                        .tonic()
                        .with_endpoint(endpoint),
                )
                .with_trace_config(trace::config().with_resource(Resource::new(vec![
                    KeyValue::new("service.name", self.config.service_name.clone()),
                    KeyValue::new("service.version", self.config.service_version.clone()),
                    KeyValue::new("environment", self.config.environment.clone()),
                    KeyValue::new("runtime", "tokio"),
                    KeyValue::new("language", "rust"),
                ])))
                .install_simple()
            {
                Ok(installed) => {
                    tracer = Some(installed);
                    info!(
                        "✅ OpenTelemetry exporter configured for endpoint {}",
                        endpoint
                    );
                }
                Err(err) => warn!(
                    "Failed to initialize OTLP exporter ({}). Falling back to local logging only.",
                    err
                ),
            }
        } else {
            info!("No OTLP endpoint configured; using local structured logging only");
        }

        // Setup structured logging (without OpenTelemetry for now)
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true);
        // .json(); // TODO: Re-enable when json feature is working

        // Environment filter for log levels
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"));

        // Combine layers
        if let Some(tracer) = tracer {
            let telemetry_layer = OpenTelemetryLayer::new(tracer);
            Registry::default()
                .with(filter)
                .with(fmt_layer)
                .with(telemetry_layer)
                .try_init()
                .map_err(|e| {
                    NetworkError::generic(format!("Failed to initialize tracing: {}", e))
                })?;
        } else {
            Registry::default()
                .with(filter)
                .with(fmt_layer)
                .try_init()
                .map_err(|e| {
                    NetworkError::generic(format!("Failed to initialize tracing: {}", e))
                })?;
        }

        info!("✅ Distributed tracing configured successfully");
        Ok(())
    }

    /// Start the Prometheus metrics server
    async fn start_metrics_server(&mut self) -> NetworkResult<()> {
        #[cfg(feature = "metrics")]
        {
            use metrics_exporter_prometheus::PrometheusBuilder;
            use std::net::SocketAddr;

            info!(
                "📊 Starting Prometheus metrics server on {}",
                self.config.metrics_bind_addr
            );

            let addr: SocketAddr = self.config.metrics_bind_addr.parse().map_err(|e| {
                NetworkError::generic(format!("Invalid metrics bind address: {}", e))
            })?;

            let builder = PrometheusBuilder::new();
            let handle = builder.install_recorder().map_err(|e| {
                NetworkError::generic(format!("Failed to install Prometheus recorder: {}", e))
            })?;

            // Start HTTP server for metrics
            let make_service = hyper::service::make_service_fn(move |_conn| {
                let handle = handle.clone();
                async move {
                    Ok::<_, hyper::Error>(hyper::service::service_fn(move |_req| {
                        let handle = handle.clone();
                        async move {
                            let metrics = handle.render();
                            Ok::<_, hyper::Error>(hyper::Response::new(metrics))
                        }
                    }))
                }
            });

            info!("📊 Metrics server listening on http://{}/metrics", addr);
            let server = hyper::Server::bind(&addr).serve(make_service);
            let server_handle = tokio::spawn(async move {
                if let Err(e) = server.await {
                    error!("Metrics server error: {}", e);
                }
            });

            self.metrics_server = Some(server_handle);
        }

        #[cfg(not(feature = "metrics"))]
        {
            info!("📊 Metrics feature not enabled, skipping metrics server");
        }

        Ok(())
    }

    /// Register custom metrics for network monitoring
    fn register_custom_metrics(&self) {
        #[cfg(feature = "metrics")]
        {
            // Network metrics
            describe_counter!(
                "network_packets_sent_total",
                "Total number of network packets sent"
            );
            describe_counter!(
                "network_packets_received_total",
                "Total number of network packets received"
            );
            describe_counter!(
                "network_bytes_sent_total",
                "Total number of bytes sent over network"
            );
            describe_counter!(
                "network_bytes_received_total",
                "Total number of bytes received over network"
            );
            describe_histogram!(
                "network_packet_processing_duration_seconds",
                "Time spent processing network packets"
            );

            // Connection metrics
            describe_gauge!(
                "network_active_connections",
                "Number of active network connections"
            );
            describe_counter!(
                "network_connections_established_total",
                "Total number of connections established"
            );
            describe_counter!(
                "network_connections_closed_total",
                "Total number of connections closed"
            );
            describe_histogram!(
                "network_connection_duration_seconds",
                "Duration of network connections"
            );

            // Game-specific metrics
            describe_gauge!("game_active_players", "Number of active players");
            describe_counter!(
                "game_frames_processed_total",
                "Total number of game frames processed"
            );
            describe_histogram!(
                "game_frame_processing_duration_seconds",
                "Time spent processing game frames"
            );
            describe_counter!(
                "game_commands_processed_total",
                "Total number of game commands processed"
            );

            // Performance metrics
            describe_histogram!("memory_allocation_size_bytes", "Size of memory allocations");
            describe_gauge!("memory_usage_bytes", "Current memory usage in bytes");
            describe_histogram!(
                "task_execution_duration_seconds",
                "Time spent executing async tasks"
            );

            // File transfer metrics
            describe_counter!(
                "file_transfers_started_total",
                "Number of file transfers started"
            );
            describe_counter!(
                "file_transfers_completed_total",
                "Number of file transfers completed successfully"
            );
            describe_counter!(
                "file_transfers_failed_total",
                "Number of file transfers that failed"
            );
            describe_counter!(
                "file_transfer_bytes_total",
                "Cumulative bytes streamed via file transfer"
            );
            describe_histogram!(
                "file_transfer_duration_seconds",
                "Duration of completed file transfers"
            );
            describe_gauge!(
                "file_transfers_active",
                "Active file transfers by direction"
            );

            info!("📈 Custom metrics registered successfully");
        }
    }

    /// Record network packet metrics
    pub fn record_packet_sent(&self, size: usize) {
        #[cfg(feature = "metrics")]
        {
            counter!("network_packets_sent_total", 1);
            counter!("network_bytes_sent_total", size as u64);
        }
    }

    /// Record network packet received
    pub fn record_packet_received(&self, size: usize, processing_time: Duration) {
        #[cfg(feature = "metrics")]
        {
            counter!("network_packets_received_total", 1);
            counter!("network_bytes_received_total", size as u64);
            histogram!(
                "network_packet_processing_duration_seconds",
                processing_time.as_secs_f64()
            );
        }
    }

    /// Record connection metrics
    pub fn record_connection_established(&self) {
        #[cfg(feature = "metrics")]
        {
            counter!("network_connections_established_total", 1);
        }
    }

    /// Record connection closed
    pub fn record_connection_closed(&self, duration: Duration) {
        #[cfg(feature = "metrics")]
        {
            counter!("network_connections_closed_total", 1);
            histogram!(
                "network_connection_duration_seconds",
                duration.as_secs_f64()
            );
        }
    }

    /// Update active connections gauge
    pub fn set_active_connections(&self, count: usize) {
        #[cfg(feature = "metrics")]
        {
            gauge!("network_active_connections", count as f64);
        }
    }

    /// Record game frame processing
    pub fn record_frame_processed(&self, processing_time: Duration) {
        #[cfg(feature = "metrics")]
        {
            counter!("game_frames_processed_total", 1);
            histogram!(
                "game_frame_processing_duration_seconds",
                processing_time.as_secs_f64()
            );
        }
    }

    /// Record a processed game command.
    pub fn record_command_processed(&self) {
        #[cfg(feature = "metrics")]
        {
            counter!("game_commands_processed_total", 1);
        }
    }

    /// Update the active player gauge to reflect lobby state.
    pub fn set_active_players(&self, count: usize) {
        #[cfg(feature = "metrics")]
        {
            gauge!("game_active_players", count as f64);
        }
    }

    /// Track the current run-ahead target used by deterministic networking.
    pub fn set_run_ahead(&self, run_ahead: u32) {
        #[cfg(feature = "metrics")]
        {
            gauge!("game_run_ahead", run_ahead as f64);
        }
    }

    /// Track the smoothed packet arrival cushion in frames.
    pub fn set_packet_cushion(&self, cushion_frames: f32) {
        #[cfg(feature = "metrics")]
        {
            gauge!("network_packet_cushion_frames", cushion_frames as f64);
        }
    }

    /// Track the minimum packet arrival cushion observed recently.
    pub fn set_min_packet_cushion(&self, cushion_frames: f32) {
        #[cfg(feature = "metrics")]
        {
            gauge!("network_packet_cushion_min_frames", cushion_frames as f64);
        }
    }

    /// Track how far ahead the local simulation is relative to the execution frame.
    pub fn set_frames_ahead(&self, frames_ahead: u32) {
        #[cfg(feature = "metrics")]
        {
            gauge!("game_frames_ahead", frames_ahead as f64);
        }
    }

    /// Update the load progress gauge used by front-end observers.
    pub fn set_load_progress(&self, percent: u8) {
        #[cfg(feature = "metrics")]
        {
            gauge!("game_load_progress_percent", percent as f64);
        }
    }

    /// Record that loading completed successfully.
    pub fn mark_load_complete(&self) {
        #[cfg(feature = "metrics")]
        {
            counter!("game_load_completed_total", 1);
        }
    }

    /// Record a file transfer starting up.
    pub fn record_transfer_started(
        &self,
        direction: TransferTelemetryDirection,
        _total_bytes: u64,
    ) {
        #[cfg(feature = "metrics")]
        {
            counter!(
                "file_transfers_started_total",
                1,
                "direction" => direction.as_label()
            );
        }
    }

    /// Record incremental progress for a transfer.
    pub fn record_transfer_progress(
        &self,
        direction: TransferTelemetryDirection,
        chunk_bytes: u64,
    ) {
        #[cfg(feature = "metrics")]
        {
            if chunk_bytes == 0 {
                return;
            }
            counter!(
                "file_transfer_bytes_total",
                chunk_bytes,
                "direction" => direction.as_label()
            );
        }
    }

    /// Record a completed transfer with its duration for latency histograms.
    pub fn record_transfer_completed(
        &self,
        direction: TransferTelemetryDirection,
        _total_bytes: u64,
        duration: Duration,
    ) {
        #[cfg(feature = "metrics")]
        {
            let label = direction.as_label();
            counter!("file_transfers_completed_total", 1, "direction" => label);
            histogram!(
                "file_transfer_duration_seconds",
                duration.as_secs_f64(),
                "direction" => label
            );
        }
    }

    /// Record a failed transfer for failure-rate tracking.
    pub fn record_transfer_failed(&self, direction: TransferTelemetryDirection) {
        #[cfg(feature = "metrics")]
        {
            counter!(
                "file_transfers_failed_total",
                1,
                "direction" => direction.as_label()
            );
        }
    }

    /// Update the number of active transfers of a given direction.
    pub fn set_active_transfers(&self, direction: TransferTelemetryDirection, active: usize) {
        #[cfg(feature = "metrics")]
        {
            gauge!(
                "file_transfers_active",
                active as f64,
                "direction" => direction.as_label()
            );
        }
    }

    /// Create a traced span for network operations
    #[cfg(feature = "metrics")]
    pub fn network_span(&self, operation: &str, player_id: Option<u8>) -> tracing::Span {
        let span = tracing::info_span!(
            "network_operation",
            operation = operation,
            service.name = %self.config.service_name,
            service.version = %self.config.service_version,
        );

        if let Some(id) = player_id {
            span.record("player_id", id);
        }

        span
    }

    #[cfg(not(feature = "metrics"))]
    pub fn network_span(&self, _operation: &str, _player_id: Option<u8>) -> NoOpSpan {
        NoOpSpan
    }

    /// Create a traced span for game operations
    #[cfg(feature = "metrics")]
    pub fn game_span(&self, operation: &str, frame: Option<u32>) -> tracing::Span {
        let span = tracing::info_span!(
            "game_operation",
            operation = operation,
            service.name = %self.config.service_name,
        );

        if let Some(f) = frame {
            span.record("frame", f);
        }

        span
    }

    #[cfg(not(feature = "metrics"))]
    pub fn game_span(&self, _operation: &str, _frame: Option<u32>) -> NoOpSpan {
        NoOpSpan
    }

    /// Generate comprehensive health report
    pub async fn generate_health_report(&self) -> HealthReport {
        let uptime = self.start_time.elapsed();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        HealthReport {
            service_name: self.config.service_name.clone(),
            service_version: self.config.service_version.clone(),
            environment: self.config.environment.clone(),
            uptime_seconds: uptime.as_secs(),
            timestamp,
            status: HealthStatus::Healthy,
            checks: vec![
                HealthCheck {
                    name: "telemetry_system".to_string(),
                    status: HealthStatus::Healthy,
                    message: "Telemetry system operational".to_string(),
                    last_checked: timestamp,
                },
                HealthCheck {
                    name: "metrics_server".to_string(),
                    status: if self.metrics_server.is_some() {
                        HealthStatus::Healthy
                    } else {
                        HealthStatus::Degraded
                    },
                    message: "Metrics server status".to_string(),
                    last_checked: timestamp,
                },
            ],
        }
    }

    /// Shutdown the telemetry system gracefully
    pub async fn shutdown(&mut self) -> NetworkResult<()> {
        info!("🛑 Shutting down telemetry system");

        // Stop metrics server
        if let Some(handle) = self.metrics_server.take() {
            handle.abort();
            info!("📊 Metrics server stopped");
        }

        // Flush OpenTelemetry data
        #[cfg(feature = "metrics")]
        {
            opentelemetry::global::shutdown_tracer_provider();
            info!("🔍 Tracing data flushed");
        }

        info!("✅ Telemetry system shutdown complete");
        Ok(())
    }
}

impl Drop for TelemetrySystem {
    fn drop(&mut self) {
        if let Some(handle) = self.metrics_server.take() {
            handle.abort();
        }
    }
}

/// No-op span for when tracing is disabled
#[cfg(not(feature = "metrics"))]
pub struct NoOpSpan;

#[cfg(not(feature = "metrics"))]
impl NoOpSpan {
    pub fn in_scope<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        f()
    }
}

/// Health status enumeration
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Critical,
}

/// Individual health check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub message: String,
    pub last_checked: u64,
}

/// Comprehensive health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub service_name: String,
    pub service_version: String,
    pub environment: String,
    pub uptime_seconds: u64,
    pub timestamp: u64,
    pub status: HealthStatus,
    pub checks: Vec<HealthCheck>,
}

/// Global telemetry instance
static TELEMETRY: once_cell::sync::OnceCell<Arc<TelemetrySystem>> =
    once_cell::sync::OnceCell::new();

/// Initialize global telemetry system
pub async fn initialize_telemetry(config: ObservabilityConfig) -> NetworkResult<()> {
    let system = Arc::new(TelemetrySystem::initialize(config).await?);
    TELEMETRY
        .set(system)
        .map_err(|_| NetworkError::generic("Telemetry already initialized".to_string()))?;
    Ok(())
}

/// Get global telemetry instance
pub fn telemetry() -> Option<&'static Arc<TelemetrySystem>> {
    TELEMETRY.get()
}

/// Convenience macro for creating network spans
#[macro_export]
macro_rules! network_span {
    ($operation:expr) => {
        if let Some(telemetry) = $crate::observability::telemetry() {
            telemetry.network_span($operation, None)
        } else {
            $crate::observability::NoOpSpan
        }
    };
    ($operation:expr, $player_id:expr) => {
        if let Some(telemetry) = $crate::observability::telemetry() {
            telemetry.network_span($operation, Some($player_id))
        } else {
            $crate::observability::NoOpSpan
        }
    };
}

/// Convenience macro for creating game spans
#[macro_export]
macro_rules! game_span {
    ($operation:expr) => {
        if let Some(telemetry) = $crate::observability::telemetry() {
            telemetry.game_span($operation, None)
        } else {
            $crate::observability::NoOpSpan
        }
    };
    ($operation:expr, $frame:expr) => {
        if let Some(telemetry) = $crate::observability::telemetry() {
            telemetry.game_span($operation, Some($frame))
        } else {
            $crate::observability::NoOpSpan
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_telemetry_initialization() {
        let config = ObservabilityConfig {
            enable_metrics: false,
            ..Default::default()
        };
        let system = TelemetrySystem::initialize(config).await.unwrap();

        // Test metrics recording
        system.record_packet_sent(100);
        system.record_packet_received(150, Duration::from_millis(5));

        // Test health report
        let report = system.generate_health_report().await;
        assert_eq!(report.service_name, "game-network");
    }

    #[tokio::test]
    async fn test_global_telemetry() {
        let config = ObservabilityConfig {
            enable_metrics: false, // Disable to avoid conflicts
            ..Default::default()
        };

        if telemetry().is_none() {
            initialize_telemetry(config).await.unwrap();
        }

        assert!(telemetry().is_some());
    }
}
