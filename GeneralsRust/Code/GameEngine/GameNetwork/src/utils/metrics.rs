//! Network metrics collection

/// Metrics collector
pub struct MetricsCollector;

impl MetricsCollector {
    /// Create new collector
    pub fn new() -> Self {
        Self
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}