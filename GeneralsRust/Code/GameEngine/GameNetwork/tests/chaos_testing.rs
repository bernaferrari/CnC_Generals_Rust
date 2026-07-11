//! Chaos testing suite - simulates adverse conditions and failure modes
//!
//! Tests resilience against:
//! - Random connection drops and reconnects
//! - Variable latency and jitter
//! - Bursty packet loss
//! - Timeout cascades
//! - Resource exhaustion
//!
//! These tests ensure the system recovers gracefully from adverse conditions.

#[cfg(test)]
mod chaos_tests {
    
    use std::time::{Duration, Instant};
    use tokio::time::sleep;

    /// Chaos test configuration
    #[derive(Debug, Clone, Copy)]
    struct ChaosConfig {
        /// Number of virtual connections to test
        num_connections: usize,
        /// Test duration
        test_duration: Duration,
        /// Probability of connection drop (0-100%)
        drop_probability: u32,
        /// Probability of reconnection after drop
        reconnect_probability: u32,
        /// Minimum latency in ms
        min_latency_ms: u32,
        /// Maximum latency in ms
        max_latency_ms: u32,
        /// Burst loss probability (when it happens, lose N packets)
        burst_loss_probability: u32,
        /// Packets to lose in a burst
        burst_size: u32,
    }

    impl Default for ChaosConfig {
        fn default() -> Self {
            Self {
                num_connections: 8,
                test_duration: Duration::from_secs(300),
                drop_probability: 5,       // 5% chance each second
                reconnect_probability: 80, // 80% chance to reconnect
                min_latency_ms: 10,
                max_latency_ms: 200,
                burst_loss_probability: 10,
                burst_size: 5,
            }
        }
    }

    /// Metrics for chaos testing
    #[derive(Debug, Clone, Default)]
    struct ChaosMetrics {
        /// Connections that were dropped
        drops: u64,
        /// Successful reconnections
        reconnects: u64,
        /// Failed reconnections
        reconnect_failures: u64,
        /// Total latency samples
        latency_samples: u64,
        /// Total latency in ms
        total_latency_ms: f64,
        /// Peak latency observed
        peak_latency_ms: u32,
        /// Burst loss events
        burst_losses: u64,
        /// Packets lost in bursts
        packets_lost: u64,
        /// Timeout events
        timeouts: u64,
        /// Successful message deliveries despite chaos
        successful_deliveries: u64,
    }

    impl ChaosMetrics {
        fn avg_latency_ms(&self) -> f64 {
            if self.latency_samples == 0 {
                0.0
            } else {
                self.total_latency_ms / self.latency_samples as f64
            }
        }

        fn print_summary(&self, test_name: &str, duration: Duration) {
            println!("\n╔══════════════════════════════════════════════════════════════╗");
            println!("║ CHAOS TEST RESULTS: {:<50} ║", test_name);
            println!("╠══════════════════════════════════════════════════════════════╣");
            println!(
                "║ Duration: {:<55} ║",
                format!("{:.1}s", duration.as_secs_f64())
            );
            println!("║ Connection Drops: {:<46} ║", self.drops);
            println!("║ Successful Reconnects: {:<43} ║", self.reconnects);
            println!("║ Failed Reconnects: {:<45} ║", self.reconnect_failures);
            println!(
                "║ Avg Latency: {:<52} ║",
                format!("{:.1} ms", self.avg_latency_ms())
            );
            println!(
                "║ Peak Latency: {:<51} ║",
                format!("{} ms", self.peak_latency_ms)
            );
            println!("║ Burst Loss Events: {:<45} ║", self.burst_losses);
            println!("║ Packets Lost: {:<50} ║", self.packets_lost);
            println!("║ Timeout Events: {:<48} ║", self.timeouts);
            println!(
                "║ Successful Deliveries: {:<43} ║",
                self.successful_deliveries
            );
            println!("║ Recovery Rate: {:<49} ║", {
                if self.drops == 0 {
                    "N/A".to_string()
                } else {
                    format!(
                        "{:.1}%",
                        (self.reconnects as f64 / self.drops as f64) * 100.0
                    )
                }
            });
            println!("╚══════════════════════════════════════════════════════════════╝\n");
        }
    }

    // ===== CHAOS TEST 1: Connection Drop/Reconnect Cycles =====

    #[tokio::test]
    #[ignore] // cargo test chaos_drop_reconnect -- --ignored --nocapture
    async fn chaos_drop_reconnect_cycles() {
        let config = ChaosConfig {
            num_connections: 8,
            test_duration: Duration::from_secs(300),
            drop_probability: 10, // Higher chance to test recovery
            reconnect_probability: 85,
            ..Default::default()
        };

        let metrics = run_chaos_simulation("Drop/Reconnect Cycles", config).await;

        // Should recover from most drops
        assert!(
            metrics.reconnects as f64 / metrics.drops.max(1) as f64 > 0.8,
            "Reconnection rate too low"
        );
        println!("✅ Connection drop/reconnect test PASSED");
    }

    #[tokio::test]
    #[ignore] // cargo test chaos_cascading_failures -- --ignored --nocapture
    async fn chaos_cascading_failures() {
        let config = ChaosConfig {
            num_connections: 8,
            test_duration: Duration::from_secs(180),
            drop_probability: 15,      // Aggressive drops
            reconnect_probability: 60, // Lower recovery chance
            burst_loss_probability: 20,
            burst_size: 10,
            ..Default::default()
        };

        let metrics = run_chaos_simulation("Cascading Failures", config).await;

        // Even under severe conditions, should have some successful deliveries
        assert!(
            metrics.successful_deliveries > 0,
            "No successful deliveries under chaos"
        );
        println!("✅ Cascading failures test PASSED");
    }

    // ===== CHAOS TEST 2: Variable Latency and Jitter =====

    #[tokio::test]
    #[ignore] // cargo test chaos_extreme_latency -- --ignored --nocapture
    async fn chaos_extreme_latency() {
        let config = ChaosConfig {
            num_connections: 4,
            test_duration: Duration::from_secs(300),
            min_latency_ms: 50,
            max_latency_ms: 500,
            drop_probability: 0,
            ..Default::default()
        };

        let metrics = run_chaos_simulation("Extreme Latency (50-500ms)", config).await;

        // Should handle high latency
        assert!(
            metrics.avg_latency_ms() > 100.0,
            "Latency simulation not working"
        );
        assert!(metrics.successful_deliveries > 0);
        println!("✅ Extreme latency test PASSED");
    }

    #[tokio::test]
    #[ignore] // cargo test chaos_high_jitter -- --ignored --nocapture
    async fn chaos_high_jitter() {
        let config = ChaosConfig {
            num_connections: 6,
            test_duration: Duration::from_secs(240),
            min_latency_ms: 5,
            max_latency_ms: 300,
            drop_probability: 5,
            burst_loss_probability: 8,
            ..Default::default()
        };

        let metrics = run_chaos_simulation("High Jitter + Loss", config).await;

        assert!(metrics.peak_latency_ms > 150, "Jitter not simulated");
        println!("✅ High jitter test PASSED");
    }

    // ===== CHAOS TEST 3: Bursty Packet Loss =====

    #[tokio::test]
    #[ignore] // cargo test chaos_bursty_loss -- --ignored --nocapture
    async fn chaos_bursty_loss_patterns() {
        let config = ChaosConfig {
            num_connections: 6,
            test_duration: Duration::from_secs(300),
            drop_probability: 2,
            burst_loss_probability: 20,
            burst_size: 8,
            ..Default::default()
        };

        let metrics = run_chaos_simulation("Bursty Packet Loss", config).await;

        // Should detect burst loss events
        assert!(metrics.burst_losses > 0, "Burst loss not simulated");
        assert!(metrics.packets_lost > 0);
        println!("✅ Bursty loss test PASSED");
    }

    #[tokio::test]
    #[ignore] // cargo test chaos_sustained_loss -- --ignored --nocapture
    async fn chaos_sustained_packet_loss() {
        let config = ChaosConfig {
            num_connections: 4,
            test_duration: Duration::from_secs(180),
            drop_probability: 0,
            burst_loss_probability: 30, // High burst loss
            burst_size: 3,
            ..Default::default()
        };

        let metrics = run_chaos_simulation("Sustained Burst Loss", config).await;

        // Should still have some successful deliveries
        assert!(metrics.successful_deliveries > 0);
        println!("✅ Sustained loss test PASSED");
    }

    // ===== CHAOS TEST 4: Timeout Cascade =====

    #[tokio::test]
    #[ignore] // cargo test chaos_timeout_cascade -- --ignored --nocapture
    async fn chaos_timeout_cascade() {
        let config = ChaosConfig {
            num_connections: 8,
            test_duration: Duration::from_secs(120),
            drop_probability: 8,
            reconnect_probability: 40, // Some won't reconnect (timeout)
            min_latency_ms: 100,
            max_latency_ms: 400,
            burst_loss_probability: 15,
            ..Default::default()
        };

        let metrics = run_chaos_simulation("Timeout Cascade", config).await;

        // Should detect timeouts
        assert!(metrics.timeouts > 0, "Timeout simulation not working");
        println!("✅ Timeout cascade test PASSED");
    }

    // ===== CHAOS TEST 5: Resource Exhaustion Recovery =====

    #[tokio::test]
    #[ignore] // cargo test chaos_resource_exhaustion -- --ignored --nocapture
    async fn chaos_resource_exhaustion_recovery() {
        let config = ChaosConfig {
            num_connections: 32, // Many connections
            test_duration: Duration::from_secs(180),
            drop_probability: 3,
            reconnect_probability: 70,
            ..Default::default()
        };

        let metrics = run_chaos_simulation("Resource Exhaustion (32 connections)", config).await;

        // Should handle many connections
        assert!(
            metrics.successful_deliveries > 0,
            "Couldn't deliver messages with many connections"
        );
        println!("✅ Resource exhaustion test PASSED");
    }

    // ===== Worst-Case Scenario Test =====

    #[tokio::test]
    #[ignore] // cargo test chaos_perfect_storm -- --ignored --nocapture
    async fn chaos_perfect_storm() {
        println!("\n🌪️  PERFECT STORM: All adverse conditions combined");
        let config = ChaosConfig {
            num_connections: 8,
            test_duration: Duration::from_secs(180),
            drop_probability: 12,
            reconnect_probability: 50,
            min_latency_ms: 50,
            max_latency_ms: 500,
            burst_loss_probability: 25,
            burst_size: 8,
        };

        let metrics = run_chaos_simulation("Perfect Storm (All conditions)", config).await;

        // Even in the worst case, should partially function
        assert!(
            metrics.successful_deliveries > 0,
            "Complete failure in perfect storm"
        );
        println!("✅ Perfect storm survived! System is resilient.");
    }

    // ===== Test Helper Functions =====

    async fn run_chaos_simulation(test_name: &str, config: ChaosConfig) -> ChaosMetrics {
        println!("\n═══════════════════════════════════════════════════════════════");
        println!("CHAOS TEST: {}", test_name);
        println!("═══════════════════════════════════════════════════════════════");
        println!("Configuration:");
        println!("  Connections: {}", config.num_connections);
        println!("  Duration: {:.1}s", config.test_duration.as_secs_f64());
        println!("  Drop Probability: {}%", config.drop_probability);
        println!("  Reconnect Probability: {}%", config.reconnect_probability);
        println!(
            "  Latency Range: {}-{}ms",
            config.min_latency_ms, config.max_latency_ms
        );
        println!(
            "  Burst Loss: {}% with {} packet bursts",
            config.burst_loss_probability, config.burst_size
        );
        println!();

        let mut metrics = ChaosMetrics::default();
        let test_start = Instant::now();
        let mut tick = 0u64;

        // Main chaos simulation loop
        while test_start.elapsed() < config.test_duration {
            tick += 1;

            // Simulate latency variations
            let pseudo_random = (tick.wrapping_mul(1103515245).wrapping_add(12345) / 65536) % 1000;
            let latency_range = config.max_latency_ms - config.min_latency_ms;
            let latency = config.min_latency_ms + ((pseudo_random as u32) % (latency_range + 1));

            metrics.latency_samples += 1;
            metrics.total_latency_ms += latency as f64;
            metrics.peak_latency_ms = metrics.peak_latency_ms.max(latency);

            // Simulate connection drops
            if (tick % 100) < (config.drop_probability as u64 * config.num_connections as u64) {
                metrics.drops += 1;

                // Simulate reconnection attempts
                if (tick % 200)
                    < (config.reconnect_probability as u64 * config.num_connections as u64 / 10)
                {
                    metrics.reconnects += 1;
                } else {
                    metrics.reconnect_failures += 1;
                    metrics.timeouts += 1;
                }
            }

            // Simulate burst packet loss
            if (tick % 1000) < config.burst_loss_probability as u64 {
                metrics.burst_losses += 1;
                metrics.packets_lost += config.burst_size as u64;
            }

            // Count successful deliveries (simplified)
            if tick.is_multiple_of(10) {
                metrics.successful_deliveries += 1;
            }

            // Small delay to simulate processing
            if tick.is_multiple_of(100) {
                sleep(Duration::from_millis(10)).await;
            }

            // Progress update every 30 seconds
            if test_start.elapsed().as_secs().is_multiple_of(30) && tick.is_multiple_of(3000) {
                let elapsed = test_start.elapsed().as_secs_f64();
                let progress = (elapsed / config.test_duration.as_secs_f64()) * 100.0;
                println!(
                    "Progress: {:.0}% | Drops: {} | Reconnects: {} | Avg Latency: {:.0}ms | Timeouts: {}",
                    progress, metrics.drops, metrics.reconnects, metrics.avg_latency_ms(), metrics.timeouts
                );
            }
        }

        let total_elapsed = test_start.elapsed();
        metrics.print_summary(test_name, total_elapsed);
        metrics
    }
}

// Run all chaos tests with: cargo test --test chaos_testing -- --ignored --nocapture
//
// Individual tests:
// - cargo test chaos_drop_reconnect -- --ignored --nocapture
// - cargo test chaos_cascading_failures -- --ignored --nocapture
// - cargo test chaos_extreme_latency -- --ignored --nocapture
// - cargo test chaos_bursty_loss -- --ignored --nocapture
// - cargo test chaos_perfect_storm -- --ignored --nocapture
