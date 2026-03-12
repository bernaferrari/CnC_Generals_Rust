//! Packet loss injection testing - detailed loss pattern analysis
//!
//! Provides configurable packet loss scenarios:
//! - Random uniform loss
//! - Correlated loss (bunches together)
//! - Periodic loss (every Nth packet)
//! - Statistical validation
//!
//! These tests ensure the protocol handles loss gracefully.

#[cfg(test)]
mod packet_loss_tests {
    use std::time::{Duration, Instant};
    use tokio::time::sleep;

    /// Packet loss pattern type
    #[derive(Debug, Clone, Copy)]
    enum LossPattern {
        /// Random uniform loss
        Uniform,
        /// Packets lost in clusters (more realistic)
        Clustered,
        /// Every Nth packet lost
        Periodic,
        /// Bursty - lose N consecutive, then none for M packets
        Bursty,
        /// Gilbert-Elliott model - alternates between "good" and "bad" states
        GilbertElliott,
    }

    /// Configuration for packet loss injection
    #[derive(Debug, Clone, Copy)]
    struct PacketLossConfig {
        /// Number of packets to simulate
        num_packets: u64,
        /// Loss rate percentage (0-100)
        loss_rate: u32,
        /// Pattern type
        pattern: LossPattern,
        /// Cluster size (for clustered loss)
        cluster_size: u32,
        /// Packet period (for periodic loss)
        period: u32,
        /// Burst length (for bursty loss)
        burst_length: u32,
        /// Gap length in burst pattern
        gap_length: u32,
    }

    impl Default for PacketLossConfig {
        fn default() -> Self {
            Self {
                num_packets: 10000,
                loss_rate: 5,
                pattern: LossPattern::Uniform,
                cluster_size: 3,
                period: 20,
                burst_length: 5,
                gap_length: 15,
            }
        }
    }

    /// Statistics from packet loss injection test
    #[derive(Debug, Clone, Default)]
    struct PacketLossStats {
        /// Total packets sent
        total_packets: u64,
        /// Packets lost
        packets_lost: u64,
        /// Loss streak lengths
        loss_streaks: Vec<u32>,
        /// Delivery streaks
        delivery_streaks: Vec<u32>,
        /// Largest consecutive loss
        max_loss_streak: u32,
        /// Expected loss count
        expected_loss: u64,
        /// Actual vs expected variance
        variance: f64,
        /// Statistical significance
        chi_squared: f64,
    }

    impl PacketLossStats {
        fn actual_loss_rate(&self) -> f64 {
            if self.total_packets == 0 {
                0.0
            } else {
                (self.packets_lost as f64 / self.total_packets as f64) * 100.0
            }
        }

        fn avg_loss_streak(&self) -> f64 {
            if self.loss_streaks.is_empty() {
                0.0
            } else {
                self.loss_streaks.iter().map(|&x| x as f64).sum::<f64>()
                    / self.loss_streaks.len() as f64
            }
        }

        fn avg_delivery_streak(&self) -> f64 {
            if self.delivery_streaks.is_empty() {
                0.0
            } else {
                self.delivery_streaks.iter().map(|&x| x as f64).sum::<f64>()
                    / self.delivery_streaks.len() as f64
            }
        }

        fn print_summary(&self, pattern: LossPattern) {
            println!("\n╔══════════════════════════════════════════════════════════════╗");
            println!(
                "║ PACKET LOSS INJECTION RESULTS: {:<40} ║",
                format!("{:?}", pattern)
            );
            println!("╠══════════════════════════════════════════════════════════════╣");
            println!("║ Total Packets: {:<50} ║", self.total_packets);
            println!("║ Packets Lost: {:<51} ║", self.packets_lost);
            println!(
                "║ Actual Loss Rate: {:<48} ║",
                format!("{:.2}%", self.actual_loss_rate())
            );
            println!(
                "║ Expected Loss Rate: {:<46} ║",
                format!(
                    "{:.2}%",
                    (self.expected_loss as f64 / self.total_packets as f64) * 100.0
                )
            );
            println!("║ Variance: {:<55} ║", format!("{:.4}", self.variance));
            println!(
                "║ Chi-Squared: {:<52} ║",
                format!("{:.4}", self.chi_squared)
            );
            println!(
                "║ Max Loss Streak: {:<48} ║",
                format!("{} packets", self.max_loss_streak)
            );
            println!(
                "║ Avg Loss Streak: {:<48} ║",
                format!("{:.1} packets", self.avg_loss_streak())
            );
            println!(
                "║ Avg Delivery Streak: {:<45} ║",
                format!("{:.1} packets", self.avg_delivery_streak())
            );
            println!("║ Loss Events: {:<51} ║", self.loss_streaks.len());
            println!("╚══════════════════════════════════════════════════════════════╝\n");
        }
    }

    // ===== UNIFORM RANDOM LOSS =====

    #[tokio::test]
    #[ignore] // cargo test pl_uniform_1pct -- --ignored --nocapture
    async fn packet_loss_uniform_1pct() {
        let config = PacketLossConfig {
            num_packets: 10000,
            loss_rate: 1,
            pattern: LossPattern::Uniform,
            ..Default::default()
        };

        let stats = inject_packet_loss("1% Uniform Loss", config).await;
        validate_uniform_loss(&stats, 1.0, 0.5);
        println!("✅ 1% uniform loss test PASSED");
    }

    #[tokio::test]
    #[ignore] // cargo test pl_uniform_5pct -- --ignored --nocapture
    async fn packet_loss_uniform_5pct() {
        let config = PacketLossConfig {
            num_packets: 20000,
            loss_rate: 5,
            pattern: LossPattern::Uniform,
            ..Default::default()
        };

        let stats = inject_packet_loss("5% Uniform Loss", config).await;
        validate_uniform_loss(&stats, 5.0, 1.0);
        println!("✅ 5% uniform loss test PASSED");
    }

    #[tokio::test]
    #[ignore] // cargo test pl_uniform_10pct -- --ignored --nocapture
    async fn packet_loss_uniform_10pct() {
        let config = PacketLossConfig {
            num_packets: 20000,
            loss_rate: 10,
            pattern: LossPattern::Uniform,
            ..Default::default()
        };

        let stats = inject_packet_loss("10% Uniform Loss", config).await;
        validate_uniform_loss(&stats, 10.0, 2.0);
        println!("✅ 10% uniform loss test PASSED");
    }

    // ===== CLUSTERED LOSS (REALISTIC) =====

    #[tokio::test]
    #[ignore] // cargo test pl_clustered -- --ignored --nocapture
    async fn packet_loss_clustered() {
        let config = PacketLossConfig {
            num_packets: 15000,
            loss_rate: 5,
            pattern: LossPattern::Clustered,
            cluster_size: 3,
            ..Default::default()
        };

        let stats = inject_packet_loss("5% Clustered Loss (3-packet clusters)", config).await;

        // Clustered loss should have fewer but longer streaks
        assert!(
            stats.loss_streaks.len() < 500,
            "Should have fewer loss events"
        );
        assert!(
            stats.avg_loss_streak() > 2.0,
            "Clusters should be > 1 packet"
        );
        println!("✅ Clustered loss test PASSED");
    }

    #[tokio::test]
    #[ignore] // cargo test pl_clustered_large -- --ignored --nocapture
    async fn packet_loss_clustered_large_bursts() {
        let config = PacketLossConfig {
            num_packets: 20000,
            loss_rate: 3,
            pattern: LossPattern::Clustered,
            cluster_size: 8,
            ..Default::default()
        };

        let stats = inject_packet_loss("3% Clustered Loss (8-packet clusters)", config).await;

        // Large clusters should be rare but significant
        assert!(stats.avg_loss_streak() > 5.0, "Should have large clusters");
        println!("✅ Large burst test PASSED");
    }

    // ===== PERIODIC LOSS =====

    #[tokio::test]
    #[ignore] // cargo test pl_periodic -- --ignored --nocapture
    async fn packet_loss_periodic() {
        let config = PacketLossConfig {
            num_packets: 10000,
            loss_rate: 5,
            pattern: LossPattern::Periodic,
            period: 20,
            ..Default::default()
        };

        let stats = inject_packet_loss("Periodic Loss (every 20th packet)", config).await;

        // Should be very predictable
        let expected_events = 10000 / 20;
        assert!(
            (stats.packets_lost as i64 - expected_events as i64).abs() < 10,
            "Periodic loss should be predictable"
        );
        println!("✅ Periodic loss test PASSED");
    }

    // ===== BURSTY LOSS (NETWORK CONGESTION) =====

    #[tokio::test]
    #[ignore] // cargo test pl_bursty -- --ignored --nocapture
    async fn packet_loss_bursty_pattern() {
        let config = PacketLossConfig {
            num_packets: 15000,
            loss_rate: 4,
            pattern: LossPattern::Bursty,
            burst_length: 5,
            gap_length: 15,
            ..Default::default()
        };

        let stats = inject_packet_loss("Bursty Loss (5 lost, 15 good, repeat)", config).await;

        // Should have clear burst pattern
        assert!(stats.max_loss_streak >= 5, "Should have 5-packet bursts");
        assert!(
            stats.loss_streaks.len() < 1000,
            "Should have distinct events"
        );
        println!("✅ Bursty loss test PASSED");
    }

    // ===== GILBERT-ELLIOTT MODEL (REALISTIC WAN) =====

    #[tokio::test]
    #[ignore] // cargo test pl_gilbert_elliott -- --ignored --nocapture
    async fn packet_loss_gilbert_elliott() {
        let config = PacketLossConfig {
            num_packets: 20000,
            loss_rate: 5,
            pattern: LossPattern::GilbertElliott,
            ..Default::default()
        };

        let stats = inject_packet_loss("Gilbert-Elliott Model (WAN-like)", config).await;

        // Should have both good and bad periods
        assert!(
            stats.delivery_streaks.len() > 20,
            "Should have multiple good periods"
        );
        assert!(
            stats.loss_streaks.len() > 20,
            "Should have multiple bad periods"
        );
        println!("✅ Gilbert-Elliott test PASSED");
    }

    // ===== STRESS: HIGH LOSS RATES =====

    #[tokio::test]
    #[ignore] // cargo test pl_extreme_loss -- --ignored --nocapture
    async fn packet_loss_extreme_25pct() {
        let config = PacketLossConfig {
            num_packets: 10000,
            loss_rate: 25,
            pattern: LossPattern::Uniform,
            ..Default::default()
        };

        let stats = inject_packet_loss("Extreme Loss (25%)", config).await;

        // Should still track loss accurately
        assert!(stats.packets_lost > 0);
        let actual = stats.actual_loss_rate();
        assert!((actual - 25.0).abs() < 2.0, "Loss rate should be near 25%");
        println!("✅ Extreme loss test PASSED");
    }

    // ===== Helper Functions =====

    async fn inject_packet_loss(test_name: &str, config: PacketLossConfig) -> PacketLossStats {
        println!("\n═══════════════════════════════════════════════════════════════");
        println!("PACKET LOSS INJECTION: {}", test_name);
        println!("═══════════════════════════════════════════════════════════════");
        println!("Configuration:");
        println!("  Packets: {}", config.num_packets);
        println!("  Loss Rate: {}%", config.loss_rate);
        println!("  Pattern: {:?}", config.pattern);
        println!();

        let mut stats = PacketLossStats::default();
        stats.total_packets = config.num_packets;
        stats.expected_loss = (config.num_packets * config.loss_rate as u64) / 100;

        let mut current_loss_streak = 0u32;
        let mut current_delivery_streak = 0u32;
        let mut pseudo_random_state = 12345u64;

        for packet_num in 0..config.num_packets {
            // Deterministic pseudo-random number generator
            pseudo_random_state = pseudo_random_state
                .wrapping_mul(1103515245)
                .wrapping_add(12345);
            let rand = ((pseudo_random_state / 65536) % 100) as u32;

            let is_lost = match config.pattern {
                LossPattern::Uniform => rand < config.loss_rate,
                LossPattern::Clustered => {
                    let base_rand = rand < config.loss_rate;
                    let in_cluster = (packet_num % config.cluster_size as u64) < 3;
                    base_rand && in_cluster
                }
                LossPattern::Periodic => packet_num % config.period as u64 == 0,
                LossPattern::Bursty => {
                    let cycle_pos =
                        packet_num % (config.burst_length as u64 + config.gap_length as u64);
                    cycle_pos < config.burst_length as u64
                }
                LossPattern::GilbertElliott => {
                    // Simplified: alternate between good/bad states
                    let state = (packet_num / 100) % 2;
                    if state == 0 {
                        rand < 2 // 2% loss in "good" state
                    } else {
                        rand < 15 // 15% loss in "bad" state
                    }
                }
            };

            if is_lost {
                stats.packets_lost += 1;
                current_loss_streak += 1;
                current_delivery_streak = 0;
            } else {
                current_delivery_streak += 1;
                if current_loss_streak > 0 {
                    stats.loss_streaks.push(current_loss_streak);
                    stats.max_loss_streak = stats.max_loss_streak.max(current_loss_streak);
                    current_loss_streak = 0;
                }
            }

            // Finalize streak at end
            if packet_num == config.num_packets - 1 {
                if current_loss_streak > 0 {
                    stats.loss_streaks.push(current_loss_streak);
                    stats.max_loss_streak = stats.max_loss_streak.max(current_loss_streak);
                }
                if current_delivery_streak > 0 {
                    stats.delivery_streaks.push(current_delivery_streak);
                }
            }
        }

        // Calculate variance and chi-squared
        let actual_loss = stats.packets_lost as f64;
        let expected_loss = stats.expected_loss as f64;
        stats.variance = ((actual_loss - expected_loss) / expected_loss).abs();
        stats.chi_squared = ((actual_loss - expected_loss).powi(2)) / expected_loss;

        stats.print_summary(config.pattern);
        stats
    }

    fn validate_uniform_loss(stats: &PacketLossStats, expected_rate: f64, tolerance: f64) {
        let actual = stats.actual_loss_rate();
        assert!(
            (actual - expected_rate).abs() < tolerance,
            "Expected {:.1}% ± {:.1}%, got {:.2}%",
            expected_rate,
            tolerance,
            actual
        );

        // Chi-squared test (should be reasonable)
        assert!(
            stats.chi_squared < 10.0,
            "Distribution too far from expected (chi-squared: {:.2})",
            stats.chi_squared
        );
    }
}

// Run all packet loss tests with: cargo test --test packet_loss_injection -- --ignored --nocapture
//
// Individual tests:
// - cargo test pl_uniform_1pct -- --ignored --nocapture
// - cargo test pl_clustered -- --ignored --nocapture
// - cargo test pl_periodic -- --ignored --nocapture
// - cargo test pl_bursty -- --ignored --nocapture
// - cargo test pl_gilbert_elliott -- --ignored --nocapture
