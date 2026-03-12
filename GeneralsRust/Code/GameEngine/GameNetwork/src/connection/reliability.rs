//! Reliable message delivery over unreliable transports
//!
//! This module implements reliability features like acknowledgments,
//! retransmission, duplicate detection, and ordered delivery.

use crate::commands::sequence_validator::{SequenceValidationResult, SequenceValidator};
use crate::commands::{NetCommand, NetCommandType};
use crate::error::{NetworkError, NetworkResult};
use crate::network_metrics::packet_loss_metrics::{
    CongestionLevel, PacketLossMetrics, PacketLossStats,
};
use crate::time::NetworkInstant;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, trace, warn};
use uuid::Uuid;

use std::sync::atomic::{AtomicU8, Ordering};

/// Reliability configuration
#[derive(Debug, Clone)]
pub struct ReliabilityConfig {
    /// Enable acknowledgments
    pub enable_acks: bool,
    /// Enable retransmission
    pub enable_retransmission: bool,
    /// Enable duplicate detection
    pub enable_duplicate_detection: bool,
    /// Enable ordered delivery
    pub enable_ordered_delivery: bool,
    /// Maximum retransmission attempts
    pub max_retries: u32,
    /// Initial retransmission timeout
    pub initial_timeout: Duration,
    /// Maximum retransmission timeout
    pub max_timeout: Duration,
    /// Timeout backoff multiplier
    pub timeout_multiplier: f64,
    /// Acknowledgment timeout
    pub ack_timeout: Duration,
    /// Maximum pending messages
    pub max_pending: usize,
    /// Duplicate detection window size
    pub duplicate_window_size: usize,
}

impl Default for ReliabilityConfig {
    fn default() -> Self {
        Self {
            enable_acks: true,
            enable_retransmission: true,
            enable_duplicate_detection: true,
            enable_ordered_delivery: true,
            max_retries: 5,
            initial_timeout: Duration::from_millis(500),
            max_timeout: Duration::from_secs(10),
            timeout_multiplier: 2.0,
            ack_timeout: Duration::from_millis(100),
            max_pending: 1000,
            duplicate_window_size: 256,
        }
    }
}

/// Reliability statistics
#[derive(Debug, Clone, Default)]
pub struct ReliabilityStats {
    /// Messages sent requiring acknowledgment
    pub messages_sent_reliable: u64,
    /// Messages successfully acknowledged
    pub messages_acknowledged: u64,
    /// Messages retransmitted
    pub messages_retransmitted: u64,
    /// Messages failed after max retries
    pub messages_failed: u64,
    /// Duplicate messages detected
    pub duplicates_detected: u64,
    /// Out-of-order messages received
    pub out_of_order_received: u64,
    /// Average round-trip time (milliseconds)
    pub average_rtt_ms: f64,
    /// Current pending messages
    pub pending_messages: usize,
    /// Sequence gaps detected
    pub sequence_gaps_detected: u64,
    /// Sequence wraparounds detected
    pub sequence_wraps_detected: u32,
    /// Sequence duplicates detected
    pub sequence_duplicates: u64,
}

/// Pending message awaiting acknowledgment
#[derive(Debug, Clone)]
struct PendingMessage {
    /// Original command
    command: NetCommand,
    /// Number of transmission attempts
    attempts: u32,
    /// Time of last send
    last_sent: NetworkInstant,
    /// Next retry time
    next_retry: NetworkInstant,
    /// Current timeout duration
    timeout: Duration,
}

impl PendingMessage {
    fn new(command: NetCommand, timeout: Duration) -> Self {
        let now = NetworkInstant::now();
        Self {
            command,
            attempts: 1,
            last_sent: now,
            next_retry: now + timeout,
            timeout,
        }
    }

    fn should_retry(&self, now: NetworkInstant, max_retries: u32) -> bool {
        self.attempts < max_retries && now >= self.next_retry
    }

    fn retry(&mut self, timeout_multiplier: f64, max_timeout: Duration) {
        let now = NetworkInstant::now();
        self.attempts += 1;
        self.last_sent = now;

        // Exponential backoff
        self.timeout =
            Duration::from_millis((self.timeout.as_millis() as f64 * timeout_multiplier) as u64)
                .min(max_timeout);

        self.next_retry = now + self.timeout;
    }

    fn rtt(&self, ack_time: NetworkInstant) -> Duration {
        ack_time.duration_since(self.last_sent)
    }
}

/// Message ordering buffer
#[derive(Debug)]
struct OrderingBuffer {
    /// Next expected sequence number
    next_expected: u64,
    /// Buffered out-of-order messages
    buffer: HashMap<u64, NetCommand>,
    /// Maximum buffer size
    max_size: usize,
    /// Sequence validator for wraparound detection
    sequence_validator: SequenceValidator,
}

impl OrderingBuffer {
    fn new(max_size: usize) -> Self {
        Self {
            next_expected: 0,
            buffer: HashMap::new(),
            max_size,
            sequence_validator: SequenceValidator::new(),
        }
    }

    /// Add message to ordering buffer with sequence validation
    ///
    /// Returns a tuple of (ready_messages, validation_result)
    fn add_message(
        &mut self,
        sequence: u64,
        command: NetCommand,
    ) -> (Vec<NetCommand>, SequenceValidationResult) {
        let mut ready_messages = Vec::new();

        // Validate sequence number (using u16 portion)
        let seq_u16 = command.sequence;
        let validation_result = self.sequence_validator.validate_and_advance(seq_u16);

        // Log validation results
        match &validation_result {
            SequenceValidationResult::Wraparound { wrap_count } => {
                debug!(
                    "Sequence wraparound detected at {} (wrap count: {})",
                    seq_u16, wrap_count
                );
            }
            SequenceValidationResult::Gap {
                expected,
                got,
                gap_size,
            } => {
                if *gap_size > 5 {
                    warn!(
                        "Large sequence gap detected: expected {}, got {} (gap: {})",
                        expected, got, gap_size
                    );
                } else {
                    debug!(
                        "Sequence gap detected: expected {}, got {} (gap: {})",
                        expected, got, gap_size
                    );
                }
            }
            SequenceValidationResult::OutOfOrder { expected, got } => {
                warn!(
                    "Out-of-order sequence detected: expected {}, got {}",
                    expected, got
                );
            }
            SequenceValidationResult::Duplicate => {
                debug!("Duplicate sequence detected: {}", seq_u16);
            }
            SequenceValidationResult::Valid => {
                trace!("Valid sequence: {}", seq_u16);
            }
        }

        if sequence == self.next_expected {
            // This is the next expected message
            ready_messages.push(command);
            self.next_expected += 1;

            // Check if any buffered messages are now ready
            while let Some(buffered_command) = self.buffer.remove(&self.next_expected) {
                ready_messages.push(buffered_command);
                self.next_expected += 1;
            }
        } else if sequence > self.next_expected {
            // Future message - buffer it
            if self.buffer.len() < self.max_size {
                self.buffer.insert(sequence, command);
            } else {
                warn!(
                    "Ordering buffer full, dropping message with sequence {}",
                    sequence
                );
            }
        }
        // Ignore past messages (sequence < next_expected)

        (ready_messages, validation_result)
    }

    /// Reset the buffer
    fn reset(&mut self, new_next_expected: u64) {
        self.next_expected = new_next_expected;
        self.buffer.clear();
        self.sequence_validator.reset();
    }
}

/// Duplicate detection window
#[derive(Debug)]
struct DuplicateDetector {
    /// Recently seen message IDs
    seen_messages: VecDeque<Uuid>,
    /// Maximum window size
    max_size: usize,
}

impl DuplicateDetector {
    fn new(max_size: usize) -> Self {
        Self {
            seen_messages: VecDeque::new(),
            max_size,
        }
    }

    /// Check if message is a duplicate
    fn is_duplicate(&mut self, message_id: Uuid) -> bool {
        if self.seen_messages.contains(&message_id) {
            return true;
        }

        // Add to window
        self.seen_messages.push_back(message_id);

        // Maintain window size
        if self.seen_messages.len() > self.max_size {
            self.seen_messages.pop_front();
        }

        false
    }

    /// Reset detector
    fn reset(&mut self) {
        self.seen_messages.clear();
    }
}

/// Reliability layer for ensuring message delivery
pub struct ReliabilityLayer {
    /// Configuration
    config: ReliabilityConfig,

    /// Pending messages awaiting acknowledgment
    pending_messages: RwLock<HashMap<Uuid, PendingMessage>>,

    /// Message ordering buffer
    ordering_buffer: RwLock<OrderingBuffer>,

    /// Duplicate detection
    duplicate_detector: RwLock<DuplicateDetector>,

    /// Statistics
    stats: RwLock<ReliabilityStats>,

    /// Outgoing message queue for retransmission
    retry_queue: RwLock<VecDeque<NetCommand>>,

    /// Notification channels
    ack_notify: mpsc::Sender<Uuid>,
    retry_notify: mpsc::Sender<NetCommand>,

    /// Sequence number generator
    sequence_counter: std::sync::atomic::AtomicU64,

    /// Local player identifier used when emitting acknowledgment commands
    local_player_id: AtomicU8,

    /// Packet loss metrics and congestion detection
    packet_loss_metrics: RwLock<PacketLossMetrics>,
}

impl ReliabilityLayer {
    /// Create new reliability layer
    pub fn new() -> Self {
        Self::with_config(ReliabilityConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: ReliabilityConfig) -> Self {
        let (ack_notify, _) = mpsc::channel(1000);
        let (retry_notify, _) = mpsc::channel(1000);

        Self {
            ordering_buffer: RwLock::new(OrderingBuffer::new(config.max_pending)),
            duplicate_detector: RwLock::new(DuplicateDetector::new(config.duplicate_window_size)),
            config,
            pending_messages: RwLock::new(HashMap::new()),
            stats: RwLock::new(ReliabilityStats::default()),
            retry_queue: RwLock::new(VecDeque::new()),
            ack_notify,
            retry_notify,
            sequence_counter: std::sync::atomic::AtomicU64::new(1),
            local_player_id: AtomicU8::new(0),
            packet_loss_metrics: RwLock::new(PacketLossMetrics::new()),
        }
    }

    /// Send a reliable message
    pub async fn send_reliable(&self, mut command: NetCommand) -> NetworkResult<()> {
        if !self.config.enable_acks {
            // No reliability needed
            return Ok(());
        }

        // Mark command as needing acknowledgment
        command.flags.needs_ack = true;

        // Assign sequence number if ordering is enabled
        if self.config.enable_ordered_delivery {
            let sequence = self
                .sequence_counter
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            command.sequence = sequence as u16;
        }

        // Add to pending messages
        let pending = PendingMessage::new(command.clone(), self.config.initial_timeout);
        {
            let mut pending_map = self.pending_messages.write().await;

            // Check if we're at capacity
            if pending_map.len() >= self.config.max_pending {
                return Err(NetworkError::resource_exhausted(
                    "too many pending reliable messages",
                ));
            }

            pending_map.insert(command.id, pending);
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.messages_sent_reliable += 1;
            let pending_map = self.pending_messages.read().await;
            stats.pending_messages = pending_map.len();
        }

        // Record packet sent in metrics
        {
            let mut metrics = self.packet_loss_metrics.write().await;
            metrics.record_packet_sent();
        }

        debug!(
            "Sent reliable message {} (sequence: {})",
            command.id, command.sequence
        );
        Ok(())
    }

    /// Drain pending control messages (ACKs, etc.) that should be transmitted immediately.
    pub async fn drain_control_queue(&self) -> Vec<NetCommand> {
        let mut queue = self.retry_queue.write().await;
        queue.drain(..).collect()
    }

    /// Mark a reliable command as transmitted at the current instant to reset its retry deadline.
    pub async fn mark_sent(&self, command_id: Uuid) {
        let mut pending_map = self.pending_messages.write().await;
        if let Some(pending) = pending_map.get_mut(&command_id) {
            let now = NetworkInstant::now();
            pending.last_sent = now;
            pending.next_retry = now + pending.timeout;
        }
    }

    /// Set the local player identifier attached to emitted acknowledgments.
    pub fn set_local_player_id(&self, player_id: u8) {
        self.local_player_id.store(player_id, Ordering::Relaxed);
    }

    /// Process incoming acknowledgment
    pub async fn process_acknowledgment(&self, ack_command: &NetCommand) -> NetworkResult<()> {
        if let crate::commands::CommandPayload::Ack(ack_data) = &ack_command.payload {
            let mut pending_map = self.pending_messages.write().await;

            if let Some(pending) = pending_map.remove(&ack_data.command_id) {
                let rtt = pending.rtt(NetworkInstant::now());

                // Update RTT statistics
                {
                    let mut stats = self.stats.write().await;
                    stats.messages_acknowledged += 1;
                    stats.pending_messages = pending_map.len();

                    // Simple moving average for RTT
                    if stats.average_rtt_ms == 0.0 {
                        stats.average_rtt_ms = rtt.as_millis() as f64;
                    } else {
                        stats.average_rtt_ms =
                            stats.average_rtt_ms * 0.8 + rtt.as_millis() as f64 * 0.2;
                    }
                }

                // Notify acknowledgment received
                let _ = self.ack_notify.send(ack_data.command_id).await;

                trace!("Processed ACK for {} (RTT: {:?})", ack_data.command_id, rtt);
            }
        }

        Ok(())
    }

    /// Process incoming message with reliability features
    pub async fn process_incoming(&self, command: NetCommand) -> NetworkResult<Vec<NetCommand>> {
        let mut result = Vec::new();

        // Record packet received in metrics
        {
            let mut metrics = self.packet_loss_metrics.write().await;
            metrics.record_packet_received();
        }

        // Check for duplicates
        if self.config.enable_duplicate_detection {
            let mut detector = self.duplicate_detector.write().await;
            if detector.is_duplicate(command.id) {
                let mut stats = self.stats.write().await;
                stats.duplicates_detected += 1;

                // Record duplicate in metrics
                {
                    let mut metrics = self.packet_loss_metrics.write().await;
                    metrics.record_duplicate();
                }

                trace!("Detected duplicate message: {}", command.id);
                return Ok(result); // Return empty - duplicate filtered out
            }
        }

        // Send acknowledgment if required
        if command.needs_acknowledgment() && self.config.enable_acks {
            let ack = NetCommand::ack(
                self.local_player_id.load(Ordering::Relaxed),
                NetCommandType::AckBoth,
                command.id,
            );

            {
                let mut queue = self.retry_queue.write().await;
                queue.push_back(ack.clone());
            }
            let _ = self.retry_notify.send(ack).await;

            trace!("Queued ACK for message {}", command.id);
        }

        // Handle ordering if enabled
        if self.config.enable_ordered_delivery && command.is_sequenced() {
            let mut ordering_buffer = self.ordering_buffer.write().await;
            let (ready_messages, validation_result) =
                ordering_buffer.add_message(command.sequence as u64, command);

            // Update statistics and metrics based on validation result
            {
                let mut stats = self.stats.write().await;
                let mut metrics = self.packet_loss_metrics.write().await;

                match validation_result {
                    SequenceValidationResult::Wraparound { wrap_count } => {
                        stats.sequence_wraps_detected = wrap_count;
                    }
                    SequenceValidationResult::Gap { gap_size, .. } => {
                        if gap_size > 5 {
                            stats.sequence_gaps_detected += 1;
                        }
                        // Record each lost packet in the gap
                        for _ in 0..gap_size {
                            metrics.record_packet_lost();
                        }
                    }
                    SequenceValidationResult::OutOfOrder { .. } => {
                        stats.out_of_order_received += 1;
                        metrics.record_out_of_order();
                    }
                    SequenceValidationResult::Duplicate => {
                        stats.sequence_duplicates += 1;
                        metrics.record_duplicate();
                    }
                    SequenceValidationResult::Valid => {
                        // Normal case, no special stats update needed
                    }
                }

                if ready_messages.len() != 1 {
                    // Some messages were buffered or multiple became ready
                    stats.out_of_order_received += ready_messages.len().saturating_sub(1) as u64;
                }
            }

            result.extend(ready_messages);
        } else {
            // No ordering - message is immediately available
            result.push(command);
        }

        Ok(result)
    }

    /// Process retransmission check
    pub async fn process_retransmission(&self) -> Vec<NetCommand> {
        if !self.config.enable_retransmission {
            return Vec::new();
        }

        let mut to_retry = Vec::new();
        let mut to_remove = Vec::new();
        let now = NetworkInstant::now();

        {
            let mut pending_map = self.pending_messages.write().await;

            for (id, pending) in pending_map.iter_mut() {
                if pending.should_retry(now, self.config.max_retries) {
                    // Retry this message
                    pending.retry(self.config.timeout_multiplier, self.config.max_timeout);
                    to_retry.push(pending.command.clone());

                    debug!("Retrying message {} (attempt {})", id, pending.attempts);
                } else if pending.attempts >= self.config.max_retries && now >= pending.next_retry {
                    // Give up on this message
                    to_remove.push(*id);
                    warn!(
                        "Giving up on message {} after {} attempts",
                        id, pending.attempts
                    );
                }
            }

            // Remove failed messages
            for id in &to_remove {
                pending_map.remove(id);
            }
        }

        // Update statistics and metrics
        if !to_retry.is_empty() || !to_remove.is_empty() {
            let mut stats = self.stats.write().await;
            stats.messages_retransmitted += to_retry.len() as u64;
            stats.messages_failed += to_remove.len() as u64;

            let pending_count = {
                let pending_map = self.pending_messages.read().await;
                pending_map.len()
            };
            stats.pending_messages = pending_count;

            // Record retransmissions in metrics
            let mut metrics = self.packet_loss_metrics.write().await;
            for _ in 0..to_retry.len() {
                metrics.record_retransmission();
            }
        }

        to_retry
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> ReliabilityStats {
        self.stats.read().await.clone()
    }

    /// Get network health report with congestion detection
    ///
    /// This provides a comprehensive view of network conditions including
    /// packet loss, congestion level, and recommendations for adaptive behavior.
    ///
    /// # Example
    /// ```no_run
    /// # use game_network::connection::reliability::ReliabilityLayer;
    /// # async fn example() {
    /// let layer = ReliabilityLayer::new();
    /// let health = layer.get_network_health().await;
    ///
    /// match health.congestion {
    ///     game_network::network_metrics::packet_loss_metrics::CongestionLevel::High |
    ///     game_network::network_metrics::packet_loss_metrics::CongestionLevel::Critical => {
    ///         println!("High congestion detected: {}", health.recommendation);
    ///         // Reduce frame rate or data transmission
    ///     }
    ///     _ => {
    ///         // Normal operation
    ///     }
    /// }
    /// # }
    /// ```
    pub async fn get_network_health(&self) -> NetworkHealthReport {
        let metrics = self.packet_loss_metrics.read().await;
        let stats = self.stats.read().await;

        let packet_loss_stats = metrics.get_stats();
        let congestion = packet_loss_stats.congestion_level;
        let recommendation = congestion.recommendation().to_string();

        NetworkHealthReport {
            congestion,
            packet_loss_rate: packet_loss_stats.current_loss_rate,
            retransmission_rate: packet_loss_stats.retransmission_rate,
            estimated_rtt_ms: stats.average_rtt_ms as u64,
            recommendation,
        }
    }

    /// Get packet loss metrics stats
    pub async fn get_packet_loss_stats(&self) -> PacketLossStats {
        let metrics = self.packet_loss_metrics.read().await;
        metrics.get_stats()
    }

    /// Reset reliability layer
    pub async fn reset(&self) {
        {
            let mut pending_map = self.pending_messages.write().await;
            pending_map.clear();
        }

        {
            let mut ordering_buffer = self.ordering_buffer.write().await;
            ordering_buffer.reset(0);
        }

        {
            let mut detector = self.duplicate_detector.write().await;
            detector.reset();
        }

        {
            let mut stats = self.stats.write().await;
            *stats = ReliabilityStats::default();
        }

        {
            let mut metrics = self.packet_loss_metrics.write().await;
            *metrics = PacketLossMetrics::new();
        }

        self.sequence_counter
            .store(1, std::sync::atomic::Ordering::Relaxed);

        debug!("Reset reliability layer");
    }

    /// Check if there are pending messages
    pub async fn has_pending_messages(&self) -> bool {
        let pending_map = self.pending_messages.read().await;
        !pending_map.is_empty()
    }

    /// Get number of pending messages
    pub async fn pending_count(&self) -> usize {
        let pending_map = self.pending_messages.read().await;
        pending_map.len()
    }

    /// Check health of reliability layer
    pub async fn health_check(&self) -> ReliabilityHealth {
        let stats = self.get_stats().await;
        let pending_count = self.pending_count().await;

        let success_rate = if stats.messages_sent_reliable > 0 {
            stats.messages_acknowledged as f64 / stats.messages_sent_reliable as f64
        } else {
            1.0
        };

        let retry_rate = if stats.messages_sent_reliable > 0 {
            stats.messages_retransmitted as f64 / stats.messages_sent_reliable as f64
        } else {
            0.0
        };

        let health = if success_rate < 0.5 || retry_rate > 2.0 {
            ReliabilityHealthStatus::Critical
        } else if success_rate < 0.8 || retry_rate > 1.0 {
            ReliabilityHealthStatus::Warning
        } else {
            ReliabilityHealthStatus::Healthy
        };

        ReliabilityHealth {
            status: health,
            success_rate_percent: success_rate * 100.0,
            retry_rate_percent: retry_rate * 100.0,
            average_rtt_ms: stats.average_rtt_ms,
            pending_messages: pending_count,
            duplicates_detected: stats.duplicates_detected,
        }
    }
}

impl Default for ReliabilityLayer {
    fn default() -> Self {
        Self::new()
    }
}

/// Reliability health status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReliabilityHealthStatus {
    Healthy,
    Warning,
    Critical,
}

/// Reliability health information
#[derive(Debug, Clone)]
pub struct ReliabilityHealth {
    pub status: ReliabilityHealthStatus,
    pub success_rate_percent: f64,
    pub retry_rate_percent: f64,
    pub average_rtt_ms: f64,
    pub pending_messages: usize,
    pub duplicates_detected: u64,
}

/// Network health report with congestion detection
#[derive(Debug, Clone)]
pub struct NetworkHealthReport {
    /// Current congestion level
    pub congestion: CongestionLevel,
    /// Packet loss rate (0.0-1.0)
    pub packet_loss_rate: f32,
    /// Retransmission rate (0.0-1.0+)
    pub retransmission_rate: f32,
    /// Estimated round-trip time in milliseconds
    pub estimated_rtt_ms: u64,
    /// Recommendation based on network conditions
    pub recommendation: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::NetCommand;

    #[tokio::test]
    async fn test_reliability_layer_creation() {
        let layer = ReliabilityLayer::new();
        let stats = layer.get_stats().await;

        assert_eq!(stats.messages_sent_reliable, 0);
        assert_eq!(stats.pending_messages, 0);
        assert!(!layer.has_pending_messages().await);
    }

    #[tokio::test]
    async fn test_send_reliable_message() {
        let layer = ReliabilityLayer::new();
        let command = NetCommand::keep_alive(0);

        layer.send_reliable(command.clone()).await.unwrap();

        assert!(layer.has_pending_messages().await);
        assert_eq!(layer.pending_count().await, 1);

        let stats = layer.get_stats().await;
        assert_eq!(stats.messages_sent_reliable, 1);
    }

    #[tokio::test]
    async fn test_acknowledgment_processing() {
        let layer = ReliabilityLayer::new();
        let command = NetCommand::keep_alive(0);
        let command_id = command.id;

        // Send reliable message
        layer.send_reliable(command).await.unwrap();
        assert_eq!(layer.pending_count().await, 1);

        // Process acknowledgment
        let ack = NetCommand::ack(1, NetCommandType::AckBoth, command_id);
        layer.process_acknowledgment(&ack).await.unwrap();

        // Should no longer be pending
        assert_eq!(layer.pending_count().await, 0);

        let stats = layer.get_stats().await;
        assert_eq!(stats.messages_acknowledged, 1);
    }

    #[tokio::test]
    async fn test_duplicate_detection() {
        let layer = ReliabilityLayer::new();
        let command = NetCommand::keep_alive(0);

        // Process message first time
        let result1 = layer.process_incoming(command.clone()).await.unwrap();
        assert_eq!(result1.len(), 1);

        // Process same message again (duplicate)
        let result2 = layer.process_incoming(command).await.unwrap();
        assert_eq!(result2.len(), 0); // Should be filtered out

        let stats = layer.get_stats().await;
        assert_eq!(stats.duplicates_detected, 1);
    }

    #[tokio::test]
    async fn test_message_ordering() {
        let layer = ReliabilityLayer::new();

        // Create messages with sequence numbers
        let mut cmd1 = NetCommand::keep_alive(0);
        cmd1.sequence = 0;
        cmd1.flags.sequenced = true;

        let mut cmd2 = NetCommand::keep_alive(0);
        cmd2.sequence = 1;
        cmd2.flags.sequenced = true;

        let mut cmd3 = NetCommand::keep_alive(0);
        cmd3.sequence = 2;
        cmd3.flags.sequenced = true;

        // Receive messages out of order
        let result3 = layer.process_incoming(cmd3).await.unwrap();
        assert_eq!(result3.len(), 0); // Buffered

        let result1 = layer.process_incoming(cmd1).await.unwrap();
        assert_eq!(result1.len(), 1); // Sequence 0 delivered

        let result2 = layer.process_incoming(cmd2).await.unwrap();
        assert_eq!(result2.len(), 2); // Sequence 1 and buffered 2 delivered
    }

    #[tokio::test]
    async fn test_retransmission() {
        let mut config = ReliabilityConfig::default();
        config.initial_timeout = Duration::from_millis(10); // Very short for testing

        let layer = ReliabilityLayer::with_config(config);
        let command = NetCommand::keep_alive(0);

        // Send reliable message
        layer.send_reliable(command).await.unwrap();

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Check for retransmission
        let retries = layer.process_retransmission().await;
        assert_eq!(retries.len(), 1);

        let stats = layer.get_stats().await;
        assert_eq!(stats.messages_retransmitted, 1);
    }

    #[test]
    fn test_ordering_buffer() {
        let mut buffer = OrderingBuffer::new(10);

        // Create test commands with sequence numbers
        let mut cmd0 = NetCommand::keep_alive(0);
        cmd0.sequence = 0;
        let mut cmd1 = NetCommand::keep_alive(1);
        cmd1.sequence = 1;
        let mut cmd2 = NetCommand::keep_alive(2);
        cmd2.sequence = 2;

        // Test basic ordering - add in sequence
        let (result, validation_result) = buffer.add_message(0, cmd0);
        assert_eq!(result.len(), 1); // Delivered immediately
        assert_eq!(validation_result, SequenceValidationResult::Valid);

        // Add message 2 (out of order - gap)
        let (result, validation_result) = buffer.add_message(2, cmd2);
        assert_eq!(result.len(), 0); // Buffered due to gap
        assert!(matches!(
            validation_result,
            SequenceValidationResult::Gap { .. }
        ));

        // Add message 1 (fills the gap)
        let (result, _) = buffer.add_message(1, cmd1);
        assert_eq!(result.len(), 2); // Both 1 and buffered 2 delivered
    }

    #[test]
    fn test_duplicate_detector() {
        let mut detector = DuplicateDetector::new(3);

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        // First occurrence
        assert!(!detector.is_duplicate(id1));
        assert!(!detector.is_duplicate(id2));

        // Duplicate
        assert!(detector.is_duplicate(id1));
        assert!(detector.is_duplicate(id2));
    }

    #[tokio::test]
    async fn test_health_check() {
        let layer = ReliabilityLayer::new();

        // Initially healthy
        let health = layer.health_check().await;
        assert_eq!(health.status, ReliabilityHealthStatus::Healthy);

        // Simulate some traffic
        let command = NetCommand::keep_alive(0);
        layer.send_reliable(command.clone()).await.unwrap();

        let ack = NetCommand::ack(1, NetCommandType::AckBoth, command.id);
        layer.process_acknowledgment(&ack).await.unwrap();

        let health = layer.health_check().await;
        assert_eq!(health.success_rate_percent, 100.0);
    }

    #[tokio::test]
    async fn test_control_queue_drain_produces_ack() {
        let layer = ReliabilityLayer::new();

        let mut command = NetCommand::keep_alive(0);
        command.flags.needs_ack = true;

        let processed = layer.process_incoming(command).await.unwrap();
        assert_eq!(processed.len(), 1);

        let control = layer.drain_control_queue().await;
        assert_eq!(control.len(), 1);
        assert!(matches!(control[0].command_type, NetCommandType::AckBoth));
    }

    #[tokio::test]
    async fn test_ack_uses_configured_local_player_id() {
        let layer = ReliabilityLayer::new();
        layer.set_local_player_id(7);

        let mut command = NetCommand::keep_alive(5);
        command.flags.needs_ack = true;

        let _ = layer.process_incoming(command).await.unwrap();
        let control = layer.drain_control_queue().await;
        assert_eq!(control.len(), 1);
        assert_eq!(control[0].player_id, 7);
    }

    // Integration tests for packet loss metrics

    #[tokio::test]
    async fn test_packet_loss_metrics_integration() {
        let layer = ReliabilityLayer::new();

        // Send some packets
        for _ in 0..10 {
            let command = NetCommand::keep_alive(0);
            layer.send_reliable(command).await.unwrap();
        }

        let metrics = layer.get_packet_loss_stats().await;
        assert_eq!(metrics.total_sent, 10);
    }

    #[tokio::test]
    async fn test_network_health_report() {
        let layer = ReliabilityLayer::new();

        // Get initial health
        let health = layer.get_network_health().await;
        assert_eq!(health.congestion, CongestionLevel::None);
        assert_eq!(health.packet_loss_rate, 0.0);
        assert_eq!(health.recommendation, "Network OK");
    }

    #[tokio::test]
    async fn test_packet_loss_tracking_with_gaps() {
        let layer = ReliabilityLayer::new();

        // Create messages with sequence gaps
        let mut cmd0 = NetCommand::keep_alive(0);
        cmd0.sequence = 0;
        cmd0.flags.sequenced = true;

        let mut cmd3 = NetCommand::keep_alive(0);
        cmd3.sequence = 3;
        cmd3.flags.sequenced = true;

        // Process in order with gap
        layer.process_incoming(cmd0).await.unwrap();
        layer.process_incoming(cmd3).await.unwrap();

        // Should have detected gap (packets 1 and 2 lost)
        let metrics = layer.get_packet_loss_stats().await;
        assert!(metrics.total_lost >= 2);
    }

    #[tokio::test]
    async fn test_duplicate_tracking_in_metrics() {
        let layer = ReliabilityLayer::new();

        let command = NetCommand::keep_alive(0);

        // Process twice (duplicate)
        layer.process_incoming(command.clone()).await.unwrap();
        layer.process_incoming(command).await.unwrap();

        let metrics = layer.get_packet_loss_stats().await;
        assert_eq!(metrics.total_received, 2);
        // The duplicate is counted separately in the metrics
    }

    #[tokio::test]
    async fn test_retransmission_tracking_in_metrics() {
        let mut config = ReliabilityConfig::default();
        config.initial_timeout = Duration::from_millis(10);

        let layer = ReliabilityLayer::with_config(config);
        let command = NetCommand::keep_alive(0);

        layer.send_reliable(command).await.unwrap();

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Process retransmission
        layer.process_retransmission().await;

        let metrics = layer.get_packet_loss_stats().await;
        assert_eq!(metrics.total_retransmitted, 1);
    }

    #[tokio::test]
    async fn test_congestion_detection_integration() {
        let layer = ReliabilityLayer::new();

        // Manually populate metrics to simulate high packet loss
        {
            let mut metrics = layer.packet_loss_metrics.write().await;
            for _ in 0..100 {
                metrics.record_packet_sent();
            }
            for _ in 0..70 {
                metrics.record_packet_received();
            }
            for _ in 0..30 {
                metrics.record_packet_lost();
            }
        }

        let health = layer.get_network_health().await;
        assert_eq!(health.congestion, CongestionLevel::Critical);
        assert!(health.packet_loss_rate > 0.25);
    }

    #[tokio::test]
    async fn test_metrics_reset_with_reliability_layer() {
        let layer = ReliabilityLayer::new();

        // Send and receive some packets
        for _ in 0..5 {
            let command = NetCommand::keep_alive(0);
            layer.send_reliable(command.clone()).await.unwrap();
            layer.process_incoming(command).await.unwrap();
        }

        let metrics_before = layer.get_packet_loss_stats().await;
        assert!(metrics_before.total_sent > 0);

        // Reset
        layer.reset().await;

        let metrics_after = layer.get_packet_loss_stats().await;
        assert_eq!(metrics_after.total_sent, 0);
        assert_eq!(metrics_after.total_received, 0);
    }
}
