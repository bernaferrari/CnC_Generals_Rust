//! Memory Pool Statistics and Monitoring
//!
//! Provides comprehensive tracking of pool usage, allocation patterns,
//! and performance metrics. Useful for optimization and debugging.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Statistics for memory allocation and usage.
#[derive(Debug)]
pub struct PoolStats {
    /// Pool name for identification.
    pub name: String,

    /// Total number of allocations performed.
    pub total_allocations: AtomicU64,

    /// Total number of deallocations performed.
    pub total_deallocations: AtomicU64,

    /// Current number of active allocations.
    pub active_allocations: AtomicUsize,

    /// Peak number of concurrent allocations.
    pub peak_allocations: AtomicUsize,

    /// Total capacity (number of slots).
    pub total_capacity: AtomicUsize,

    /// Number of times the pool had to grow.
    pub growth_count: AtomicU64,

    /// Number of allocation failures (when pool is full and can't grow).
    pub allocation_failures: AtomicU64,

    /// Total bytes allocated for pool storage.
    pub bytes_allocated: AtomicUsize,

    /// Bytes currently in use.
    pub bytes_in_use: AtomicUsize,

    /// Peak bytes in use.
    pub peak_bytes_in_use: AtomicUsize,

    /// Time spent in allocations (microseconds).
    pub alloc_time_us: AtomicU64,

    /// Time spent in deallocations (microseconds).
    pub dealloc_time_us: AtomicU64,

    /// Pool creation time.
    pub created_at: Instant,

    /// Last reset time.
    pub last_reset: Option<Instant>,
}

impl PoolStats {
    /// Create new pool statistics.
    pub fn new(name: String) -> Self {
        Self {
            name,
            total_allocations: AtomicU64::new(0),
            total_deallocations: AtomicU64::new(0),
            active_allocations: AtomicUsize::new(0),
            peak_allocations: AtomicUsize::new(0),
            total_capacity: AtomicUsize::new(0),
            growth_count: AtomicU64::new(0),
            allocation_failures: AtomicU64::new(0),
            bytes_allocated: AtomicUsize::new(0),
            bytes_in_use: AtomicUsize::new(0),
            peak_bytes_in_use: AtomicUsize::new(0),
            alloc_time_us: AtomicU64::new(0),
            dealloc_time_us: AtomicU64::new(0),
            created_at: Instant::now(),
            last_reset: None,
        }
    }

    /// Record an allocation.
    #[inline]
    pub fn record_alloc(&self, bytes: usize, duration: Duration) {
        self.total_allocations.fetch_add(1, Ordering::Relaxed);
        let active = self.active_allocations.fetch_add(1, Ordering::Relaxed) + 1;
        self.bytes_in_use.fetch_add(bytes, Ordering::Relaxed);

        // Update peak
        self.peak_allocations.fetch_max(active, Ordering::Relaxed);
        let bytes = self.bytes_in_use.load(Ordering::Relaxed);
        self.peak_bytes_in_use.fetch_max(bytes, Ordering::Relaxed);

        // Record timing
        self.alloc_time_us
            .fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
    }

    /// Record a deallocation.
    #[inline]
    pub fn record_dealloc(&self, bytes: usize, duration: Duration) {
        self.total_deallocations.fetch_add(1, Ordering::Relaxed);
        self.active_allocations.fetch_sub(1, Ordering::Relaxed);
        self.bytes_in_use.fetch_sub(bytes, Ordering::Relaxed);

        // Record timing
        self.dealloc_time_us
            .fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
    }

    /// Record a pool growth event.
    #[inline]
    pub fn record_growth(&self, new_capacity: usize, bytes_added: usize) {
        self.growth_count.fetch_add(1, Ordering::Relaxed);
        self.total_capacity.store(new_capacity, Ordering::Relaxed);
        self.bytes_allocated
            .fetch_add(bytes_added, Ordering::Relaxed);
    }

    /// Record a pool shrink event.
    ///
    /// Called when the pool releases memory back to the system.
    /// This is the inverse of record_growth.
    #[inline]
    pub fn record_shrink(&self, _slots_freed: usize, bytes_freed: usize) {
        // Note: We don't have a shrink_count counter, but we could add one
        // For now, we just update the bytes_allocated
        self.bytes_allocated
            .fetch_sub(bytes_freed, Ordering::Relaxed);
    }

    /// Record an allocation failure.
    #[inline]
    pub fn record_failure(&self) {
        self.allocation_failures.fetch_add(1, Ordering::Relaxed);
    }

    /// Get a snapshot of current statistics.
    pub fn snapshot(&self) -> AllocationStats {
        AllocationStats {
            name: self.name.clone(),
            total_allocations: self.total_allocations.load(Ordering::Relaxed),
            total_deallocations: self.total_deallocations.load(Ordering::Relaxed),
            active_allocations: self.active_allocations.load(Ordering::Relaxed),
            peak_allocations: self.peak_allocations.load(Ordering::Relaxed),
            total_capacity: self.total_capacity.load(Ordering::Relaxed),
            growth_count: self.growth_count.load(Ordering::Relaxed),
            allocation_failures: self.allocation_failures.load(Ordering::Relaxed),
            bytes_allocated: self.bytes_allocated.load(Ordering::Relaxed),
            bytes_in_use: self.bytes_in_use.load(Ordering::Relaxed),
            peak_bytes_in_use: self.peak_bytes_in_use.load(Ordering::Relaxed),
            avg_alloc_time_us: self.avg_alloc_time_us(),
            avg_dealloc_time_us: self.avg_dealloc_time_us(),
            uptime: self.created_at.elapsed(),
            utilization: self.utilization(),
            fragmentation: self.fragmentation(),
        }
    }

    /// Calculate average allocation time in microseconds.
    fn avg_alloc_time_us(&self) -> f64 {
        let total = self.alloc_time_us.load(Ordering::Relaxed);
        let count = self.total_allocations.load(Ordering::Relaxed);
        if count == 0 {
            0.0
        } else {
            total as f64 / count as f64
        }
    }

    /// Calculate average deallocation time in microseconds.
    fn avg_dealloc_time_us(&self) -> f64 {
        let total = self.dealloc_time_us.load(Ordering::Relaxed);
        let count = self.total_deallocations.load(Ordering::Relaxed);
        if count == 0 {
            0.0
        } else {
            total as f64 / count as f64
        }
    }

    /// Calculate pool utilization (0.0 to 1.0).
    pub fn utilization(&self) -> f64 {
        let capacity = self.total_capacity.load(Ordering::Relaxed);
        if capacity == 0 {
            0.0
        } else {
            let active = self.active_allocations.load(Ordering::Relaxed);
            active as f64 / capacity as f64
        }
    }

    /// Calculate fragmentation ratio (wasted space).
    pub fn fragmentation(&self) -> f64 {
        let allocated = self.bytes_allocated.load(Ordering::Relaxed);
        if allocated == 0 {
            0.0
        } else {
            let in_use = self.bytes_in_use.load(Ordering::Relaxed);
            1.0 - (in_use as f64 / allocated as f64)
        }
    }

    /// Reset statistics (useful for profiling specific scenarios).
    pub fn reset(&mut self) {
        self.total_allocations.store(0, Ordering::Relaxed);
        self.total_deallocations.store(0, Ordering::Relaxed);
        self.peak_allocations.store(
            self.active_allocations.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
        self.peak_bytes_in_use
            .store(self.bytes_in_use.load(Ordering::Relaxed), Ordering::Relaxed);
        self.growth_count.store(0, Ordering::Relaxed);
        self.allocation_failures.store(0, Ordering::Relaxed);
        self.alloc_time_us.store(0, Ordering::Relaxed);
        self.dealloc_time_us.store(0, Ordering::Relaxed);
        self.last_reset = Some(Instant::now());
    }
}

/// Snapshot of pool statistics at a point in time.
#[derive(Debug, Clone)]
pub struct AllocationStats {
    pub name: String,
    pub total_allocations: u64,
    pub total_deallocations: u64,
    pub active_allocations: usize,
    pub peak_allocations: usize,
    pub total_capacity: usize,
    pub growth_count: u64,
    pub allocation_failures: u64,
    pub bytes_allocated: usize,
    pub bytes_in_use: usize,
    pub peak_bytes_in_use: usize,
    pub avg_alloc_time_us: f64,
    pub avg_dealloc_time_us: f64,
    pub uptime: Duration,
    pub utilization: f64,
    pub fragmentation: f64,
}

impl AllocationStats {
    /// Format statistics as a human-readable report.
    pub fn report(&self) -> String {
        format!(
            r#"Pool Statistics: {}
==================================================
Allocations:
  Total:   {}
  Active:  {} / {} ({:.1}% utilization)
  Peak:    {}
  Failures: {}

Memory:
  Allocated: {} bytes ({:.2} MB)
  In Use:    {} bytes ({:.2} MB)
  Peak:      {} bytes ({:.2} MB)
  Fragmentation: {:.1}%

Performance:
  Avg Alloc Time:   {:.2} μs
  Avg Dealloc Time: {:.2} μs
  Growth Events:    {}
  Uptime:           {:.2}s
"#,
            self.name,
            self.total_allocations,
            self.active_allocations,
            self.total_capacity,
            self.utilization * 100.0,
            self.peak_allocations,
            self.allocation_failures,
            self.bytes_allocated,
            self.bytes_allocated as f64 / 1_048_576.0,
            self.bytes_in_use,
            self.bytes_in_use as f64 / 1_048_576.0,
            self.peak_bytes_in_use,
            self.peak_bytes_in_use as f64 / 1_048_576.0,
            self.fragmentation * 100.0,
            self.avg_alloc_time_us,
            self.avg_dealloc_time_us,
            self.growth_count,
            self.uptime.as_secs_f64()
        )
    }

    /// Check if the pool needs optimization.
    pub fn needs_optimization(&self) -> Vec<String> {
        let mut recommendations = Vec::new();

        // High fragmentation
        if self.fragmentation > 0.3 {
            recommendations.push(format!(
                "High fragmentation ({:.1}%) - consider defragmentation or increasing initial capacity",
                self.fragmentation * 100.0
            ));
        }

        // Low utilization
        if self.utilization < 0.2 && self.total_capacity > 100 {
            recommendations.push(format!(
                "Low utilization ({:.1}%) - consider reducing initial capacity",
                self.utilization * 100.0
            ));
        }

        // High growth count
        if self.growth_count > 10 {
            recommendations.push(format!(
                "Frequent growth ({} events) - increase initial capacity to avoid reallocation",
                self.growth_count
            ));
        }

        // Allocation failures
        if self.allocation_failures > 0 {
            recommendations.push(format!(
                "{} allocation failures - increase max capacity or investigate leaks",
                self.allocation_failures
            ));
        }

        // High peak usage
        if self.peak_allocations as f64 / self.total_capacity as f64 > 0.9 {
            recommendations
                .push("Peak usage exceeds 90% - consider increasing capacity headroom".to_string());
        }

        recommendations
    }
}

/// Aggregate memory statistics across all pools.
#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub total_pools: usize,
    pub total_allocations: u64,
    pub total_bytes_allocated: usize,
    pub total_bytes_in_use: usize,
    pub overall_utilization: f64,
    pub pools: Vec<AllocationStats>,
}

impl MemoryStats {
    /// Generate a comprehensive report.
    pub fn report(&self) -> String {
        let mut report = format!(
            r#"=== Global Memory Pool Statistics ===
Total Pools: {}
Total Allocations: {}
Total Bytes Allocated: {} ({:.2} MB)
Total Bytes In Use: {} ({:.2} MB)
Overall Utilization: {:.1}%

=== Per-Pool Statistics ===
"#,
            self.total_pools,
            self.total_allocations,
            self.total_bytes_allocated,
            self.total_bytes_allocated as f64 / 1_048_576.0,
            self.total_bytes_in_use,
            self.total_bytes_in_use as f64 / 1_048_576.0,
            self.overall_utilization * 100.0
        );

        for pool_stats in &self.pools {
            report.push_str(&pool_stats.report());
            report.push_str("\n");
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stats_creation() {
        let stats = PoolStats::new("TestPool".to_string());
        assert_eq!(stats.name, "TestPool");
        assert_eq!(stats.total_allocations.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_record_alloc() {
        let stats = PoolStats::new("Test".to_string());
        stats.record_alloc(64, Duration::from_micros(10));

        assert_eq!(stats.total_allocations.load(Ordering::Relaxed), 1);
        assert_eq!(stats.active_allocations.load(Ordering::Relaxed), 1);
        assert_eq!(stats.bytes_in_use.load(Ordering::Relaxed), 64);
    }

    #[test]
    fn test_utilization() {
        let stats = PoolStats::new("Test".to_string());
        stats.total_capacity.store(100, Ordering::Relaxed);
        stats.active_allocations.store(50, Ordering::Relaxed);

        assert_eq!(stats.utilization(), 0.5);
    }

    #[test]
    fn test_snapshot() {
        let stats = PoolStats::new("Test".to_string());
        stats.record_alloc(128, Duration::from_micros(5));
        stats.record_alloc(256, Duration::from_micros(7));

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.total_allocations, 2);
        assert_eq!(snapshot.active_allocations, 2);
        assert_eq!(snapshot.bytes_in_use, 384);
    }
}
