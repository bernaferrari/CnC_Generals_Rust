//! Packet loss metrics and congestion detection
//!
//! This module provides comprehensive tracking of packet loss, retransmissions,
//! and network congestion. It enables adaptive behavior based on network conditions.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Congestion level based on packet loss rate
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CongestionLevel {
    /// Loss < 2%
    None,
    /// Loss 2-5%
    Low,
    /// Loss 5-10%
    Moderate,
    /// Loss 10-20%
    High,
    /// Loss > 20%
    Critical,
}

impl CongestionLevel {
    /// Get recommendation string for this congestion level
    pub fn recommendation(&self) -> &'static str {
        match self {
            CongestionLevel::None => "Network OK",
            CongestionLevel::Low => "Monitor network conditions",
            CongestionLevel::Moderate => "Consider reducing data rate",
            CongestionLevel::High => "Reduce frame rate and data transmission",
            CongestionLevel::Critical => "Critical congestion - minimize traffic",
        }
    }
}

/// Snapshot of packet loss statistics
#[derive(Debug, Clone)]
pub struct PacketLossStats {
    /// Total packets sent
    pub total_sent: u64,
    /// Total packets received
    pub total_received: u64,
    /// Total packets lost (inferred from gaps)
    pub total_lost: u64,
    /// Total packets retransmitted
    pub total_retransmitted: u64,
    /// Current loss rate (0.0-1.0)
    pub current_loss_rate: f32,
    /// Average loss rate over history (0.0-1.0)
    pub avg_loss_rate: f32,
    /// Retransmission rate (0.0-1.0)
    pub retransmission_rate: f32,
    /// Duration of current measurement window
    pub window_duration: Duration,
    /// Current congestion level
    pub congestion_level: CongestionLevel,
    /// Number of measurements in history
    pub measurements_count: usize,
}

/// Tracks packet loss metrics and detects congestion
#[derive(Debug)]
pub struct PacketLossMetrics {
    /// Total packets sent
    total_packets_sent: u64,
    /// Total packets received
    total_packets_received: u64,
    /// Retransmitted packets
    retransmitted_packets: u64,
    /// Packets lost (inferred from gaps)
    packets_lost: u64,
    /// Duplicate packets received
    duplicate_packets: u64,
    /// Out-of-order packets received
    out_of_order_packets: u64,
    /// Start time of current measurement window
    start_time: Instant,
    /// Measurement window duration
    #[allow(dead_code)]
    measurement_window: Duration,
    /// History of loss rates (last N measurements, 0.0 = 0%, 1.0 = 100%)
    loss_history: VecDeque<f32>,
    /// Maximum history size
    max_history_size: usize,
}

impl PacketLossMetrics {
    /// Create new packet loss metrics tracker
    ///
    /// # Example
    /// ```
    /// use game_network::network_metrics::packet_loss_metrics::PacketLossMetrics;
    ///
    /// let metrics = PacketLossMetrics::new();
    /// ```
    pub fn new() -> Self {
        Self::with_window(Duration::from_secs(10))
    }

    /// Create with custom measurement window
    ///
    /// # Arguments
    /// * `window` - Duration of each measurement window
    ///
    /// # Example
    /// ```
    /// use game_network::network_metrics::packet_loss_metrics::PacketLossMetrics;
    /// use std::time::Duration;
    ///
    /// let metrics = PacketLossMetrics::with_window(Duration::from_secs(5));
    /// ```
    pub fn with_window(window: Duration) -> Self {
        Self {
            total_packets_sent: 0,
            total_packets_received: 0,
            retransmitted_packets: 0,
            packets_lost: 0,
            duplicate_packets: 0,
            out_of_order_packets: 0,
            start_time: Instant::now(),
            measurement_window: window,
            loss_history: VecDeque::new(),
            max_history_size: 10,
        }
    }

    /// Record a packet sent
    pub fn record_packet_sent(&mut self) {
        self.total_packets_sent += 1;
    }

    /// Record a packet received
    pub fn record_packet_received(&mut self) {
        self.total_packets_received += 1;
    }

    /// Record a packet retransmission
    pub fn record_retransmission(&mut self) {
        self.retransmitted_packets += 1;
    }

    /// Record a packet lost (inferred from sequence gap)
    pub fn record_packet_lost(&mut self) {
        self.packets_lost += 1;
    }

    /// Record a duplicate packet
    pub fn record_duplicate(&mut self) {
        self.duplicate_packets += 1;
    }

    /// Record an out-of-order packet
    pub fn record_out_of_order(&mut self) {
        self.out_of_order_packets += 1;
    }

    /// Calculate current packet loss rate
    ///
    /// Returns a value between 0.0 (0% loss) and 1.0 (100% loss)
    pub fn calculate_loss_rate(&self) -> f32 {
        if self.total_packets_sent == 0 {
            return 0.0;
        }

        let lost = self.packets_lost as f32;
        let sent = self.total_packets_sent as f32;

        (lost / sent).min(1.0)
    }

    /// Calculate current retransmission rate
    ///
    /// Returns a value between 0.0 (0% retransmitted) and 1.0+ (can exceed 100%)
    pub fn calculate_retransmission_rate(&self) -> f32 {
        if self.total_packets_sent == 0 {
            return 0.0;
        }

        let retransmitted = self.retransmitted_packets as f32;
        let sent = self.total_packets_sent as f32;

        retransmitted / sent
    }

    /// Get loss history
    ///
    /// Returns a vector of the last N loss rate measurements
    pub fn get_loss_history(&self) -> Vec<f32> {
        self.loss_history.iter().copied().collect()
    }

    /// Get average loss rate from history
    ///
    /// Returns the average of all measurements in the loss history
    pub fn get_avg_loss_rate(&self) -> f32 {
        if self.loss_history.is_empty() {
            return 0.0;
        }

        let sum: f32 = self.loss_history.iter().sum();
        sum / self.loss_history.len() as f32
    }

    /// Detect congestion level based on current loss rate
    ///
    /// # Example
    /// ```
    /// use game_network::network_metrics::packet_loss_metrics::{PacketLossMetrics, CongestionLevel};
    ///
    /// let mut metrics = PacketLossMetrics::new();
    /// // Simulate 100 packets sent, 95 received (5% loss)
    /// for _ in 0..100 {
    ///     metrics.record_packet_sent();
    /// }
    /// for _ in 0..95 {
    ///     metrics.record_packet_received();
    /// }
    /// for _ in 0..5 {
    ///     metrics.record_packet_lost();
    /// }
    ///
    /// let congestion = metrics.detect_congestion();
    /// assert_eq!(congestion, CongestionLevel::Moderate);
    /// ```
    pub fn detect_congestion(&self) -> CongestionLevel {
        let loss_rate = self.calculate_loss_rate();

        if loss_rate > 0.20 {
            CongestionLevel::Critical
        } else if loss_rate > 0.10 {
            CongestionLevel::High
        } else if loss_rate > 0.05 {
            CongestionLevel::Moderate
        } else if loss_rate > 0.02 {
            CongestionLevel::Low
        } else {
            CongestionLevel::None
        }
    }

    /// Reset window for new measurement period
    ///
    /// Saves current loss rate to history and resets counters
    pub fn reset_window(&mut self) {
        let current_loss_rate = self.calculate_loss_rate();

        // Add to history
        self.loss_history.push_back(current_loss_rate);

        // Maintain max history size
        if self.loss_history.len() > self.max_history_size {
            self.loss_history.pop_front();
        }

        // Reset counters
        self.total_packets_sent = 0;
        self.total_packets_received = 0;
        self.retransmitted_packets = 0;
        self.packets_lost = 0;
        self.duplicate_packets = 0;
        self.out_of_order_packets = 0;
        self.start_time = Instant::now();
    }

    /// Get comprehensive statistics snapshot
    pub fn get_stats(&self) -> PacketLossStats {
        PacketLossStats {
            total_sent: self.total_packets_sent,
            total_received: self.total_packets_received,
            total_lost: self.packets_lost,
            total_retransmitted: self.retransmitted_packets,
            current_loss_rate: self.calculate_loss_rate(),
            avg_loss_rate: self.get_avg_loss_rate(),
            retransmission_rate: self.calculate_retransmission_rate(),
            window_duration: self.start_time.elapsed(),
            congestion_level: self.detect_congestion(),
            measurements_count: self.loss_history.len(),
        }
    }

    /// Get total packets sent (for integration purposes)
    pub fn total_sent(&self) -> u64 {
        self.total_packets_sent
    }

    /// Get total packets received (for integration purposes)
    pub fn total_received(&self) -> u64 {
        self.total_packets_received
    }

    /// Get total duplicate packets
    pub fn total_duplicates(&self) -> u64 {
        self.duplicate_packets
    }

    /// Get total out-of-order packets
    pub fn total_out_of_order(&self) -> u64 {
        self.out_of_order_packets
    }
}

impl Default for PacketLossMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = PacketLossMetrics::new();

        assert_eq!(metrics.total_packets_sent, 0);
        assert_eq!(metrics.total_packets_received, 0);
        assert_eq!(metrics.retransmitted_packets, 0);
        assert_eq!(metrics.packets_lost, 0);
        assert_eq!(metrics.calculate_loss_rate(), 0.0);
        assert_eq!(metrics.detect_congestion(), CongestionLevel::None);
    }

    #[test]
    fn test_record_packets_sent_received() {
        let mut metrics = PacketLossMetrics::new();

        // Send 100 packets, receive 95
        for _ in 0..100 {
            metrics.record_packet_sent();
        }
        for _ in 0..95 {
            metrics.record_packet_received();
        }
        for _ in 0..5 {
            metrics.record_packet_lost();
        }

        assert_eq!(metrics.total_packets_sent, 100);
        assert_eq!(metrics.total_packets_received, 95);
        assert_eq!(metrics.packets_lost, 5);

        let loss_rate = metrics.calculate_loss_rate();
        assert!((loss_rate - 0.05).abs() < 0.001); // 5% loss
    }

    #[test]
    fn test_record_retransmissions() {
        let mut metrics = PacketLossMetrics::new();

        // Send 100, receive 95, retransmit 10
        for _ in 0..100 {
            metrics.record_packet_sent();
        }
        for _ in 0..95 {
            metrics.record_packet_received();
        }
        for _ in 0..5 {
            metrics.record_packet_lost();
        }
        for _ in 0..10 {
            metrics.record_retransmission();
        }

        let retransmission_rate = metrics.calculate_retransmission_rate();
        assert!((retransmission_rate - 0.10).abs() < 0.001); // 10% retransmission rate
    }

    #[test]
    fn test_congestion_detection() {
        let mut metrics = PacketLossMetrics::new();

        // Test 1% loss - None
        for _ in 0..100 {
            metrics.record_packet_sent();
        }
        for _ in 0..99 {
            metrics.record_packet_received();
        }
        metrics.record_packet_lost();
        assert_eq!(metrics.detect_congestion(), CongestionLevel::None);

        metrics = PacketLossMetrics::new();

        // Test 3% loss - Low
        for _ in 0..100 {
            metrics.record_packet_sent();
        }
        for _ in 0..97 {
            metrics.record_packet_received();
        }
        for _ in 0..3 {
            metrics.record_packet_lost();
        }
        assert_eq!(metrics.detect_congestion(), CongestionLevel::Low);

        metrics = PacketLossMetrics::new();

        // Test 7% loss - Moderate
        for _ in 0..100 {
            metrics.record_packet_sent();
        }
        for _ in 0..93 {
            metrics.record_packet_received();
        }
        for _ in 0..7 {
            metrics.record_packet_lost();
        }
        assert_eq!(metrics.detect_congestion(), CongestionLevel::Moderate);

        metrics = PacketLossMetrics::new();

        // Test 15% loss - High
        for _ in 0..100 {
            metrics.record_packet_sent();
        }
        for _ in 0..85 {
            metrics.record_packet_received();
        }
        for _ in 0..15 {
            metrics.record_packet_lost();
        }
        assert_eq!(metrics.detect_congestion(), CongestionLevel::High);

        metrics = PacketLossMetrics::new();

        // Test 25% loss - Critical
        for _ in 0..100 {
            metrics.record_packet_sent();
        }
        for _ in 0..75 {
            metrics.record_packet_received();
        }
        for _ in 0..25 {
            metrics.record_packet_lost();
        }
        assert_eq!(metrics.detect_congestion(), CongestionLevel::Critical);
    }

    #[test]
    fn test_loss_history() {
        let mut metrics = PacketLossMetrics::new();

        // Record multiple measurement windows
        for i in 0..15 {
            // Create different loss rates: i% loss
            for _ in 0..100 {
                metrics.record_packet_sent();
            }
            for _ in 0..(100 - i) {
                metrics.record_packet_received();
            }
            for _ in 0..i {
                metrics.record_packet_lost();
            }

            metrics.reset_window();
        }

        let history = metrics.get_loss_history();

        // Should only keep last 10 measurements
        assert_eq!(history.len(), 10);

        // History should contain measurements from windows 5-14
        for (idx, &loss_rate) in history.iter().enumerate() {
            let expected_loss = (idx + 5) as f32 / 100.0;
            assert!((loss_rate - expected_loss).abs() < 0.001);
        }

        // Average should be average of 5% to 14% = 9.5%
        let avg = metrics.get_avg_loss_rate();
        assert!((avg - 0.095).abs() < 0.001);
    }

    #[test]
    fn test_reset_window() {
        let mut metrics = PacketLossMetrics::new();

        // Record some metrics
        for _ in 0..100 {
            metrics.record_packet_sent();
        }
        for _ in 0..95 {
            metrics.record_packet_received();
        }
        for _ in 0..5 {
            metrics.record_packet_lost();
        }
        for _ in 0..10 {
            metrics.record_retransmission();
        }

        assert_eq!(metrics.total_packets_sent, 100);

        metrics.reset_window();

        // Counters should be reset
        assert_eq!(metrics.total_packets_sent, 0);
        assert_eq!(metrics.total_packets_received, 0);
        assert_eq!(metrics.packets_lost, 0);
        assert_eq!(metrics.retransmitted_packets, 0);

        // History should have one entry
        assert_eq!(metrics.loss_history.len(), 1);
        assert!((metrics.loss_history[0] - 0.05).abs() < 0.001);
    }

    #[test]
    fn test_network_health_report() {
        let mut metrics = PacketLossMetrics::new();

        // Simulate moderate congestion
        for _ in 0..100 {
            metrics.record_packet_sent();
        }
        for _ in 0..93 {
            metrics.record_packet_received();
        }
        for _ in 0..7 {
            metrics.record_packet_lost();
        }
        for _ in 0..5 {
            metrics.record_retransmission();
        }

        let stats = metrics.get_stats();

        assert_eq!(stats.total_sent, 100);
        assert_eq!(stats.total_received, 93);
        assert_eq!(stats.total_lost, 7);
        assert_eq!(stats.total_retransmitted, 5);
        assert!((stats.current_loss_rate - 0.07).abs() < 0.001);
        assert!((stats.retransmission_rate - 0.05).abs() < 0.001);
        assert_eq!(stats.congestion_level, CongestionLevel::Moderate);
    }

    #[test]
    fn test_duplicate_and_ooo() {
        let mut metrics = PacketLossMetrics::new();

        // Record various events
        for _ in 0..50 {
            metrics.record_packet_sent();
        }
        for _ in 0..45 {
            metrics.record_packet_received();
        }
        for _ in 0..3 {
            metrics.record_duplicate();
        }
        for _ in 0..7 {
            metrics.record_out_of_order();
        }
        for _ in 0..2 {
            metrics.record_packet_lost();
        }

        assert_eq!(metrics.total_packets_sent, 50);
        assert_eq!(metrics.total_packets_received, 45);
        assert_eq!(metrics.duplicate_packets, 3);
        assert_eq!(metrics.out_of_order_packets, 7);
        assert_eq!(metrics.packets_lost, 2);

        let loss_rate = metrics.calculate_loss_rate();
        assert!((loss_rate - 0.04).abs() < 0.001); // 2/50 = 4%
    }

    #[test]
    fn test_congestion_level_recommendations() {
        assert_eq!(CongestionLevel::None.recommendation(), "Network OK");
        assert_eq!(
            CongestionLevel::Low.recommendation(),
            "Monitor network conditions"
        );
        assert_eq!(
            CongestionLevel::Moderate.recommendation(),
            "Consider reducing data rate"
        );
        assert_eq!(
            CongestionLevel::High.recommendation(),
            "Reduce frame rate and data transmission"
        );
        assert_eq!(
            CongestionLevel::Critical.recommendation(),
            "Critical congestion - minimize traffic"
        );
    }

    #[test]
    fn test_zero_packets_sent() {
        let metrics = PacketLossMetrics::new();

        // With no packets sent, everything should be 0
        assert_eq!(metrics.calculate_loss_rate(), 0.0);
        assert_eq!(metrics.calculate_retransmission_rate(), 0.0);
        assert_eq!(metrics.get_avg_loss_rate(), 0.0);
        assert_eq!(metrics.detect_congestion(), CongestionLevel::None);
    }
}
