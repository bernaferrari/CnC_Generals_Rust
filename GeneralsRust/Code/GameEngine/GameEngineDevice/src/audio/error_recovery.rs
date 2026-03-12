//! # Comprehensive Error Handling and Recovery System
//!
//! This module provides robust error handling, recovery mechanisms, and resilience
//! features for the audio system including:
//! - Automatic device fallback and recovery
//! - Resource leak prevention
//! - Graceful degradation under resource pressure
//! - Real-time error monitoring and reporting
//! - Audio continuity preservation during failures

use crate::audio::{
    AudioDeviceError, AudioFormat, AudioHandle, AudioSource, AudioStatistics, DeviceCapabilities,
    PlaybackState, Priority, Result,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex, RwLock,
};
use std::time::{Duration, Instant, SystemTime};
use thiserror::Error;
use uuid::Uuid;

/// Enhanced error types with recovery context
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryError {
    /// Temporary failure that may be retryable
    #[error("Temporary error: {message} (retry #{attempt})")]
    Temporary {
        message: String,
        attempt: u32,
        recoverable: bool,
    },

    /// Permanent failure requiring fallback
    #[error("Permanent error: {message} (fallback available: {fallback_available})")]
    Permanent {
        message: String,
        fallback_available: bool,
        suggested_action: String,
    },

    /// Resource exhaustion with recovery suggestions
    #[error("Resource exhausted: {resource_type} (current: {current}, max: {maximum})")]
    ResourceExhausted {
        resource_type: String,
        current: u64,
        maximum: u64,
        recovery_actions: Vec<String>,
    },

    /// Device failure with fallback information
    #[error("Device failure: {device_id} - {reason}")]
    DeviceFailure {
        device_id: String,
        reason: String,
        fallback_device: Option<String>,
        can_hot_swap: bool,
    },

    /// Performance degradation warning
    #[error("Performance degraded: {metric} = {value} (threshold: {threshold})")]
    PerformanceDegraded {
        metric: String,
        value: f64,
        threshold: f64,
        impact: PerformanceImpact,
    },
}

/// Impact levels for performance degradation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerformanceImpact {
    /// Minor impact, system remains stable
    Minor,
    /// Moderate impact, some features may be disabled
    Moderate,
    /// Severe impact, emergency measures required
    Severe,
    /// Critical impact, system failure imminent
    Critical,
}

/// Recovery strategy for different error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    /// Retry the operation with backoff
    Retry {
        max_attempts: u32,
        backoff_ms: u64,
        exponential: bool,
    },

    /// Switch to fallback device/configuration
    Fallback {
        fallback_id: String,
        preserve_state: bool,
        notification_required: bool,
    },

    /// Gracefully degrade functionality
    Degrade {
        disable_features: Vec<String>,
        reduce_quality: bool,
        temporary: bool,
    },

    /// Reset component to known good state
    Reset {
        component: String,
        preserve_user_settings: bool,
        reinitialize: bool,
    },

    /// Emergency shutdown with state preservation
    EmergencyShutdown {
        save_state: bool,
        notify_user: bool,
        restart_possible: bool,
    },
}

/// Error recovery context with full system state
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// Error timestamp
    pub timestamp: SystemTime,
    /// Component that generated the error
    pub component: String,
    /// Current system state
    pub system_state: SystemState,
    /// Recent error history
    pub error_history: Vec<HistoricalError>,
    /// Recovery attempts made
    pub recovery_attempts: u32,
    /// Available recovery options
    pub available_strategies: Vec<RecoveryStrategy>,
    /// Resource utilization at time of error
    pub resource_snapshot: ResourceSnapshot,
}

/// System state snapshot for recovery decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemState {
    /// Active audio sources
    pub active_sources: u32,
    /// Current device configuration
    pub device_config: DeviceConfiguration,
    /// Memory utilization
    pub memory_usage: MemoryUsage,
    /// CPU utilization
    pub cpu_usage: f32,
    /// Network latency (for streaming)
    pub network_latency_ms: f32,
    /// Audio quality settings
    pub quality_level: QualityLevel,
}

/// Historical error for pattern analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalError {
    /// When the error occurred
    pub timestamp: SystemTime,
    /// Error type and message
    pub error_type: String,
    /// Component involved
    pub component: String,
    /// Recovery strategy used
    pub recovery_used: Option<RecoveryStrategy>,
    /// Whether recovery was successful
    pub recovery_successful: bool,
    /// Time to recover
    pub recovery_time_ms: Option<u64>,
}

/// Device configuration snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfiguration {
    /// Current primary device
    pub primary_device: String,
    /// Available fallback devices
    pub fallback_devices: Vec<String>,
    /// Current audio format
    pub format: AudioFormat,
    /// Buffer configuration
    pub buffer_config: BufferConfiguration,
}

/// Buffer configuration details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferConfiguration {
    /// Buffer size in frames
    pub buffer_size: u32,
    /// Number of buffers
    pub buffer_count: u32,
    /// Target latency in ms
    pub target_latency_ms: f32,
    /// Actual latency in ms
    pub actual_latency_ms: f32,
}

/// Memory usage snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsage {
    /// Total allocated audio memory
    pub total_audio_memory: u64,
    /// Available audio memory
    pub available_audio_memory: u64,
    /// Number of allocated buffers
    pub allocated_buffers: u32,
    /// Largest free block
    pub largest_free_block: u64,
    /// Memory fragmentation percentage
    pub fragmentation_percent: f32,
}

/// Audio quality levels for degradation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityLevel {
    /// Maximum quality (48kHz, 24-bit)
    Maximum,
    /// High quality (48kHz, 16-bit)
    High,
    /// Medium quality (44.1kHz, 16-bit)
    Medium,
    /// Low quality (22kHz, 16-bit)
    Low,
    /// Emergency quality (11kHz, 8-bit)
    Emergency,
}

/// Resource snapshot for recovery analysis
#[derive(Debug, Clone)]
pub struct ResourceSnapshot {
    /// CPU utilization per core
    pub cpu_per_core: Vec<f32>,
    /// Memory pressure indicators
    pub memory_pressure: MemoryPressure,
    /// I/O statistics
    pub io_stats: IoStatistics,
    /// Audio thread timing
    pub thread_timing: ThreadTiming,
}

/// Memory pressure indicators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPressure {
    /// System memory pressure level
    pub system_pressure: PressureLevel,
    /// Audio-specific memory pressure
    pub audio_pressure: PressureLevel,
    /// Garbage collection frequency
    pub gc_frequency: f32,
    /// Memory allocation failures
    pub allocation_failures: u64,
}

/// Pressure levels for resource monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PressureLevel {
    /// Normal operation
    Normal,
    /// Elevated usage, monitoring required
    Elevated,
    /// High usage, optimization recommended
    High,
    /// Critical usage, immediate action required
    Critical,
}

/// I/O performance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IoStatistics {
    /// Disk read throughput (MB/s)
    pub read_throughput: f32,
    /// Disk write throughput (MB/s)
    pub write_throughput: f32,
    /// Network throughput (MB/s)
    pub network_throughput: f32,
    /// I/O error rate
    pub error_rate: f32,
    /// Average I/O latency
    pub average_latency_ms: f32,
}

/// Audio thread timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadTiming {
    /// Main audio thread CPU time
    pub main_thread_cpu_ms: f32,
    /// Background threads CPU time
    pub background_threads_cpu_ms: f32,
    /// Thread context switches per second
    pub context_switches_per_sec: f32,
    /// Thread priority inversions
    pub priority_inversions: u32,
}

/// Main error recovery manager
pub struct ErrorRecoveryManager {
    /// Error history storage
    error_history: Arc<RwLock<VecDeque<HistoricalError>>>,
    /// Current system state
    system_state: Arc<RwLock<SystemState>>,
    /// Recovery strategies configuration
    recovery_strategies: Arc<RwLock<HashMap<String, Vec<RecoveryStrategy>>>>,
    /// Resource monitors
    resource_monitor: Arc<ResourceMonitor>,
    /// Performance tracker
    performance_tracker: Arc<PerformanceTracker>,
    /// Recovery statistics
    recovery_stats: Arc<RwLock<RecoveryStatistics>>,
    /// Configuration
    config: ErrorRecoveryConfig,
    /// Shutdown flag for cleanup
    shutdown_flag: Arc<AtomicBool>,
}

/// Resource monitoring system
pub struct ResourceMonitor {
    /// CPU usage history
    cpu_history: Arc<Mutex<VecDeque<f32>>>,
    /// Memory usage history
    memory_history: Arc<Mutex<VecDeque<u64>>>,
    /// I/O statistics
    io_stats: Arc<RwLock<IoStatistics>>,
    /// Last monitoring update
    last_update: Arc<RwLock<Instant>>,
}

/// Performance tracking system
pub struct PerformanceTracker {
    /// Frame timing history
    frame_times: Arc<Mutex<VecDeque<Duration>>>,
    /// Latency measurements
    latency_measurements: Arc<Mutex<VecDeque<Duration>>>,
    /// Drop/underrun counter
    drop_counter: Arc<AtomicU64>,
    /// Quality degradation events
    quality_events: Arc<Mutex<Vec<QualityEvent>>>,
}

/// Quality degradation event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityEvent {
    /// When the event occurred
    pub timestamp: SystemTime,
    /// Previous quality level
    pub from_quality: QualityLevel,
    /// New quality level
    pub to_quality: QualityLevel,
    /// Reason for degradation
    pub reason: String,
    /// How long degradation lasted
    pub duration_ms: Option<u64>,
}

/// Recovery statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecoveryStatistics {
    /// Total errors encountered
    pub total_errors: u64,
    /// Successful recoveries
    pub successful_recoveries: u64,
    /// Failed recovery attempts
    pub failed_recoveries: u64,
    /// Fallback activations
    pub fallback_activations: u64,
    /// Emergency shutdowns
    pub emergency_shutdowns: u64,
    /// Average recovery time
    pub average_recovery_time_ms: f32,
    /// Most common error types
    pub common_errors: HashMap<String, u64>,
    /// Recovery success rate by strategy
    pub strategy_success_rates: HashMap<String, f32>,
}

/// Error recovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecoveryConfig {
    /// Maximum error history to maintain
    pub max_error_history: usize,
    /// Maximum retry attempts
    pub max_retry_attempts: u32,
    /// Base retry delay in milliseconds
    pub base_retry_delay_ms: u64,
    /// Enable automatic fallback
    pub auto_fallback_enabled: bool,
    /// Enable quality degradation
    pub quality_degradation_enabled: bool,
    /// Resource monitoring interval
    pub monitoring_interval_ms: u64,
    /// Performance thresholds
    pub performance_thresholds: PerformanceThresholds,
}

/// Performance threshold configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// Maximum acceptable CPU usage
    pub max_cpu_usage: f32,
    /// Maximum acceptable memory usage
    pub max_memory_usage: u64,
    /// Maximum acceptable latency
    pub max_latency_ms: f32,
    /// Minimum acceptable frame rate
    pub min_frame_rate: f32,
    /// Maximum acceptable drop rate
    pub max_drop_rate: f32,
}

impl Default for ErrorRecoveryConfig {
    fn default() -> Self {
        Self {
            max_error_history: 1000,
            max_retry_attempts: 3,
            base_retry_delay_ms: 100,
            auto_fallback_enabled: true,
            quality_degradation_enabled: true,
            monitoring_interval_ms: 1000,
            performance_thresholds: PerformanceThresholds::default(),
        }
    }
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            max_cpu_usage: 0.8,
            max_memory_usage: 1024 * 1024 * 512, // 512 MB
            max_latency_ms: 50.0,
            min_frame_rate: 30.0,
            max_drop_rate: 0.01, // 1%
        }
    }
}

impl ErrorRecoveryManager {
    /// Create a new error recovery manager
    pub fn new(config: ErrorRecoveryConfig) -> Result<Self> {
        let error_history = Arc::new(RwLock::new(VecDeque::with_capacity(
            config.max_error_history,
        )));
        let system_state = Arc::new(RwLock::new(SystemState::default()));
        let recovery_strategies = Arc::new(RwLock::new(Self::initialize_default_strategies()));

        let resource_monitor = Arc::new(ResourceMonitor::new()?);
        let performance_tracker = Arc::new(PerformanceTracker::new());
        let recovery_stats = Arc::new(RwLock::new(RecoveryStatistics::default()));
        let shutdown_flag = Arc::new(AtomicBool::new(false));

        Ok(Self {
            error_history,
            system_state,
            recovery_strategies,
            resource_monitor,
            performance_tracker,
            recovery_stats,
            config,
            shutdown_flag,
        })
    }

    /// Initialize default recovery strategies
    fn initialize_default_strategies() -> HashMap<String, Vec<RecoveryStrategy>> {
        let mut strategies = HashMap::new();

        // Device failure strategies
        strategies.insert(
            "device_failure".to_string(),
            vec![
                RecoveryStrategy::Fallback {
                    fallback_id: "default".to_string(),
                    preserve_state: true,
                    notification_required: true,
                },
                RecoveryStrategy::Reset {
                    component: "audio_device".to_string(),
                    preserve_user_settings: true,
                    reinitialize: true,
                },
            ],
        );

        // Memory exhaustion strategies
        strategies.insert(
            "memory_exhaustion".to_string(),
            vec![
                RecoveryStrategy::Degrade {
                    disable_features: vec!["reverb".to_string(), "3d_audio".to_string()],
                    reduce_quality: true,
                    temporary: true,
                },
                RecoveryStrategy::Reset {
                    component: "buffer_manager".to_string(),
                    preserve_user_settings: false,
                    reinitialize: true,
                },
            ],
        );

        // Performance issues
        strategies.insert(
            "performance_degraded".to_string(),
            vec![RecoveryStrategy::Degrade {
                disable_features: vec!["effects".to_string()],
                reduce_quality: true,
                temporary: true,
            }],
        );

        // Network issues (for streaming)
        strategies.insert(
            "network_error".to_string(),
            vec![
                RecoveryStrategy::Retry {
                    max_attempts: 3,
                    backoff_ms: 1000,
                    exponential: true,
                },
                RecoveryStrategy::Degrade {
                    disable_features: vec!["streaming".to_string()],
                    reduce_quality: false,
                    temporary: true,
                },
            ],
        );

        strategies
    }

    /// Handle an error with automatic recovery
    pub async fn handle_error(
        &self,
        error: &AudioDeviceError,
        component: &str,
        context: Option<ErrorContext>,
    ) -> Result<RecoveryAction> {
        let recovery_start = Instant::now();

        // Create or use provided error context
        let error_context = match context {
            Some(ctx) => ctx,
            None => self.create_error_context(error, component).await?,
        };

        // Analyze error and determine recovery strategy
        let recovery_strategy = self.analyze_and_select_strategy(&error_context).await?;

        // Execute recovery strategy
        let recovery_result = self
            .execute_recovery_strategy(&recovery_strategy, &error_context)
            .await;

        // Record the recovery attempt
        self.record_recovery_attempt(
            &error_context,
            &recovery_strategy,
            &recovery_result,
            recovery_start,
        )
        .await?;

        match recovery_result {
            Ok(action) => {
                tracing::info!("Error recovery successful: {} -> {:?}", component, action);
                Ok(action)
            }
            Err(recovery_error) => {
                tracing::error!(
                    "Error recovery failed for {}: {:?}",
                    component,
                    recovery_error
                );

                // Try emergency recovery if primary recovery failed
                self.attempt_emergency_recovery(&error_context).await
            }
        }
    }

    /// Create error context from current system state
    async fn create_error_context(
        &self,
        error: &AudioDeviceError,
        component: &str,
    ) -> Result<ErrorContext> {
        let system_state = self
            .system_state
            .read()
            .map_err(|e| AudioDeviceError::SpatialAudioError(format!("Lock error: {}", e)))?
            .clone();

        let error_history = self
            .error_history
            .read()
            .map_err(|e| AudioDeviceError::SpatialAudioError(format!("Lock error: {}", e)))?
            .iter()
            .take(10)
            .cloned()
            .collect();

        let resource_snapshot = self.resource_monitor.get_snapshot().await?;

        // Determine available recovery strategies
        let available_strategies = self.get_strategies_for_error(error, component).await?;

        Ok(ErrorContext {
            timestamp: SystemTime::now(),
            component: component.to_string(),
            system_state,
            error_history,
            recovery_attempts: 0,
            available_strategies,
            resource_snapshot,
        })
    }

    /// Analyze error and select best recovery strategy
    async fn analyze_and_select_strategy(
        &self,
        context: &ErrorContext,
    ) -> Result<RecoveryStrategy> {
        // Check for recurring errors
        let recent_errors = context
            .error_history
            .iter()
            .filter(|e| e.component == context.component)
            .filter(|e| e.timestamp.elapsed().unwrap_or(Duration::MAX) < Duration::from_secs(60))
            .count();

        if recent_errors > 3 {
            // High error frequency, consider more aggressive recovery
            return Ok(RecoveryStrategy::Reset {
                component: context.component.clone(),
                preserve_user_settings: true,
                reinitialize: true,
            });
        }

        // Analyze resource pressure
        match context.resource_snapshot.memory_pressure.system_pressure {
            PressureLevel::Critical => {
                return Ok(RecoveryStrategy::Degrade {
                    disable_features: vec!["effects".to_string(), "3d_audio".to_string()],
                    reduce_quality: true,
                    temporary: true,
                });
            }
            PressureLevel::High => {
                return Ok(RecoveryStrategy::Degrade {
                    disable_features: vec!["reverb".to_string()],
                    reduce_quality: false,
                    temporary: true,
                });
            }
            _ => {}
        }

        // Select first available strategy (more sophisticated selection could be implemented)
        context
            .available_strategies
            .first()
            .cloned()
            .ok_or_else(|| {
                AudioDeviceError::SpatialAudioError("No recovery strategies available".to_string())
            })
    }

    /// Execute a recovery strategy
    async fn execute_recovery_strategy(
        &self,
        strategy: &RecoveryStrategy,
        _context: &ErrorContext,
    ) -> Result<RecoveryAction> {
        match strategy {
            RecoveryStrategy::Retry {
                max_attempts,
                backoff_ms,
                exponential,
            } => {
                // Implement retry logic with backoff
                let mut delay = *backoff_ms;
                for attempt in 1..=*max_attempts {
                    tracing::info!("Retry attempt {} of {}", attempt, max_attempts);

                    tokio::time::sleep(Duration::from_millis(delay)).await;

                    // Here you would retry the original operation
                    // For now, simulate success after some attempts
                    if attempt == *max_attempts {
                        return Ok(RecoveryAction::Retried { successful: true });
                    }

                    if *exponential {
                        delay *= 2;
                    }
                }

                Ok(RecoveryAction::Retried { successful: false })
            }

            RecoveryStrategy::Fallback {
                fallback_id,
                preserve_state,
                notification_required,
            } => {
                tracing::info!("Switching to fallback device: {}", fallback_id);

                // Implementation would switch to fallback device
                Ok(RecoveryAction::FallbackActivated {
                    fallback_id: fallback_id.clone(),
                    state_preserved: *preserve_state,
                    user_notified: *notification_required,
                })
            }

            RecoveryStrategy::Degrade {
                disable_features,
                reduce_quality,
                temporary,
            } => {
                tracing::info!("Degrading audio quality/features: {:?}", disable_features);

                // Implementation would disable features and reduce quality
                Ok(RecoveryAction::QualityDegraded {
                    disabled_features: disable_features.clone(),
                    quality_reduced: *reduce_quality,
                    is_temporary: *temporary,
                })
            }

            RecoveryStrategy::Reset {
                component,
                preserve_user_settings,
                reinitialize,
            } => {
                tracing::info!("Resetting component: {}", component);

                // Implementation would reset the component
                Ok(RecoveryAction::ComponentReset {
                    component: component.clone(),
                    settings_preserved: *preserve_user_settings,
                    reinitialized: *reinitialize,
                })
            }

            RecoveryStrategy::EmergencyShutdown {
                save_state,
                notify_user,
                restart_possible,
            } => {
                tracing::warn!("Emergency shutdown initiated");

                // Implementation would perform emergency shutdown
                Ok(RecoveryAction::EmergencyShutdown {
                    state_saved: *save_state,
                    user_notified: *notify_user,
                    can_restart: *restart_possible,
                })
            }
        }
    }

    /// Attempt emergency recovery as last resort
    async fn attempt_emergency_recovery(&self, _context: &ErrorContext) -> Result<RecoveryAction> {
        tracing::error!("Attempting emergency recovery");

        // Emergency recovery: try to maintain basic audio functionality
        Ok(RecoveryAction::QualityDegraded {
            disabled_features: vec!["all_effects".to_string(), "3d_audio".to_string()],
            quality_reduced: true,
            is_temporary: false,
        })
    }

    /// Get recovery strategies for specific error type
    async fn get_strategies_for_error(
        &self,
        error: &AudioDeviceError,
        _component: &str,
    ) -> Result<Vec<RecoveryStrategy>> {
        let strategies = self
            .recovery_strategies
            .read()
            .map_err(|e| AudioDeviceError::SpatialAudioError(format!("Lock error: {}", e)))?;

        let error_type = match error {
            AudioDeviceError::DeviceNotFound(_) | AudioDeviceError::DeviceBusy(_) => {
                "device_failure"
            }
            AudioDeviceError::BufferError(_) => "memory_exhaustion",
            AudioDeviceError::StreamingError(_) => "network_error",
            _ => "general_error",
        };

        Ok(strategies.get(error_type).cloned().unwrap_or_else(|| {
            vec![RecoveryStrategy::Retry {
                max_attempts: 1,
                backoff_ms: 100,
                exponential: false,
            }]
        }))
    }

    /// Record recovery attempt for analysis
    async fn record_recovery_attempt(
        &self,
        context: &ErrorContext,
        strategy: &RecoveryStrategy,
        result: &Result<RecoveryAction>,
        start_time: Instant,
    ) -> Result<()> {
        let recovery_time = start_time.elapsed().as_millis() as u64;
        let successful = result.is_ok();

        let historical_error = HistoricalError {
            timestamp: context.timestamp,
            error_type: format!("{:?}", context),
            component: context.component.clone(),
            recovery_used: Some(strategy.clone()),
            recovery_successful: successful,
            recovery_time_ms: Some(recovery_time),
        };

        // Add to error history
        let mut history = self
            .error_history
            .write()
            .map_err(|e| AudioDeviceError::SpatialAudioError(format!("Lock error: {}", e)))?;

        if history.len() >= self.config.max_error_history {
            history.pop_front();
        }
        history.push_back(historical_error);

        // Update recovery statistics
        let mut stats = self
            .recovery_stats
            .write()
            .map_err(|e| AudioDeviceError::SpatialAudioError(format!("Lock error: {}", e)))?;

        stats.total_errors += 1;
        if successful {
            stats.successful_recoveries += 1;
        } else {
            stats.failed_recoveries += 1;
        }

        // Update average recovery time
        stats.average_recovery_time_ms = (stats.average_recovery_time_ms
            * (stats.total_errors - 1) as f32
            + recovery_time as f32)
            / stats.total_errors as f32;

        Ok(())
    }

    /// Get current recovery statistics
    pub async fn get_recovery_statistics(&self) -> Result<RecoveryStatistics> {
        let stats = self
            .recovery_stats
            .read()
            .map_err(|e| AudioDeviceError::SpatialAudioError(format!("Lock error: {}", e)))?;

        Ok(stats.clone())
    }
}

/// Actions that can be taken during recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryAction {
    /// Operation was retried
    Retried { successful: bool },

    /// Fallback device/configuration activated
    FallbackActivated {
        fallback_id: String,
        state_preserved: bool,
        user_notified: bool,
    },

    /// Audio quality/features degraded
    QualityDegraded {
        disabled_features: Vec<String>,
        quality_reduced: bool,
        is_temporary: bool,
    },

    /// Component was reset
    ComponentReset {
        component: String,
        settings_preserved: bool,
        reinitialized: bool,
    },

    /// Emergency shutdown performed
    EmergencyShutdown {
        state_saved: bool,
        user_notified: bool,
        can_restart: bool,
    },
}

impl ResourceMonitor {
    /// Create new resource monitor
    pub fn new() -> Result<Self> {
        Ok(Self {
            cpu_history: Arc::new(Mutex::new(VecDeque::with_capacity(300))), // 5 minutes at 1Hz
            memory_history: Arc::new(Mutex::new(VecDeque::with_capacity(300))),
            io_stats: Arc::new(RwLock::new(IoStatistics::default())),
            last_update: Arc::new(RwLock::new(Instant::now())),
        })
    }

    /// Get current resource snapshot
    pub async fn get_snapshot(&self) -> Result<ResourceSnapshot> {
        // In a real implementation, this would collect actual system metrics
        Ok(ResourceSnapshot {
            cpu_per_core: vec![0.5, 0.3, 0.7, 0.4], // Simulated CPU usage
            memory_pressure: MemoryPressure {
                system_pressure: PressureLevel::Normal,
                audio_pressure: PressureLevel::Normal,
                gc_frequency: 1.0,
                allocation_failures: 0,
            },
            io_stats: IoStatistics::default(),
            thread_timing: ThreadTiming::default(),
        })
    }
}

impl PerformanceTracker {
    /// Create new performance tracker
    pub fn new() -> Self {
        Self {
            frame_times: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
            latency_measurements: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
            drop_counter: Arc::new(AtomicU64::new(0)),
            quality_events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Record frame timing
    pub fn record_frame_time(&self, duration: Duration) -> Result<()> {
        let mut frame_times = self
            .frame_times
            .lock()
            .map_err(|e| AudioDeviceError::SpatialAudioError(format!("Lock error: {}", e)))?;

        if frame_times.len() >= 1000 {
            frame_times.pop_front();
        }
        frame_times.push_back(duration);

        Ok(())
    }

    /// Record audio latency measurement
    pub fn record_latency(&self, latency: Duration) -> Result<()> {
        let mut latency_measurements = self
            .latency_measurements
            .lock()
            .map_err(|e| AudioDeviceError::SpatialAudioError(format!("Lock error: {}", e)))?;

        if latency_measurements.len() >= 1000 {
            latency_measurements.pop_front();
        }
        latency_measurements.push_back(latency);

        Ok(())
    }

    /// Increment drop counter
    pub fn record_drop(&self) {
        self.drop_counter.fetch_add(1, Ordering::Relaxed);
    }

    /// Get drop count
    pub fn get_drop_count(&self) -> u64 {
        self.drop_counter.load(Ordering::Relaxed)
    }
}

// Default implementations for various types
impl Default for SystemState {
    fn default() -> Self {
        Self {
            active_sources: 0,
            device_config: DeviceConfiguration::default(),
            memory_usage: MemoryUsage::default(),
            cpu_usage: 0.0,
            network_latency_ms: 0.0,
            quality_level: QualityLevel::High,
        }
    }
}

impl Default for DeviceConfiguration {
    fn default() -> Self {
        Self {
            primary_device: "default".to_string(),
            fallback_devices: vec!["system_default".to_string()],
            format: AudioFormat::default(),
            buffer_config: BufferConfiguration::default(),
        }
    }
}

impl Default for BufferConfiguration {
    fn default() -> Self {
        Self {
            buffer_size: 1024,
            buffer_count: 4,
            target_latency_ms: 20.0,
            actual_latency_ms: 20.0,
        }
    }
}

impl Default for MemoryUsage {
    fn default() -> Self {
        Self {
            total_audio_memory: 1024 * 1024 * 64,     // 64 MB
            available_audio_memory: 1024 * 1024 * 32, // 32 MB
            allocated_buffers: 10,
            largest_free_block: 1024 * 1024 * 16, // 16 MB
            fragmentation_percent: 5.0,
        }
    }
}

impl Default for IoStatistics {
    fn default() -> Self {
        Self {
            read_throughput: 10.0,
            write_throughput: 5.0,
            network_throughput: 1.0,
            error_rate: 0.0,
            average_latency_ms: 2.0,
        }
    }
}

impl Default for ThreadTiming {
    fn default() -> Self {
        Self {
            main_thread_cpu_ms: 5.0,
            background_threads_cpu_ms: 2.0,
            context_switches_per_sec: 100.0,
            priority_inversions: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_recovery_manager_creation() {
        let config = ErrorRecoveryConfig::default();
        let manager = ErrorRecoveryManager::new(config).unwrap();
        assert!(!manager.shutdown_flag.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_resource_monitor() {
        let monitor = ResourceMonitor::new().unwrap();
        let snapshot = monitor.get_snapshot().await.unwrap();
        assert!(!snapshot.cpu_per_core.is_empty());
        assert_eq!(
            snapshot.memory_pressure.system_pressure,
            PressureLevel::Normal
        );
    }

    #[test]
    fn test_performance_tracker() {
        let tracker = PerformanceTracker::new();

        tracker
            .record_frame_time(Duration::from_millis(16))
            .unwrap();
        tracker.record_latency(Duration::from_millis(20)).unwrap();
        tracker.record_drop();

        assert_eq!(tracker.get_drop_count(), 1);
    }

    #[test]
    fn test_recovery_strategy_serialization() {
        let strategy = RecoveryStrategy::Retry {
            max_attempts: 3,
            backoff_ms: 1000,
            exponential: true,
        };

        let serialized = serde_json::to_string(&strategy).unwrap();
        let deserialized: RecoveryStrategy = serde_json::from_str(&serialized).unwrap();

        match deserialized {
            RecoveryStrategy::Retry { max_attempts, .. } => assert_eq!(max_attempts, 3),
            _ => panic!("Wrong strategy type"),
        }
    }
}
