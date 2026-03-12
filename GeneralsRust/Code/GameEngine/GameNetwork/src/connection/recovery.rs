//! Error handling and recovery mechanisms for network connections
//!
//! This module provides comprehensive error handling, automatic recovery,
//! and graceful degradation mechanisms for network connections in multiplayer
//! games, ensuring stable gameplay even under adverse network conditions.

use crate::commands::{NetCommand, NetCommandType};
use crate::connection::state::{ConnectionStateMachine, DetailedConnectionState, TransitionReason};
use crate::connection::{Connection, ConnectionState};
use crate::error::{NetworkError, NetworkResult};
use crate::time::NetworkInstant;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::task::JoinHandle;
use tokio::time::{interval, timeout};
use log;
use uuid::Uuid;

/// Recovery strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryConfig {
    /// Maximum recovery attempts before giving up
    pub max_recovery_attempts: u32,
    /// Initial retry delay
    pub initial_retry_delay: Duration,
    /// Maximum retry delay (for exponential backoff)
    pub max_retry_delay: Duration,
    /// Retry delay multiplier for exponential backoff
    pub retry_multiplier: f64,
    /// Connection health check interval
    pub health_check_interval: Duration,
    /// Timeout for recovery operations
    pub recovery_timeout: Duration,
    /// Enable automatic recovery
    pub enable_auto_recovery: bool,
    /// Enable connection redundancy
    pub enable_redundancy: bool,
    /// Degraded mode threshold (packet loss %)
    pub degraded_mode_threshold: f64,
    /// Circuit breaker failure threshold
    pub circuit_breaker_threshold: u32,
    /// Circuit breaker reset timeout
    pub circuit_breaker_reset: Duration,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_recovery_attempts: 5,
            initial_retry_delay: Duration::from_millis(500),
            max_retry_delay: Duration::from_secs(30),
            retry_multiplier: 2.0,
            health_check_interval: Duration::from_secs(10),
            recovery_timeout: Duration::from_secs(60),
            enable_auto_recovery: true,
            enable_redundancy: false,
            degraded_mode_threshold: 5.0,
            circuit_breaker_threshold: 10,
            circuit_breaker_reset: Duration::from_secs(60),
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// Minor, non-critical error
    Low = 1,
    /// Moderate error requiring attention
    Medium = 2,
    /// Serious error affecting functionality
    High = 3,
    /// Critical error requiring immediate action
    Critical = 4,
}

/// Network error types for recovery
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RecoverableError {
    /// Temporary connection timeout
    ConnectionTimeout,
    /// High packet loss
    PacketLoss { loss_rate: f64 },
    /// High network latency
    HighLatency { latency_ms: u32 },
    /// Connection reset by peer
    ConnectionReset,
    /// Network unreachable
    NetworkUnreachable,
    /// Protocol error
    ProtocolError { details: String },
    /// Authentication failure
    AuthenticationError,
    /// Resource exhaustion
    ResourceExhaustion { resource: String },
    /// Serialization/deserialization error
    SerializationError { details: String },
    /// Transport layer error
    TransportError { transport_error: String },
    /// Congestion control triggered
    Congestion,
}

impl RecoverableError {
    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::ConnectionTimeout => ErrorSeverity::Medium,
            Self::PacketLoss { loss_rate } => {
                if *loss_rate > 20.0 { ErrorSeverity::High }
                else if *loss_rate > 10.0 { ErrorSeverity::Medium }
                else { ErrorSeverity::Low }
            }
            Self::HighLatency { latency_ms } => {
                if *latency_ms > 1000 { ErrorSeverity::High }
                else if *latency_ms > 500 { ErrorSeverity::Medium }
                else { ErrorSeverity::Low }
            }
            Self::ConnectionReset => ErrorSeverity::High,
            Self::NetworkUnreachable => ErrorSeverity::High,
            Self::ProtocolError { .. } => ErrorSeverity::High,
            Self::AuthenticationError => ErrorSeverity::Critical,
            Self::ResourceExhaustion { .. } => ErrorSeverity::High,
            Self::SerializationError { .. } => ErrorSeverity::Medium,
            Self::TransportError { .. } => ErrorSeverity::Medium,
            Self::Congestion => ErrorSeverity::Medium,
        }
    }

    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::AuthenticationError => false, // Usually not recoverable
            _ => true,
        }
    }

    /// Get recommended recovery strategy
    pub fn recovery_strategy(&self) -> RecoveryStrategy {
        match self {
            Self::ConnectionTimeout => RecoveryStrategy::Reconnect,
            Self::PacketLoss { .. } => RecoveryStrategy::DegradedMode,
            Self::HighLatency { .. } => RecoveryStrategy::DegradedMode,
            Self::ConnectionReset => RecoveryStrategy::Reconnect,
            Self::NetworkUnreachable => RecoveryStrategy::Reconnect,
            Self::ProtocolError { .. } => RecoveryStrategy::Reset,
            Self::AuthenticationError => RecoveryStrategy::Fail,
            Self::ResourceExhaustion { .. } => RecoveryStrategy::BackOff,
            Self::SerializationError { .. } => RecoveryStrategy::Reset,
            Self::TransportError { .. } => RecoveryStrategy::Reconnect,
            Self::Congestion => RecoveryStrategy::BackOff,
        }
    }
}

/// Recovery strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    /// Retry the operation
    Retry,
    /// Reconnect the connection
    Reconnect,
    /// Reset the connection state
    Reset,
    /// Enable degraded mode operation
    DegradedMode,
    /// Back off and reduce load
    BackOff,
    /// Failover to backup connection
    Failover,
    /// Give up and fail
    Fail,
}

/// Recovery attempt record
#[derive(Debug, Clone)]
pub struct RecoveryAttempt {
    /// Unique attempt identifier
    pub attempt_id: Uuid,
    /// Player ID being recovered
    pub player_id: u8,
    /// Error being recovered from
    pub error: RecoverableError,
    /// Recovery strategy being used
    pub strategy: RecoveryStrategy,
    /// Attempt number (1-based)
    pub attempt_number: u32,
    /// When attempt started
    pub started_at: NetworkInstant,
    /// Current status
    pub status: RecoveryStatus,
    /// Additional context
    pub context: HashMap<String, String>,
}

/// Recovery attempt status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryStatus {
    /// Recovery in progress
    InProgress,
    /// Recovery succeeded
    Success,
    /// Recovery failed
    Failed,
    /// Recovery timed out
    TimedOut,
    /// Recovery cancelled
    Cancelled,
}

/// Connection health metrics
#[derive(Debug, Clone, Default)]
pub struct ConnectionHealth {
    /// Connection uptime
    pub uptime: Duration,
    /// Current round-trip time
    pub rtt: Duration,
    /// Packet loss rate (0.0 to 1.0)
    pub packet_loss_rate: f64,
    /// Messages sent successfully
    pub messages_sent: u64,
    /// Messages failed
    pub messages_failed: u64,
    /// Reliability score (0.0 to 1.0)
    pub reliability_score: f64,
    /// Last successful communication
    pub last_successful_ping: Option<NetworkInstant>,
    /// Error count in last window
    pub recent_error_count: u32,
    /// Is connection degraded
    pub is_degraded: bool,
}

impl ConnectionHealth {
    /// Calculate overall health score (0.0 to 1.0)
    pub fn health_score(&self) -> f64 {
        let mut score = 1.0;

        // Factor in packet loss
        score -= self.packet_loss_rate * 0.5;

        // Factor in RTT (penalize high latency)
        let rtt_penalty = (self.rtt.as_millis() as f64 / 1000.0).min(1.0) * 0.3;
        score -= rtt_penalty;

        // Factor in reliability
        score *= self.reliability_score;

        // Factor in recent errors
        let error_penalty = (self.recent_error_count as f64 / 10.0).min(0.5);
        score -= error_penalty;

        score.max(0.0).min(1.0)
    }

    /// Check if connection needs recovery
    pub fn needs_recovery(&self, config: &RecoveryConfig) -> bool {
        self.packet_loss_rate * 100.0 > config.degraded_mode_threshold ||
        self.rtt > Duration::from_millis(1000) ||
        self.recent_error_count > config.circuit_breaker_threshold ||
        self.reliability_score < 0.5
    }
}

/// Circuit breaker for preventing cascading failures
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// Current state
    pub state: CircuitState,
    /// Failure count
    pub failure_count: u32,
    /// Last failure time
    pub last_failure_time: Option<NetworkInstant>,
    /// Configuration
    pub config: RecoveryConfig,
}

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed - operations allowed
    Closed,
    /// Circuit is open - operations blocked
    Open,
    /// Circuit is half-open - testing if service recovered
    HalfOpen,
}

impl CircuitBreaker {
    /// Create new circuit breaker
    pub fn new(config: RecoveryConfig) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            last_failure_time: None,
            config,
        }
    }

    /// Check if operation is allowed
    pub fn is_operation_allowed(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if enough time has passed to try half-open
                if let Some(last_failure) = self.last_failure_time {
                    if last_failure.elapsed() > self.config.circuit_breaker_reset {
                        self.state = CircuitState::HalfOpen;
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record operation success
    pub fn record_success(&mut self) {
        match self.state {
            CircuitState::HalfOpen => {
                self.state = CircuitState::Closed;
                self.failure_count = 0;
                self.last_failure_time = None;
            }
            CircuitState::Closed => {
                // Reset failure count on success
                if self.failure_count > 0 {
                    self.failure_count = 0;
                }
            }
            CircuitState::Open => {
                // Should not happen
            }
        }
    }

    /// Record operation failure
    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(NetworkInstant::now());

        match self.state {
            CircuitState::Closed => {
                if self.failure_count >= self.config.circuit_breaker_threshold {
                    self.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open;
            }
            CircuitState::Open => {
                // Already open
            }
        }
    }
}

/// Connection recovery coordinator
pub struct ConnectionRecoveryCoordinator {
    /// Configuration
    config: RecoveryConfig,

    /// Active recovery attempts
    active_recoveries: Arc<RwLock<HashMap<u8, RecoveryAttempt>>>,

    /// Connection health tracking
    connection_health: Arc<RwLock<HashMap<u8, ConnectionHealth>>>,

    /// Circuit breakers per connection
    circuit_breakers: Arc<RwLock<HashMap<u8, CircuitBreaker>>>,

    /// Recovery event notifications
    recovery_events_tx: broadcast::Sender<RecoveryEvent>,

    /// Background task handles
    background_tasks: Arc<RwLock<Vec<JoinHandle<()>>>>,

    /// Shutdown coordination
    shutdown_tx: broadcast::Sender<()>,
}

/// Recovery event notifications
#[derive(Debug, Clone)]
pub enum RecoveryEvent {
    /// Recovery attempt started
    RecoveryStarted {
        player_id: u8,
        error: RecoverableError,
        strategy: RecoveryStrategy,
    },
    /// Recovery attempt completed
    RecoveryCompleted {
        player_id: u8,
        status: RecoveryStatus,
        duration: Duration,
    },
    /// Connection health changed
    HealthChanged {
        player_id: u8,
        old_score: f64,
        new_score: f64,
    },
    /// Degraded mode activated
    DegradedModeActivated {
        player_id: u8,
        reason: String,
    },
    /// Circuit breaker state changed
    CircuitBreakerChanged {
        player_id: u8,
        old_state: CircuitState,
        new_state: CircuitState,
    },
}

impl ConnectionRecoveryCoordinator {
    /// Create new recovery coordinator
    pub fn new() -> Self {
        Self::with_config(RecoveryConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: RecoveryConfig) -> Self {
        let (recovery_events_tx, _) = broadcast::channel(1000);
        let (shutdown_tx, _) = broadcast::channel(1);

        Self {
            config,
            active_recoveries: Arc::new(RwLock::new(HashMap::new())),
            connection_health: Arc::new(RwLock::new(HashMap::new())),
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
            recovery_events_tx,
            background_tasks: Arc::new(RwLock::new(Vec::new())),
            shutdown_tx,
        }
    }

    /// Start the recovery coordinator
    pub async fn start(&mut self) -> NetworkResult<()> {
        info!("Starting connection recovery coordinator");

        // Start health monitoring task
        self.start_health_monitoring().await?;

        // Start recovery processing task
        self.start_recovery_processing().await?;

        info!("Connection recovery coordinator started");
        Ok(())
    }

    /// Add connection for monitoring
    pub async fn add_connection(&self, player_id: u8) {
        {
            let mut health_map = self.connection_health.write().await;
            health_map.insert(player_id, ConnectionHealth::default());
        }

        {
            let mut circuit_breaker_map = self.circuit_breakers.write().await;
            circuit_breaker_map.insert(player_id, CircuitBreaker::new(self.config.clone()));
        }

        debug!("Added connection {} to recovery coordinator", player_id);
    }

    /// Remove connection from monitoring
    pub async fn remove_connection(&self, player_id: u8) {
        // Cancel any active recovery
        {
            let mut recoveries = self.active_recoveries.write().await;
            if let Some(mut recovery) = recoveries.remove(&player_id) {
                recovery.status = RecoveryStatus::Cancelled;
            }
        }

        {
            let mut health_map = self.connection_health.write().await;
            health_map.remove(&player_id);
        }

        {
            let mut circuit_breakers = self.circuit_breakers.write().await;
            circuit_breakers.remove(&player_id);
        }

        debug!("Removed connection {} from recovery coordinator", player_id);
    }

    /// Report error for potential recovery
    pub async fn report_error(
        &self,
        player_id: u8,
        error: RecoverableError,
    ) -> NetworkResult<()> {
        debug!("Reported error for player {}: {:?}", player_id, error);

        // Update circuit breaker
        {
            let mut circuit_breakers = self.circuit_breakers.write().await;
            if let Some(breaker) = circuit_breakers.get_mut(&player_id) {
                breaker.record_failure();
            }
        }

        // Update connection health
        {
            let mut health_map = self.connection_health.write().await;
            if let Some(health) = health_map.get_mut(&player_id) {
                health.recent_error_count += 1;
                health.messages_failed += 1;

                // Update specific metrics based on error type
                match &error {
                    RecoverableError::PacketLoss { loss_rate } => {
                        health.packet_loss_rate = *loss_rate / 100.0;
                    }
                    RecoverableError::HighLatency { latency_ms } => {
                        health.rtt = Duration::from_millis(*latency_ms as u64);
                    }
                    _ => {}
                }

                // Recalculate reliability score
                let total_messages = health.messages_sent + health.messages_failed;
                if total_messages > 0 {
                    health.reliability_score = health.messages_sent as f64 / total_messages as f64;
                }
            }
        }

        // Check if recovery is needed and not already active
        if self.config.enable_auto_recovery && !self.is_recovery_active(player_id).await {
            if error.is_recoverable() {
                self.start_recovery(player_id, error).await?;
            }
        }

        Ok(())
    }

    /// Report successful operation
    pub async fn report_success(&self, player_id: u8) {
        // Update circuit breaker
        {
            let mut circuit_breakers = self.circuit_breakers.write().await;
            if let Some(breaker) = circuit_breakers.get_mut(&player_id) {
                breaker.record_success();
            }
        }

        // Update connection health
        {
            let mut health_map = self.connection_health.write().await;
            if let Some(health) = health_map.get_mut(&player_id) {
                health.messages_sent += 1;
                health.last_successful_ping = Some(NetworkInstant::now());

                // Decay recent error count
                if health.recent_error_count > 0 {
                    health.recent_error_count = (health.recent_error_count * 9) / 10;
                }

                // Recalculate reliability score
                let total_messages = health.messages_sent + health.messages_failed;
                if total_messages > 0 {
                    health.reliability_score = health.messages_sent as f64 / total_messages as f64;
                }
            }
        }
    }

    /// Start recovery for a connection
    async fn start_recovery(
        &self,
        player_id: u8,
        error: RecoverableError,
    ) -> NetworkResult<()> {
        // Check if recovery is already active
        if self.is_recovery_active(player_id).await {
            return Ok(());
        }

        let strategy = error.recovery_strategy();
        let attempt_id = Uuid::new_v4();

        let recovery = RecoveryAttempt {
            attempt_id,
            player_id,
            error: error.clone(),
            strategy,
            attempt_number: 1,
            started_at: NetworkInstant::now(),
            status: RecoveryStatus::InProgress,
            context: HashMap::new(),
        };

        // Store active recovery
        {
            let mut recoveries = self.active_recoveries.write().await;
            recoveries.insert(player_id, recovery);
        }

        // Send recovery started event
        let _ = self.recovery_events_tx.send(RecoveryEvent::RecoveryStarted {
            player_id,
            error,
            strategy,
        });

        info!("Started recovery for player {} with strategy {:?}", player_id, strategy);
        Ok(())
    }

    /// Check if recovery is active for connection
    async fn is_recovery_active(&self, player_id: u8) -> bool {
        let recoveries = self.active_recoveries.read().await;
        recoveries.contains_key(&player_id)
    }

    /// Get connection health
    pub async fn get_connection_health(&self, player_id: u8) -> Option<ConnectionHealth> {
        let health_map = self.connection_health.read().await;
        health_map.get(&player_id).cloned()
    }

    /// Check if operation is allowed (circuit breaker)
    pub async fn is_operation_allowed(&self, player_id: u8) -> bool {
        let mut circuit_breakers = self.circuit_breakers.write().await;
        if let Some(breaker) = circuit_breakers.get_mut(&player_id) {
            breaker.is_operation_allowed()
        } else {
            true
        }
    }

    /// Subscribe to recovery events
    pub fn subscribe_events(&self) -> broadcast::Receiver<RecoveryEvent> {
        self.recovery_events_tx.subscribe()
    }

    /// Start health monitoring background task
    async fn start_health_monitoring(&mut self) -> NetworkResult<()> {
        let connection_health = self.connection_health.clone();
        let recovery_events_tx = self.recovery_events_tx.clone();
        let config = self.config.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut interval = interval(config.health_check_interval);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let mut health_changes = Vec::new();

                        {
                            let mut health_map = connection_health.write().await;
                            
                            for (&player_id, health) in health_map.iter_mut() {
                                let old_score = health.health_score();
                                
                                // Update health metrics
                                health.uptime = health.uptime + config.health_check_interval;
                                
                                // Check for degraded mode
                                if health.needs_recovery(&config) && !health.is_degraded {
                                    health.is_degraded = true;
                                    
                                    let _ = recovery_events_tx.send(RecoveryEvent::DegradedModeActivated {
                                        player_id,
                                        reason: format!("Health score: {:.2}", old_score),
                                    });
                                }
                                
                                let new_score = health.health_score();
                                
                                // Report significant health changes
                                if (old_score - new_score).abs() > 0.1 {
                                    health_changes.push((player_id, old_score, new_score));
                                }
                            }
                        }

                        // Send health change events
                        for (player_id, old_score, new_score) in health_changes {
                            let _ = recovery_events_tx.send(RecoveryEvent::HealthChanged {
                                player_id,
                                old_score,
                                new_score,
                            });
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Health monitoring task shutting down");
                        break;
                    }
                }
            }
        });

        {
            let mut tasks = self.background_tasks.write().await;
            tasks.push(handle);
        }

        Ok(())
    }

    /// Start recovery processing background task
    async fn start_recovery_processing(&mut self) -> NetworkResult<()> {
        let active_recoveries = self.active_recoveries.clone();
        let recovery_events_tx = self.recovery_events_tx.clone();
        let config = self.config.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(1));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let mut completed_recoveries = Vec::new();

                        {
                            let mut recoveries = active_recoveries.write().await;
                            
                            for (&player_id, recovery) in recoveries.iter_mut() {
                                // Check for timeout
                                if recovery.started_at.elapsed() > config.recovery_timeout {
                                    recovery.status = RecoveryStatus::TimedOut;
                                    completed_recoveries.push(player_id);
                                    continue;
                                }

                                // Process recovery based on strategy
                                // In real implementation, this would interact with connections
                                match recovery.strategy {
                                    RecoveryStrategy::Retry => {
                                        // Simulate retry logic
                                        if recovery.attempt_number < config.max_recovery_attempts {
                                            // Would retry the operation here
                                            trace!("Retrying operation for player {}", player_id);
                                        } else {
                                            recovery.status = RecoveryStatus::Failed;
                                            completed_recoveries.push(player_id);
                                        }
                                    }
                                    RecoveryStrategy::Reconnect => {
                                        // Would trigger reconnection logic
                                        trace!("Reconnecting player {}", player_id);
                                        recovery.status = RecoveryStatus::Success;
                                        completed_recoveries.push(player_id);
                                    }
                                    RecoveryStrategy::Reset => {
                                        // Would reset connection state
                                        trace!("Resetting connection for player {}", player_id);
                                        recovery.status = RecoveryStatus::Success;
                                        completed_recoveries.push(player_id);
                                    }
                                    RecoveryStrategy::DegradedMode => {
                                        // Would enable degraded mode
                                        trace!("Enabling degraded mode for player {}", player_id);
                                        recovery.status = RecoveryStatus::Success;
                                        completed_recoveries.push(player_id);
                                    }
                                    RecoveryStrategy::BackOff => {
                                        // Would implement backoff
                                        trace!("Backing off for player {}", player_id);
                                        recovery.status = RecoveryStatus::Success;
                                        completed_recoveries.push(player_id);
                                    }
                                    RecoveryStrategy::Failover => {
                                        // Would trigger failover
                                        trace!("Failing over for player {}", player_id);
                                        recovery.status = RecoveryStatus::Success;
                                        completed_recoveries.push(player_id);
                                    }
                                    RecoveryStrategy::Fail => {
                                        // Give up
                                        recovery.status = RecoveryStatus::Failed;
                                        completed_recoveries.push(player_id);
                                    }
                                }
                            }

                            // Remove completed recoveries and send events
                            for player_id in &completed_recoveries {
                                if let Some(recovery) = recoveries.remove(player_id) {
                                    let _ = recovery_events_tx.send(RecoveryEvent::RecoveryCompleted {
                                        player_id: *player_id,
                                        status: recovery.status,
                                        duration: recovery.started_at.elapsed(),
                                    });
                                }
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Recovery processing task shutting down");
                        break;
                    }
                }
            }
        });

        {
            let mut tasks = self.background_tasks.write().await;
            tasks.push(handle);
        }

        Ok(())
    }

    /// Shutdown the recovery coordinator
    pub async fn shutdown(&mut self) -> NetworkResult<()> {
        info!("Shutting down connection recovery coordinator");

        // Send shutdown signal
        let _ = self.shutdown_tx.send(());

        // Wait for background tasks
        {
            let mut tasks = self.background_tasks.write().await;
            for handle in tasks.drain(..) {
                handle.abort();
                let _ = handle.await;
            }
        }

        // Cancel all active recoveries
        {
            let mut recoveries = self.active_recoveries.write().await;
            for (_, mut recovery) in recoveries.drain() {
                recovery.status = RecoveryStatus::Cancelled;
            }
        }

        info!("Connection recovery coordinator shutdown complete");
        Ok(())
    }
}

impl Default for ConnectionRecoveryCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_severity() {
        let timeout_error = RecoverableError::ConnectionTimeout;
        let high_latency = RecoverableError::HighLatency { latency_ms: 1500 };
        let auth_error = RecoverableError::AuthenticationError;

        assert_eq!(timeout_error.severity(), ErrorSeverity::Medium);
        assert_eq!(high_latency.severity(), ErrorSeverity::High);
        assert_eq!(auth_error.severity(), ErrorSeverity::Critical);
        
        assert!(timeout_error.is_recoverable());
        assert!(!auth_error.is_recoverable());
    }

    #[test]
    fn test_recovery_strategy() {
        let timeout_error = RecoverableError::ConnectionTimeout;
        let packet_loss = RecoverableError::PacketLoss { loss_rate: 15.0 };
        
        assert_eq!(timeout_error.recovery_strategy(), RecoveryStrategy::Reconnect);
        assert_eq!(packet_loss.recovery_strategy(), RecoveryStrategy::DegradedMode);
    }

    #[test]
    fn test_connection_health() {
        let mut health = ConnectionHealth::default();
        health.packet_loss_rate = 0.1; // 10%
        health.rtt = Duration::from_millis(200);
        health.reliability_score = 0.9;
        health.recent_error_count = 2;

        let score = health.health_score();
        assert!(score > 0.0 && score <= 1.0);
        assert!(score < 1.0); // Should be reduced due to packet loss and errors
    }

    #[test]
    fn test_circuit_breaker() {
        let config = RecoveryConfig::default();
        let mut breaker = CircuitBreaker::new(config);

        assert_eq!(breaker.state, CircuitState::Closed);
        assert!(breaker.is_operation_allowed());

        // Simulate failures
        for _ in 0..10 {
            breaker.record_failure();
        }

        assert_eq!(breaker.state, CircuitState::Open);
        assert!(!breaker.is_operation_allowed());

        // Simulate success after half-open
        breaker.state = CircuitState::HalfOpen;
        breaker.record_success();
        assert_eq!(breaker.state, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_recovery_coordinator() {
        let mut coordinator = ConnectionRecoveryCoordinator::new();
        coordinator.start().await.unwrap();

        coordinator.add_connection(0).await;
        
        let error = RecoverableError::ConnectionTimeout;
        coordinator.report_error(0, error).await.unwrap();

        // Check that recovery was started
        let is_active = coordinator.is_recovery_active(0).await;
        assert!(is_active);

        coordinator.shutdown().await.unwrap();
    }
}
