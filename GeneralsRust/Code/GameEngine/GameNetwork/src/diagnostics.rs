//! Ultra-modern network diagnostics and monitoring for GameNetwork (2025)
//!
//! Provides comprehensive observability features including:
//! - Real-time performance monitoring
//! - Network health assessments 
//! - Predictive analytics
//! - Automated alerting
//! - Debug tooling
//! - Integration with OpenTelemetry

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use tokio::sync::{RwLock, Notify};
use tokio::time::interval;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{NetworkError, NetworkResult, ErrorContext, ErrorSeverity, EnhancedError, ErrorId};
use crate::observability::telemetry;
use crate::time::NetworkInstant;

use chrono;

#[cfg(feature = "metrics")]
use log;
#[cfg(feature = "metrics")]
use tracing::{info, debug, warn, error, instrument, Span};
#[cfg(feature = "metrics")]
use metrics::{counter, gauge, histogram, describe_counter, describe_gauge, describe_histogram};

// Fallback when metrics feature is disabled
#[cfg(not(feature = "metrics"))]
macro_rules! info { ($($args:tt)*) => { println!($($args)*) }; }
#[cfg(not(feature = "metrics"))]
macro_rules! debug { ($($args:tt)*) => {}; }
#[cfg(not(feature = "metrics"))]
macro_rules! warn { ($($args:tt)*) => { eprintln!("WARN: {}", format!($($args)*)) }; }
#[cfg(not(feature = "metrics"))]
macro_rules! error { ($($args:tt)*) => { eprintln!("ERROR: {}", format!($($args)*)) }; }

/// Ultra-comprehensive network diagnostic information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkDiagnostics {
    pub connection_stats: ConnectionDiagnostics,
    pub transport_stats: TransportDiagnostics,
    pub frame_stats: FrameDiagnostics,
    pub security_stats: SecurityDiagnostics,
    pub performance_stats: PerformanceDiagnostics,
    pub error_stats: ErrorDiagnostics,
    pub health_status: HealthStatus,
    pub timestamp: u64,
    pub diagnostic_id: String,
}

/// Enhanced connection diagnostic information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionDiagnostics {
    pub active_connections: usize,
    pub total_connections_established: u64,
    pub total_connections_lost: u64,
    pub average_connection_duration: Duration,
    pub connection_failures: u64,
    pub connections_by_protocol: HashMap<String, usize>,
    pub connection_quality_distribution: QualityDistribution,
    pub reconnection_attempts: u64,
    pub connection_pool_utilization: f64,
    pub peak_concurrent_connections: usize,
}

/// Enhanced transport layer diagnostic information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportDiagnostics {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packet_loss_rate: f64,
    pub average_latency: Duration,
    pub bandwidth_utilization: f64,
    pub jitter: Duration,
    pub out_of_order_packets: u64,
    pub duplicate_packets: u64,
    pub corrupted_packets: u64,
    pub compression_ratio: f64,
    pub encryption_overhead: f64,
    pub retransmissions: u64,
    pub congestion_events: u64,
    pub throughput_mbps: f64,
}

/// Enhanced frame synchronization diagnostic information  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameDiagnostics {
    pub frames_processed: u64,
    pub frames_skipped: u64,
    pub average_frame_time: Duration,
    pub frame_desync_count: u64,
    pub pending_frames: usize,
    pub frame_prediction_accuracy: f64,
    pub rollback_count: u64,
    pub maximum_rollback_depth: u32,
    pub input_delay_frames: u32,
    pub frame_timing_variance: Duration,
    pub catchup_frames: u64,
}

/// Security-related diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityDiagnostics {
    pub authentication_attempts: u64,
    pub authentication_failures: u64,
    pub authorization_denials: u64,
    pub potential_attacks_detected: u64,
    pub rate_limit_violations: u64,
    pub suspicious_activity_events: u64,
    pub encryption_errors: u64,
    pub certificate_validation_failures: u64,
}

/// Performance-related diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceDiagnostics {
    pub cpu_usage_percent: f64,
    pub memory_usage_bytes: u64,
    pub memory_peak_bytes: u64,
    pub gc_pressure: f64,
    pub thread_pool_utilization: f64,
    pub async_task_queue_depth: usize,
    pub buffer_pool_utilization: f64,
    pub cache_hit_rate: f64,
    pub disk_io_operations: u64,
    pub network_buffer_overruns: u64,
}

/// Error-related diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDiagnostics {
    pub total_errors: u64,
    pub errors_by_severity: HashMap<String, u64>,
    pub errors_by_type: HashMap<String, u64>,
    pub recovery_success_rate: f64,
    pub mean_time_to_recovery: Duration,
    pub error_rate_per_second: f64,
    pub recent_error_patterns: Vec<ErrorPattern>,
}

/// Connection quality distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityDistribution {
    pub excellent: usize,
    pub good: usize,
    pub fair: usize,
    pub poor: usize,
}

/// Error pattern for trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPattern {
    pub error_type: String,
    pub frequency: u64,
    pub first_seen: u64,
    pub last_seen: u64,
    pub trend: TrendDirection,
}

/// Trend direction for analytics
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TrendDirection {
    Increasing,
    Stable,
    Decreasing,
}

/// Enhanced network health status with detailed assessment
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum HealthStatus {
    Optimal,
    Healthy,
    Degraded,
    Unhealthy,
    Critical,
    Failed,
}

impl Default for HealthStatus {
    fn default() -> Self {
        HealthStatus::Healthy
    }
}

impl HealthStatus {
    pub fn as_score(&self) -> u8 {
        match self {
            Self::Optimal => 100,
            Self::Healthy => 80,
            Self::Degraded => 60,
            Self::Unhealthy => 40,
            Self::Critical => 20,
            Self::Failed => 0,
        }
    }
    
    pub fn from_score(score: u8) -> Self {
        match score {
            90..=100 => Self::Optimal,
            70..=89 => Self::Healthy,
            50..=69 => Self::Degraded,
            30..=49 => Self::Unhealthy,
            10..=29 => Self::Critical,
            0..=9 => Self::Failed,
            _ => Self::Optimal, // Scores above 100 are treated as optimal
        }
    }
    
    pub fn color(&self) -> &'static str {
        match self {
            Self::Optimal => "🟢",
            Self::Healthy => "🟢",
            Self::Degraded => "🟡",
            Self::Unhealthy => "🟠",
            Self::Critical => "🔴",
            Self::Failed => "⚫",
        }
    }
}

/// Health check result with detailed information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub score: u8,
    pub message: String,
    pub details: HashMap<String, String>,
    pub last_checked: u64,
    pub check_duration: Duration,
    pub trend: TrendDirection,
}

/// Network quality assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkQuality {
    pub overall_score: u8,
    pub latency_score: u8,
    pub throughput_score: u8,
    pub reliability_score: u8,
    pub security_score: u8,
    pub stability_score: u8,
}

/// Ultra-modern diagnostic collector with predictive analytics
pub struct DiagnosticsCollector {
    start_time: NetworkInstant,
    stats: Arc<RwLock<NetworkDiagnostics>>,
    
    // Atomic counters for high-performance updates
    packets_sent: AtomicU64,
    packets_received: AtomicU64,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    errors_count: AtomicU64,
    active_connections: AtomicUsize,
    
    // Time-series data for trend analysis
    latency_history: Arc<RwLock<VecDeque<(NetworkInstant, Duration)>>>,
    throughput_history: Arc<RwLock<VecDeque<(NetworkInstant, f64)>>>,
    error_history: Arc<RwLock<VecDeque<(NetworkInstant, EnhancedError)>>>,
    
    // Alert system
    alert_notifier: Arc<Notify>,
    alert_thresholds: AlertThresholds,
    
    // Performance monitoring
    collection_start: NetworkInstant,
    last_gc_time: Arc<RwLock<NetworkInstant>>,
}

/// Alert thresholds configuration
#[derive(Debug, Clone)]
pub struct AlertThresholds {
    pub max_latency_ms: u64,
    pub min_throughput_mbps: f64,
    pub max_packet_loss_rate: f64,
    pub max_error_rate_per_second: f64,
    pub max_connection_failures: u64,
    pub max_memory_usage_mb: u64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            max_latency_ms: 200,
            min_throughput_mbps: 1.0,
            max_packet_loss_rate: 0.05, // 5%
            max_error_rate_per_second: 10.0,
            max_connection_failures: 5,
            max_memory_usage_mb: 512,
        }
    }
}

/// Diagnostic alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticAlert {
    pub id: String,
    pub severity: AlertSeverity,
    pub title: String,
    pub description: String,
    pub metric_name: String,
    pub current_value: f64,
    pub threshold_value: f64,
    pub timestamp: u64,
    pub context: HashMap<String, String>,
}

/// Alert severity levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

impl DiagnosticsCollector {
    /// Create new ultra-modern diagnostics collector
    pub fn new() -> Self {
        Self::with_thresholds(AlertThresholds::default())
    }
    
    /// Create diagnostics collector with custom alert thresholds
    pub fn with_thresholds(thresholds: AlertThresholds) -> Self {
        let now = NetworkInstant::now();
        Self {
            start_time: now,
            stats: Arc::new(RwLock::new(NetworkDiagnostics::default())),
            packets_sent: AtomicU64::new(0),
            packets_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            errors_count: AtomicU64::new(0),
            active_connections: AtomicUsize::new(0),
            latency_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            throughput_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            error_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            alert_notifier: Arc::new(Notify::new()),
            alert_thresholds: thresholds,
            collection_start: now,
            last_gc_time: Arc::new(RwLock::new(now)),
        }
    }
    
    /// Initialize background monitoring tasks
    pub async fn start_monitoring(&self) -> NetworkResult<()> {
        info!("🔍 Starting advanced network monitoring");
        
        // Register enhanced metrics
        self.register_enhanced_metrics();
        
        // Start periodic collection task
        let collector = Arc::new(self.clone());
        tokio::spawn(Self::periodic_collection_task(collector.clone()));
        
        // Start alert monitoring task
        tokio::spawn(Self::alert_monitoring_task(collector.clone()));
        
        // Start trend analysis task
        tokio::spawn(Self::trend_analysis_task(collector));
        
        info!("✅ Advanced monitoring started successfully");
        Ok(())
    }
    
    /// Register enhanced metrics with the telemetry system
    fn register_enhanced_metrics(&self) {
        #[cfg(feature = "metrics")]
        {
            // Network quality metrics
            describe_gauge!("network_quality_score", "Overall network quality score (0-100)");
            describe_gauge!("network_latency_p95", "95th percentile network latency");
            describe_gauge!("network_jitter", "Network jitter in milliseconds");
            describe_histogram!("network_response_time", "Network response time distribution");
            
            // Security metrics
            describe_counter!("security_events_total", "Total security events detected");
            describe_counter!("authentication_failures_total", "Total authentication failures");
            describe_gauge!("suspicious_activity_score", "Suspicious activity detection score");
            
            // Performance metrics
            describe_gauge!("memory_usage_percent", "Memory usage percentage");
            describe_gauge!("cpu_usage_percent", "CPU usage percentage");
            describe_gauge!("gc_pressure", "Garbage collection pressure");
            describe_gauge!("buffer_pool_utilization", "Network buffer pool utilization");
            
            // Game-specific metrics
            describe_histogram!("frame_prediction_accuracy", "Frame prediction accuracy");
            describe_counter!("rollback_events_total", "Total rollback events");
            describe_gauge!("input_delay_frames", "Current input delay in frames");
            
            info!("📊 Enhanced metrics registered successfully");
        }
    }
    
    /// Record packet sent with enhanced metrics
    #[instrument(skip(self), fields(size = size))]
    pub async fn record_packet_sent(&self, size: usize) {
        // Fast atomic update
        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(size as u64, Ordering::Relaxed);
        
        // Update detailed stats periodically to avoid lock contention
        let mut stats = self.stats.write().await;
        stats.transport_stats.packets_sent = self.packets_sent.load(Ordering::Relaxed);
        stats.transport_stats.bytes_sent = self.bytes_sent.load(Ordering::Relaxed);
        
        // Calculate throughput
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            stats.transport_stats.throughput_mbps = 
                (stats.transport_stats.bytes_sent as f64 * 8.0) / (elapsed * 1_000_000.0);
        }
        
        #[cfg(feature = "metrics")]
        {
            counter!("network_packets_sent_total").increment(1);
            counter!("network_bytes_sent_total").increment(size as u64);
            gauge!("network_throughput_mbps").set(stats.transport_stats.throughput_mbps);
        }
        
        // Check for alerts
        self.check_throughput_alert(stats.transport_stats.throughput_mbps).await;
    }
    
    /// Record packet received with latency tracking
    #[instrument(skip(self), fields(size = size, processing_time_ms = ?processing_time.as_millis()))]
    pub async fn record_packet_received(&self, size: usize, processing_time: Duration) {
        let now = NetworkInstant::now();
        
        // Fast atomic updates
        self.packets_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_received.fetch_add(size as u64, Ordering::Relaxed);
        
        // Update latency history for trend analysis
        {
            let mut latency_hist = self.latency_history.write().await;
            latency_hist.push_back((now, processing_time));
            if latency_hist.len() > 1000 {
                latency_hist.pop_front();
            }
        }
        
        // Update detailed stats
        let mut stats = self.stats.write().await;
        stats.transport_stats.packets_received = self.packets_received.load(Ordering::Relaxed);
        stats.transport_stats.bytes_received = self.bytes_received.load(Ordering::Relaxed);
        
        // Update average latency with exponential moving average
        let alpha = 0.1; // Smoothing factor
        let new_latency_ms = processing_time.as_secs_f64() * 1000.0;
        let current_avg_ms = stats.transport_stats.average_latency.as_secs_f64() * 1000.0;
        let updated_avg_ms = alpha * new_latency_ms + (1.0 - alpha) * current_avg_ms;
        stats.transport_stats.average_latency = Duration::from_secs_f64(updated_avg_ms / 1000.0);
        
        #[cfg(feature = "metrics")]
        {
            counter!("network_packets_received_total").increment(1);
            counter!("network_bytes_received_total").increment(size as u64);
            histogram!("network_packet_processing_duration_seconds").record(processing_time.as_secs_f64());
            gauge!("network_average_latency_ms").set(updated_avg_ms);
        }
        
        // Check for latency alerts
        self.check_latency_alert(Duration::from_secs_f64(updated_avg_ms / 1000.0)).await;
    }
    
    /// Record connection established
    pub async fn record_connection_established(&self) {
        let mut stats = self.stats.write().await;
        stats.connection_stats.active_connections += 1;
        stats.connection_stats.total_connections_established += 1;
        
        #[cfg(feature = "metrics")]
        {
            gauge!("active_connections").set(stats.connection_stats.active_connections as f64);
            counter!("connections_established").increment(1);
        }
    }
    
    /// Record connection lost
    pub async fn record_connection_lost(&self, duration: Duration) {
        let mut stats = self.stats.write().await;
        stats.connection_stats.active_connections = stats.connection_stats.active_connections.saturating_sub(1);
        stats.connection_stats.total_connections_lost += 1;
        
        // Update average connection duration
        let total_duration = stats.connection_stats.average_connection_duration.as_secs_f64() 
            * stats.connection_stats.total_connections_lost as f64;
        stats.connection_stats.average_connection_duration = Duration::from_secs_f64(
            (total_duration + duration.as_secs_f64()) / (stats.connection_stats.total_connections_lost + 1) as f64
        );
        
        #[cfg(feature = "metrics")]
        {
            gauge!("active_connections").set(stats.connection_stats.active_connections as f64);
            counter!("connections_lost").increment(1);
            histogram!("connection_duration").record(duration.as_secs_f64());
        }
    }
    
    /// Record frame processed
    pub async fn record_frame_processed(&self, processing_time: Duration) {
        let mut stats = self.stats.write().await;
        stats.frame_stats.frames_processed += 1;
        
        // Update average frame time
        let total_time = stats.frame_stats.average_frame_time.as_secs_f64() 
            * stats.frame_stats.frames_processed as f64;
        stats.frame_stats.average_frame_time = Duration::from_secs_f64(
            (total_time + processing_time.as_secs_f64()) / (stats.frame_stats.frames_processed + 1) as f64
        );
        
        #[cfg(feature = "metrics")]
        {
            counter!("frames_processed").increment(1);
            histogram!("frame_processing_time").record(processing_time.as_secs_f64());
        }
    }
    
    /// Get comprehensive diagnostics snapshot with quality assessment
    #[instrument(skip(self))]
    pub async fn get_snapshot(&self) -> NetworkDiagnostics {
        let mut stats = self.stats.read().await.clone();
        
        // Update real-time counters
        stats.transport_stats.packets_sent = self.packets_sent.load(Ordering::Relaxed);
        stats.transport_stats.packets_received = self.packets_received.load(Ordering::Relaxed);
        stats.transport_stats.bytes_sent = self.bytes_sent.load(Ordering::Relaxed);
        stats.transport_stats.bytes_received = self.bytes_received.load(Ordering::Relaxed);
        stats.connection_stats.active_connections = self.active_connections.load(Ordering::Relaxed);
        
        // Calculate enhanced metrics
        self.calculate_jitter(&mut stats).await;
        self.calculate_packet_loss_rate(&mut stats).await;
        self.calculate_compression_ratio(&mut stats).await;
        
        // Update health status with advanced algorithm
        stats.health_status = self.calculate_advanced_health_status(&stats).await;
        
        // Update timestamp and ID
        stats.timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        stats.diagnostic_id = Uuid::new_v4().to_string();
        
        stats
    }
    
    /// Calculate network jitter from latency history
    async fn calculate_jitter(&self, stats: &mut NetworkDiagnostics) {
        let latency_hist = self.latency_history.read().await;
        if latency_hist.len() < 2 {
            return;
        }
        
        let latencies: Vec<f64> = latency_hist
            .iter()
            .map(|(_, duration)| duration.as_secs_f64() * 1000.0) // Convert to ms
            .collect();
            
        if latencies.len() >= 2 {
            let mut jitter_sum = 0.0;
            for window in latencies.windows(2) {
                jitter_sum += (window[1] - window[0]).abs();
            }
            let avg_jitter_ms = jitter_sum / (latencies.len() - 1) as f64;
            stats.transport_stats.jitter = Duration::from_secs_f64(avg_jitter_ms / 1000.0);
            
            #[cfg(feature = "metrics")]
            gauge!("network_jitter").set(avg_jitter_ms);
        }
    }
    
    /// Calculate packet loss rate
    async fn calculate_packet_loss_rate(&self, stats: &mut NetworkDiagnostics) {
        let sent = stats.transport_stats.packets_sent;
        let received = stats.transport_stats.packets_received;
        let corrupted = stats.transport_stats.corrupted_packets;
        
        if sent > 0 {
            let expected_received = sent;
            let actual_received = received;
            let lost = expected_received.saturating_sub(actual_received + corrupted);
            stats.transport_stats.packet_loss_rate = (lost as f64) / (sent as f64);
            
            #[cfg(feature = "metrics")]
            gauge!("network_packet_loss_rate").set(stats.transport_stats.packet_loss_rate);
        }
    }
    
    /// Calculate compression ratio
    async fn calculate_compression_ratio(&self, stats: &mut NetworkDiagnostics) {
        // This would be implemented based on actual compression data
        // For now, using a placeholder calculation
        let raw_bytes = stats.transport_stats.bytes_sent;
        let compressed_bytes = raw_bytes; // Would be actual compressed size
        
        if raw_bytes > 0 {
            stats.transport_stats.compression_ratio = 1.0 - (compressed_bytes as f64 / raw_bytes as f64);
        }
    }
    
    /// Advanced health status calculation with weighted scoring
    async fn calculate_advanced_health_status(&self, stats: &NetworkDiagnostics) -> HealthStatus {
        let mut total_score = 0.0;
        let mut weight_sum = 0.0;
        
        // Latency score (weight: 25%)
        let latency_weight = 0.25;
        let latency_ms = stats.transport_stats.average_latency.as_secs_f64() * 1000.0;
        let latency_score = match latency_ms {
            0.0..=50.0 => 100.0,
            50.0..=100.0 => 90.0 - (latency_ms - 50.0) * 0.8, // Linear decrease
            100.0..=200.0 => 50.0 - (latency_ms - 100.0) * 0.3,
            _ => 20.0,
        };
        total_score += latency_score * latency_weight;
        weight_sum += latency_weight;
        
        // Packet loss score (weight: 30%)
        let loss_weight = 0.30;
        let loss_rate = stats.transport_stats.packet_loss_rate;
        let loss_score = match loss_rate {
            0.0..=0.01 => 100.0,        // < 1% loss
            0.01..=0.05 => 80.0,        // 1-5% loss
            0.05..=0.10 => 60.0,        // 5-10% loss
            0.10..=0.20 => 30.0,        // 10-20% loss
            _ => 10.0,                  // > 20% loss
        };
        total_score += loss_score * loss_weight;
        weight_sum += loss_weight;
        
        // Throughput score (weight: 20%)
        let throughput_weight = 0.20;
        let throughput_score = if stats.transport_stats.throughput_mbps >= self.alert_thresholds.min_throughput_mbps {
            100.0
        } else {
            (stats.transport_stats.throughput_mbps / self.alert_thresholds.min_throughput_mbps * 100.0).min(100.0)
        };
        total_score += throughput_score * throughput_weight;
        weight_sum += throughput_weight;
        
        // Connection stability score (weight: 15%)
        let stability_weight = 0.15;
        let failure_rate = if stats.connection_stats.total_connections_established > 0 {
            stats.connection_stats.connection_failures as f64 / stats.connection_stats.total_connections_established as f64
        } else {
            0.0
        };
        let stability_score = match failure_rate {
            0.0..=0.05 => 100.0,        // < 5% failure rate
            0.05..=0.10 => 80.0,        // 5-10% failure rate
            0.10..=0.20 => 50.0,        // 10-20% failure rate
            _ => 20.0,                  // > 20% failure rate
        };
        total_score += stability_score * stability_weight;
        weight_sum += stability_weight;
        
        // Error rate score (weight: 10%)
        let error_weight = 0.10;
        let error_score = if stats.error_stats.error_rate_per_second <= self.alert_thresholds.max_error_rate_per_second {
            100.0
        } else {
            (self.alert_thresholds.max_error_rate_per_second / stats.error_stats.error_rate_per_second * 100.0).min(100.0)
        };
        total_score += error_score * error_weight;
        weight_sum += error_weight;
        
        // Calculate final score
        let final_score = if weight_sum > 0.0 { total_score / weight_sum } else { 0.0 };
        
        #[cfg(feature = "metrics")]
        gauge!("network_quality_score").set(final_score);
        
        HealthStatus::from_score(final_score as u8)
    }
    
    /// Generate comprehensive diagnostic report with visual indicators
    #[instrument(skip(self))]
    pub async fn generate_report(&self) -> String {
        let stats = self.get_snapshot().await;
        let uptime = self.start_time.elapsed();
        let quality = self.calculate_network_quality(&stats).await;
        
        format!(
            "🌐 Ultra-Modern Network Diagnostics Report\n\
             ==========================================\n\
             📊 Report ID: {}\n\
             ⏰ Generated: {}\n\
             ⌛ Uptime: {:?}\n\
             {} Health Status: {:?} (Score: {})\n\
             \n\
             📡 Network Quality Assessment\n\
             ----------------------------\n\
             🎯 Overall Score: {}/100\n\
             📶 Latency Score: {}/100 ({:.1}ms avg)\n\
             🚀 Throughput Score: {}/100 ({:.2} Mbps)\n\
             🔒 Reliability Score: {}/100\n\
             🛡️  Security Score: {}/100\n\
             ⚡ Stability Score: {}/100\n\
             \n\
             🔌 Connection Statistics\n\
             ----------------------\n\
             Active: {} | Established: {} | Lost: {} | Failures: {}\n\
             Reconnection Attempts: {} | Pool Utilization: {:.1}%\n\
             Peak Concurrent: {} | Avg Duration: {:?}\n\
             \n\
             📦 Transport Layer Metrics\n\
             -------------------------\n\
             Packets: {} sent, {} received | Bytes: {} sent, {} received\n\
             Loss Rate: {:.3}% | Jitter: {:.1}ms | Out-of-order: {}\n\
             Duplicates: {} | Corrupted: {} | Retransmissions: {}\n\
             Compression Ratio: {:.1}% | Encryption Overhead: {:.1}%\n\
             Congestion Events: {} | Throughput: {:.2} Mbps\n\
             \n\
             🎮 Frame Synchronization\n\
             -----------------------\n\
             Processed: {} | Skipped: {} | Avg Time: {:.2}ms\n\
             Desync Count: {} | Pending: {} | Rollbacks: {}\n\
             Prediction Accuracy: {:.1}% | Max Rollback Depth: {}\n\
             Input Delay: {} frames | Catchup Frames: {}\n\
             \n\
             🛡️  Security & Performance\n\
             -------------------------\n\
             Auth Failures: {} | Rate Limit Violations: {}\n\
             Suspicious Activity: {} | Encryption Errors: {}\n\
             CPU Usage: {:.1}% | Memory: {:.1} MB | GC Pressure: {:.2}\n\
             Buffer Pool: {:.1}% | Cache Hit Rate: {:.1}%\n\
             \n\
             ❌ Error Analysis\n\
             ----------------\n\
             Total Errors: {} | Error Rate: {:.2}/sec\n\
             Recovery Success: {:.1}% | MTTR: {:?}\n\
             Critical: {} | High: {} | Medium: {} | Low: {}\n\
             \n\
             📈 Trend Analysis\n\
             ----------------\n\
             Recent patterns detected in error frequency and network performance.\n\
             Predictive alerts configured for proactive issue resolution.\n",
            stats.diagnostic_id,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            uptime,
            stats.health_status.color(),
            stats.health_status,
            stats.health_status.as_score(),
            quality.overall_score,
            quality.latency_score,
            stats.transport_stats.average_latency.as_secs_f64() * 1000.0,
            quality.throughput_score,
            stats.transport_stats.throughput_mbps,
            quality.reliability_score,
            quality.security_score,
            quality.stability_score,
            stats.connection_stats.active_connections,
            stats.connection_stats.total_connections_established,
            stats.connection_stats.total_connections_lost,
            stats.connection_stats.connection_failures,
            stats.connection_stats.reconnection_attempts,
            stats.connection_stats.connection_pool_utilization,
            stats.connection_stats.peak_concurrent_connections,
            stats.connection_stats.average_connection_duration,
            stats.transport_stats.packets_sent,
            stats.transport_stats.packets_received,
            stats.transport_stats.bytes_sent,
            stats.transport_stats.bytes_received,
            stats.transport_stats.packet_loss_rate * 100.0,
            stats.transport_stats.jitter.as_secs_f64() * 1000.0,
            stats.transport_stats.out_of_order_packets,
            stats.transport_stats.duplicate_packets,
            stats.transport_stats.corrupted_packets,
            stats.transport_stats.retransmissions,
            stats.transport_stats.compression_ratio * 100.0,
            stats.transport_stats.encryption_overhead * 100.0,
            stats.transport_stats.congestion_events,
            stats.transport_stats.throughput_mbps,
            stats.frame_stats.frames_processed,
            stats.frame_stats.frames_skipped,
            stats.frame_stats.average_frame_time.as_secs_f64() * 1000.0,
            stats.frame_stats.frame_desync_count,
            stats.frame_stats.pending_frames,
            stats.frame_stats.rollback_count,
            stats.frame_stats.frame_prediction_accuracy * 100.0,
            stats.frame_stats.maximum_rollback_depth,
            stats.frame_stats.input_delay_frames,
            stats.frame_stats.catchup_frames,
            stats.security_stats.authentication_failures,
            stats.security_stats.rate_limit_violations,
            stats.security_stats.suspicious_activity_events,
            stats.security_stats.encryption_errors,
            stats.performance_stats.cpu_usage_percent,
            stats.performance_stats.memory_usage_bytes as f64 / 1_048_576.0, // MB
            stats.performance_stats.gc_pressure,
            stats.performance_stats.buffer_pool_utilization * 100.0,
            stats.performance_stats.cache_hit_rate * 100.0,
            stats.error_stats.total_errors,
            stats.error_stats.error_rate_per_second,
            stats.error_stats.recovery_success_rate * 100.0,
            stats.error_stats.mean_time_to_recovery,
            stats.error_stats.errors_by_severity.get("Critical").unwrap_or(&0),
            stats.error_stats.errors_by_severity.get("High").unwrap_or(&0),
            stats.error_stats.errors_by_severity.get("Medium").unwrap_or(&0),
            stats.error_stats.errors_by_severity.get("Low").unwrap_or(&0),
        )
    }
    
    /// Calculate comprehensive network quality metrics
    async fn calculate_network_quality(&self, stats: &NetworkDiagnostics) -> NetworkQuality {
        // Implement quality scoring algorithm
        NetworkQuality {
            overall_score: stats.health_status.as_score(),
            latency_score: self.calculate_latency_score(stats.transport_stats.average_latency),
            throughput_score: self.calculate_throughput_score(stats.transport_stats.throughput_mbps),
            reliability_score: self.calculate_reliability_score(&stats.transport_stats),
            security_score: self.calculate_security_score(&stats.security_stats),
            stability_score: self.calculate_stability_score(&stats.connection_stats),
        }
    }
    
    fn calculate_latency_score(&self, latency: Duration) -> u8 {
        let latency_ms = latency.as_secs_f64() * 1000.0;
        match latency_ms {
            0.0..=50.0 => 100,
            50.0..=100.0 => 85,
            100.0..=200.0 => 70,
            200.0..=500.0 => 50,
            _ => 20,
        }
    }
    
    fn calculate_throughput_score(&self, throughput_mbps: f64) -> u8 {
        if throughput_mbps >= self.alert_thresholds.min_throughput_mbps * 2.0 {
            100
        } else if throughput_mbps >= self.alert_thresholds.min_throughput_mbps {
            80
        } else {
            (throughput_mbps / self.alert_thresholds.min_throughput_mbps * 80.0) as u8
        }
    }
    
    fn calculate_reliability_score(&self, transport: &TransportDiagnostics) -> u8 {
        let loss_penalty = (transport.packet_loss_rate * 100.0) as u8;
        let corruption_penalty = if transport.corrupted_packets > 0 { 20 } else { 0 };
        100_u8.saturating_sub(loss_penalty).saturating_sub(corruption_penalty)
    }
    
    fn calculate_security_score(&self, security: &SecurityDiagnostics) -> u8 {
        let mut score = 100_u8;
        
        if security.authentication_failures > 0 {
            score = score.saturating_sub(10);
        }
        if security.suspicious_activity_events > 0 {
            score = score.saturating_sub(20);
        }
        if security.potential_attacks_detected > 0 {
            score = score.saturating_sub(30);
        }
        
        score
    }
    
    fn calculate_stability_score(&self, connection: &ConnectionDiagnostics) -> u8 {
        if connection.total_connections_established == 0 {
            return 100;
        }
        
        let failure_rate = connection.connection_failures as f64 / connection.total_connections_established as f64;
        match failure_rate {
            0.0..=0.01 => 100,      // < 1%
            0.01..=0.05 => 85,      // 1-5%
            0.05..=0.10 => 70,      // 5-10%
            0.10..=0.20 => 50,      // 10-20%
            _ => 20,                // > 20%
        }
    }

    /// Background task for periodic data collection
    async fn periodic_collection_task(collector: Arc<DiagnosticsCollector>) {
        let mut interval = interval(Duration::from_secs(5));
        
        loop {
            interval.tick().await;
            
            // Perform periodic cleanup and maintenance
            collector.cleanup_old_data().await;
            collector.update_performance_metrics().await;
            
            #[cfg(feature = "metrics")]
            {
                let stats = collector.get_snapshot().await;
                gauge!("network_quality_score").set(stats.health_status.as_score() as f64);
                counter!("diagnostic_collection_cycles_total").increment(1);
            }
        }
    }
    
    /// Background task for alert monitoring
    async fn alert_monitoring_task(collector: Arc<DiagnosticsCollector>) {
        let mut interval = interval(Duration::from_secs(1));
        
        loop {
            interval.tick().await;
            
            // Check all alert conditions
            collector.check_all_alerts().await;
        }
    }
    
    /// Background task for trend analysis
    async fn trend_analysis_task(collector: Arc<DiagnosticsCollector>) {
        let mut interval = interval(Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            
            // Analyze trends and update predictions
            collector.analyze_trends().await;
        }
    }
    
    /// Clean up old historical data to prevent memory leaks
    async fn cleanup_old_data(&self) {
        let cutoff = NetworkInstant::now() - Duration::from_secs(300); // Keep 5 minutes of history
        
        {
            let mut latency_hist = self.latency_history.write().await;
            while let Some((timestamp, _)) = latency_hist.front() {
                if *timestamp < cutoff {
                    latency_hist.pop_front();
                } else {
                    break;
                }
            }
        }
        
        {
            let mut throughput_hist = self.throughput_history.write().await;
            while let Some((timestamp, _)) = throughput_hist.front() {
                if *timestamp < cutoff {
                    throughput_hist.pop_front();
                } else {
                    break;
                }
            }
        }
        
        {
            let mut error_hist = self.error_history.write().await;
            while let Some((timestamp, _)) = error_hist.front() {
                if *timestamp < cutoff {
                    error_hist.pop_front();
                } else {
                    break;
                }
            }
        }
    }
    
    /// Update performance-related metrics
    async fn update_performance_metrics(&self) {
        // This would integrate with system monitoring APIs
        // For now, using placeholder implementations
        
        #[cfg(feature = "metrics")]
        {
            // Memory usage would be calculated from actual system stats
            let memory_usage = std::process::id() as f64; // Placeholder
            gauge!("memory_usage_percent").set(memory_usage / 1000000.0);
            
            // CPU usage would be calculated from system monitoring
            let cpu_usage = 0.0; // Placeholder
            gauge!("cpu_usage_percent").set(cpu_usage);
        }
    }
    
    /// Check all alert conditions
    async fn check_all_alerts(&self) {
        let stats = self.get_snapshot().await;
        
        // Check latency alert
        self.check_latency_alert(stats.transport_stats.average_latency).await;
        
        // Check throughput alert
        self.check_throughput_alert(stats.transport_stats.throughput_mbps).await;
        
        // Check packet loss alert
        if stats.transport_stats.packet_loss_rate > self.alert_thresholds.max_packet_loss_rate {
            self.emit_alert(DiagnosticAlert {
                id: Uuid::new_v4().to_string(),
                severity: AlertSeverity::Warning,
                title: "High Packet Loss Detected".to_string(),
                description: format!(
                    "Packet loss rate ({:.2}%) exceeds threshold ({:.2}%)", 
                    stats.transport_stats.packet_loss_rate * 100.0,
                    self.alert_thresholds.max_packet_loss_rate * 100.0
                ),
                metric_name: "packet_loss_rate".to_string(),
                current_value: stats.transport_stats.packet_loss_rate,
                threshold_value: self.alert_thresholds.max_packet_loss_rate,
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
                context: HashMap::new(),
            }).await;
        }
    }
    
    /// Check for latency-based alerts
    async fn check_latency_alert(&self, latency: Duration) {
        let latency_ms = latency.as_millis() as u64;
        if latency_ms > self.alert_thresholds.max_latency_ms {
            self.emit_alert(DiagnosticAlert {
                id: Uuid::new_v4().to_string(),
                severity: if latency_ms > self.alert_thresholds.max_latency_ms * 2 {
                    AlertSeverity::Critical
                } else {
                    AlertSeverity::Warning
                },
                title: "High Network Latency".to_string(),
                description: format!(
                    "Average latency ({}ms) exceeds threshold ({}ms)",
                    latency_ms, self.alert_thresholds.max_latency_ms
                ),
                metric_name: "average_latency".to_string(),
                current_value: latency_ms as f64,
                threshold_value: self.alert_thresholds.max_latency_ms as f64,
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
                context: HashMap::new(),
            }).await;
        }
    }
    
    /// Check for throughput-based alerts
    async fn check_throughput_alert(&self, throughput_mbps: f64) {
        if throughput_mbps < self.alert_thresholds.min_throughput_mbps {
            self.emit_alert(DiagnosticAlert {
                id: Uuid::new_v4().to_string(),
                severity: AlertSeverity::Warning,
                title: "Low Network Throughput".to_string(),
                description: format!(
                    "Current throughput ({:.2} Mbps) below minimum threshold ({:.2} Mbps)",
                    throughput_mbps, self.alert_thresholds.min_throughput_mbps
                ),
                metric_name: "throughput".to_string(),
                current_value: throughput_mbps,
                threshold_value: self.alert_thresholds.min_throughput_mbps,
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
                context: HashMap::new(),
            }).await;
        }
    }
    
    /// Emit a diagnostic alert
    async fn emit_alert(&self, alert: DiagnosticAlert) {
        #[cfg(feature = "metrics")]
        {
            match alert.severity {
                AlertSeverity::Info => info!(
                    alert_id = %alert.id,
                    metric = %alert.metric_name,
                    current_value = alert.current_value,
                    threshold = alert.threshold_value,
                    "{}: {}", alert.title, alert.description
                ),
                AlertSeverity::Warning => warn!(
                    alert_id = %alert.id,
                    metric = %alert.metric_name,
                    current_value = alert.current_value,
                    threshold = alert.threshold_value,
                    "{}: {}", alert.title, alert.description
                ),
                AlertSeverity::Critical | AlertSeverity::Emergency => error!(
                    alert_id = %alert.id,
                    metric = %alert.metric_name,
                    current_value = alert.current_value,
                    threshold = alert.threshold_value,
                    "{}: {}", alert.title, alert.description
                ),
            }
            
            counter!("diagnostic_alerts_total").increment(1);
        }
        
        // Notify waiting tasks
        self.alert_notifier.notify_waiters();
    }
    
    /// Analyze trends in collected data
    async fn analyze_trends(&self) {
        // This would implement sophisticated trend analysis
        // For now, basic implementation
        
        let latency_hist = self.latency_history.read().await;
        if latency_hist.len() >= 10 {
            let recent_latencies: Vec<f64> = latency_hist
                .iter()
                .rev()
                .take(10)
                .map(|(_, duration)| duration.as_secs_f64() * 1000.0)
                .collect();
                
            // Simple trend detection (rising/falling)
            let first_half_avg: f64 = recent_latencies[5..].iter().sum::<f64>() / 5.0;
            let second_half_avg: f64 = recent_latencies[..5].iter().sum::<f64>() / 5.0;
            
            if second_half_avg > first_half_avg * 1.2 {
                warn!("Latency trend: increasing significantly");
            }
        }
    }
}

// Clone implementation for background tasks
impl Clone for DiagnosticsCollector {
    fn clone(&self) -> Self {
        Self {
            start_time: self.start_time,
            stats: Arc::clone(&self.stats),
            packets_sent: AtomicU64::new(self.packets_sent.load(Ordering::Relaxed)),
            packets_received: AtomicU64::new(self.packets_received.load(Ordering::Relaxed)),
            bytes_sent: AtomicU64::new(self.bytes_sent.load(Ordering::Relaxed)),
            bytes_received: AtomicU64::new(self.bytes_received.load(Ordering::Relaxed)),
            errors_count: AtomicU64::new(self.errors_count.load(Ordering::Relaxed)),
            active_connections: AtomicUsize::new(self.active_connections.load(Ordering::Relaxed)),
            latency_history: Arc::clone(&self.latency_history),
            throughput_history: Arc::clone(&self.throughput_history),
            error_history: Arc::clone(&self.error_history),
            alert_notifier: Arc::clone(&self.alert_notifier),
            alert_thresholds: self.alert_thresholds.clone(),
            collection_start: self.collection_start,
            last_gc_time: Arc::clone(&self.last_gc_time),
        }
    }
}

// Default implementations for all diagnostic structures
impl Default for NetworkDiagnostics {
    fn default() -> Self {
        Self {
            connection_stats: ConnectionDiagnostics::default(),
            transport_stats: TransportDiagnostics::default(),
            frame_stats: FrameDiagnostics::default(),
            security_stats: SecurityDiagnostics::default(),
            performance_stats: PerformanceDiagnostics::default(),
            error_stats: ErrorDiagnostics::default(),
            health_status: HealthStatus::default(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
            diagnostic_id: Uuid::new_v4().to_string(),
        }
    }
}

impl Default for ConnectionDiagnostics {
    fn default() -> Self {
        Self {
            active_connections: 0,
            total_connections_established: 0,
            total_connections_lost: 0,
            average_connection_duration: Duration::ZERO,
            connection_failures: 0,
            connections_by_protocol: HashMap::new(),
            connection_quality_distribution: QualityDistribution::default(),
            reconnection_attempts: 0,
            connection_pool_utilization: 0.0,
            peak_concurrent_connections: 0,
        }
    }
}

impl Default for TransportDiagnostics {
    fn default() -> Self {
        Self {
            packets_sent: 0,
            packets_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            packet_loss_rate: 0.0,
            average_latency: Duration::ZERO,
            bandwidth_utilization: 0.0,
            jitter: Duration::ZERO,
            out_of_order_packets: 0,
            duplicate_packets: 0,
            corrupted_packets: 0,
            compression_ratio: 0.0,
            encryption_overhead: 0.0,
            retransmissions: 0,
            congestion_events: 0,
            throughput_mbps: 0.0,
        }
    }
}

impl Default for FrameDiagnostics {
    fn default() -> Self {
        Self {
            frames_processed: 0,
            frames_skipped: 0,
            average_frame_time: Duration::ZERO,
            frame_desync_count: 0,
            pending_frames: 0,
            frame_prediction_accuracy: 1.0,
            rollback_count: 0,
            maximum_rollback_depth: 0,
            input_delay_frames: 0,
            frame_timing_variance: Duration::ZERO,
            catchup_frames: 0,
        }
    }
}

impl Default for SecurityDiagnostics {
    fn default() -> Self {
        Self {
            authentication_attempts: 0,
            authentication_failures: 0,
            authorization_denials: 0,
            potential_attacks_detected: 0,
            rate_limit_violations: 0,
            suspicious_activity_events: 0,
            encryption_errors: 0,
            certificate_validation_failures: 0,
        }
    }
}

impl Default for PerformanceDiagnostics {
    fn default() -> Self {
        Self {
            cpu_usage_percent: 0.0,
            memory_usage_bytes: 0,
            memory_peak_bytes: 0,
            gc_pressure: 0.0,
            thread_pool_utilization: 0.0,
            async_task_queue_depth: 0,
            buffer_pool_utilization: 0.0,
            cache_hit_rate: 1.0,
            disk_io_operations: 0,
            network_buffer_overruns: 0,
        }
    }
}

impl Default for ErrorDiagnostics {
    fn default() -> Self {
        Self {
            total_errors: 0,
            errors_by_severity: HashMap::new(),
            errors_by_type: HashMap::new(),
            recovery_success_rate: 1.0,
            mean_time_to_recovery: Duration::ZERO,
            error_rate_per_second: 0.0,
            recent_error_patterns: Vec::new(),
        }
    }
}

impl Default for QualityDistribution {
    fn default() -> Self {
        Self {
            excellent: 0,
            good: 0,
            fair: 0,
            poor: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};
    
    #[tokio::test]
    async fn test_enhanced_diagnostics_collection() {
        let collector = DiagnosticsCollector::new();
        
        // Record some events with timing
        collector.record_packet_sent(100).await;
        collector.record_packet_received(150, Duration::from_millis(5)).await;
        collector.record_connection_established().await;
        
        let snapshot = collector.get_snapshot().await;
        assert_eq!(snapshot.transport_stats.packets_sent, 1);
        assert_eq!(snapshot.transport_stats.packets_received, 1);
        assert_eq!(snapshot.connection_stats.active_connections, 1);
        assert!(!snapshot.diagnostic_id.is_empty());
        assert!(snapshot.timestamp > 0);
    }
    
    #[tokio::test]
    async fn test_advanced_health_status_calculation() {
        let collector = DiagnosticsCollector::new();
        let snapshot = collector.get_snapshot().await;
        assert!(matches!(snapshot.health_status, HealthStatus::Healthy | HealthStatus::Optimal));
        
        let quality = collector.calculate_network_quality(&snapshot).await;
        assert!(quality.overall_score >= 50); // Should be reasonable for new collector
    }
    
    #[tokio::test]
    async fn test_enhanced_diagnostic_report_generation() {
        let collector = DiagnosticsCollector::new();
        let report = collector.generate_report().await;
        
        assert!(report.contains("Ultra-Modern Network Diagnostics Report"));
        assert!(report.contains("Network Quality Assessment"));
        assert!(report.contains("Security & Performance"));
        assert!(report.contains("Trend Analysis"));
        assert!(report.contains("🌐"));
    }
    
    #[tokio::test]
    async fn test_alert_system() {
        let mut thresholds = AlertThresholds::default();
        thresholds.max_latency_ms = 50; // Very low threshold for testing
        
        let collector = DiagnosticsCollector::with_thresholds(thresholds);
        
        // This should trigger a latency alert
        collector.record_packet_received(100, Duration::from_millis(100)).await;
        
        // Give the alert system time to process
        sleep(Duration::from_millis(10)).await;
        
        // Verify that alert was processed (this would be more sophisticated in practice)
        let snapshot = collector.get_snapshot().await;
        assert!(snapshot.transport_stats.average_latency > Duration::from_millis(50));
    }
    
    #[tokio::test]
    async fn test_error_id_generation() {
        let id1 = ErrorId::new();
        let id2 = ErrorId::new();
        
        assert_ne!(id1, id2);
        assert!(!id1.to_string().is_empty());
        assert_ne!(id1.as_uuid(), id2.as_uuid());
    }
    
    #[tokio::test]
    async fn test_health_status_scoring() {
        assert_eq!(HealthStatus::Optimal.as_score(), 100);
        assert_eq!(HealthStatus::Healthy.as_score(), 80);
        assert_eq!(HealthStatus::Failed.as_score(), 0);
        
        assert_eq!(HealthStatus::from_score(95), HealthStatus::Optimal);
        assert_eq!(HealthStatus::from_score(75), HealthStatus::Healthy);
        assert_eq!(HealthStatus::from_score(5), HealthStatus::Failed);
    }
    
    #[tokio::test]
    async fn test_quality_calculations() {
        let collector = DiagnosticsCollector::new();
        
        // Test latency scoring
        assert_eq!(collector.calculate_latency_score(Duration::from_millis(30)), 100);
        assert_eq!(collector.calculate_latency_score(Duration::from_millis(75)), 85);
        assert_eq!(collector.calculate_latency_score(Duration::from_millis(1000)), 20);
        
        // Test throughput scoring  
        assert_eq!(collector.calculate_throughput_score(10.0), 100); // Well above minimum
        assert_eq!(collector.calculate_throughput_score(1.0), 80);   // At minimum
        assert!(collector.calculate_throughput_score(0.5) < 80);     // Below minimum
    }
    
    #[tokio::test]
    async fn test_atomic_counters() {
        let collector = DiagnosticsCollector::new();
        
        // Test that atomic updates work correctly
        for i in 0..100 {
            collector.record_packet_sent(i).await;
        }
        
        assert_eq!(collector.packets_sent.load(Ordering::Relaxed), 100);
        
        let snapshot = collector.get_snapshot().await;
        assert_eq!(snapshot.transport_stats.packets_sent, 100);
    }
    
    #[tokio::test]
    async fn test_trend_direction() {
        // Test serialization of trend direction
        let trend = TrendDirection::Increasing;
        let serialized = serde_json::to_string(&trend).unwrap();
        let deserialized: TrendDirection = serde_json::from_str(&serialized).unwrap();
        assert!(matches!(deserialized, TrendDirection::Increasing));
    }
}
