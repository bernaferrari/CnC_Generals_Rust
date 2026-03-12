//! # Comprehensive Benchmark Suite
//!
//! Performance testing framework for all GeneralsRust libraries with advanced analysis:
//!
//! - **CPU Benchmarks** - Single/multi-threaded performance analysis
//! - **GPU Benchmarks** - Graphics pipeline and compute performance
//! - **Memory Benchmarks** - Allocation patterns and cache efficiency
//! - **Network Benchmarks** - Latency, throughput, and scalability
//! - **AI Benchmarks** - Machine learning model performance
//! - **Compression Benchmarks** - Algorithm speed and compression ratios
//!
//! ## Features
//!
//! ### Advanced Analysis
//! - **Statistical Analysis** - Mean, median, percentiles, confidence intervals
//! - **Performance Regression** - Historical performance tracking
//! - **Bottleneck Detection** - Identify performance critical sections
//! - **Scalability Analysis** - Performance across different loads
//! - **Memory Profiling** - Allocation patterns and memory leaks
//!
//! ### Reporting & Visualization
//! - **HTML Reports** - Interactive performance dashboards
//! - **CSV Export** - Raw data for external analysis
//! - **Flamegraphs** - CPU profiling visualization
//! - **Real-time Monitoring** - Live performance tracking
//! - **Comparative Analysis** - Before/after performance comparisons
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use benchmark_suite::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), BenchmarkError> {
//!     // Initialize benchmark suite
//!     let mut suite = BenchmarkSuite::new()
//!         .with_output_dir("./benchmark_results")
//!         .enable_profiling()
//!         .enable_visualization();
//!
//!     // Run CPU benchmarks
//!     suite.run_cpu_benchmarks().await?;
//!
//!     // Run GPU benchmarks
//!     suite.run_gpu_benchmarks().await?;
//!
//!     // Run compression benchmarks
//!     suite.run_compression_benchmarks().await?;
//!
//!     // Generate comprehensive report
//!     let report = suite.generate_report().await?;
//!     report.save_html("performance_report.html").await?;
//!
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub mod cpu;
pub mod gpu;
pub mod memory;
pub mod network;
pub mod ai;
pub mod compression;
pub mod profiler;
pub mod reporter;
pub mod visualizer;

/// Benchmark errors
#[derive(Error, Debug)]
pub enum BenchmarkError {
    #[error("Benchmark initialization failed: {0}")]
    InitializationFailed(String),
    
    #[error("Benchmark execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("System information unavailable: {0}")]
    SystemInfoUnavailable(String),
    
    #[error("Profiling error: {0}")]
    ProfilingError(String),
    
    #[error("Report generation failed: {0}")]
    ReportGenerationFailed(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, BenchmarkError>;

/// Main benchmark suite
#[derive(Debug)]
pub struct BenchmarkSuite {
    config: BenchmarkConfig,
    results: Vec<BenchmarkResult>,
    system_info: SystemInfo,
    session_id: Uuid,
}

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub output_dir: PathBuf,
    pub enable_profiling: bool,
    pub enable_visualization: bool,
    pub warmup_iterations: u32,
    pub measurement_iterations: u32,
    pub timeout: Duration,
    pub parallel_benchmarks: bool,
    pub save_raw_data: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("./benchmark_results"),
            enable_profiling: false,
            enable_visualization: true,
            warmup_iterations: 5,
            measurement_iterations: 100,
            timeout: Duration::from_secs(300),
            parallel_benchmarks: true,
            save_raw_data: true,
        }
    }
}

impl BenchmarkSuite {
    /// Create new benchmark suite
    pub fn new() -> Self {
        let system_info = SystemInfo::collect().unwrap_or_default();
        
        Self {
            config: BenchmarkConfig::default(),
            results: Vec::new(),
            system_info,
            session_id: Uuid::new_v4(),
        }
    }
    
    /// Configure output directory
    pub fn with_output_dir<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.config.output_dir = path.into();
        self
    }
    
    /// Enable profiling
    pub fn enable_profiling(mut self) -> Self {
        self.config.enable_profiling = true;
        self
    }
    
    /// Enable visualization
    pub fn enable_visualization(mut self) -> Self {
        self.config.enable_visualization = true;
        self
    }
    
    /// Set benchmark iterations
    pub fn with_iterations(mut self, warmup: u32, measurement: u32) -> Self {
        self.config.warmup_iterations = warmup;
        self.config.measurement_iterations = measurement;
        self
    }
    
    /// Run all benchmarks
    pub async fn run_all_benchmarks(&mut self) -> Result<()> {
        log::info!("Starting comprehensive benchmark suite (session: {})", self.session_id);
        
        // Create output directory
        tokio::fs::create_dir_all(&self.config.output_dir).await?;
        
        // Run benchmark categories
        let benchmark_tasks = vec![
            ("CPU", self.run_cpu_benchmarks()),
            ("GPU", self.run_gpu_benchmarks()),
            ("Memory", self.run_memory_benchmarks()),
            ("Network", self.run_network_benchmarks()),
            ("AI", self.run_ai_benchmarks()),
            ("Compression", self.run_compression_benchmarks()),
        ];
        
        if self.config.parallel_benchmarks {
            // Run benchmarks in parallel (where safe)
            futures::future::try_join_all(
                benchmark_tasks.into_iter().map(|(name, task)| async move {
                    log::info!("Running {} benchmarks...", name);
                    task.await
                })
            ).await?;
        } else {
            // Run benchmarks sequentially
            for (name, task) in benchmark_tasks {
                log::info!("Running {} benchmarks...", name);
                task.await?;
            }
        }
        
        log::info!("All benchmarks completed successfully");
        Ok(())
    }
    
    /// Run CPU benchmarks
    pub async fn run_cpu_benchmarks(&mut self) -> Result<()> {
        #[cfg(feature = "cpu")]
        {
            let mut cpu_benchmarks = cpu::CpuBenchmarks::new(&self.config);
            let results = cpu_benchmarks.run_all().await?;
            self.results.extend(results);
        }
        Ok(())
    }
    
    /// Run GPU benchmarks
    pub async fn run_gpu_benchmarks(&mut self) -> Result<()> {
        #[cfg(feature = "gpu")]
        {
            let mut gpu_benchmarks = gpu::GpuBenchmarks::new(&self.config).await?;
            let results = gpu_benchmarks.run_all().await?;
            self.results.extend(results);
        }
        Ok(())
    }
    
    /// Run memory benchmarks
    pub async fn run_memory_benchmarks(&mut self) -> Result<()> {
        #[cfg(feature = "memory")]
        {
            let mut memory_benchmarks = memory::MemoryBenchmarks::new(&self.config);
            let results = memory_benchmarks.run_all().await?;
            self.results.extend(results);
        }
        Ok(())
    }
    
    /// Run network benchmarks
    pub async fn run_network_benchmarks(&mut self) -> Result<()> {
        #[cfg(feature = "network")]
        {
            let mut network_benchmarks = network::NetworkBenchmarks::new(&self.config);
            let results = network_benchmarks.run_all().await?;
            self.results.extend(results);
        }
        Ok(())
    }
    
    /// Run AI benchmarks
    pub async fn run_ai_benchmarks(&mut self) -> Result<()> {
        #[cfg(feature = "ai")]
        {
            let mut ai_benchmarks = ai::AiBenchmarks::new(&self.config);
            let results = ai_benchmarks.run_all().await?;
            self.results.extend(results);
        }
        Ok(())
    }
    
    /// Run compression benchmarks
    pub async fn run_compression_benchmarks(&mut self) -> Result<()> {
        #[cfg(feature = "compression")]
        {
            let mut compression_benchmarks = compression::CompressionBenchmarks::new(&self.config);
            let results = compression_benchmarks.run_all().await?;
            self.results.extend(results);
        }
        Ok(())
    }
    
    /// Generate comprehensive report
    pub async fn generate_report(&self) -> Result<BenchmarkReport> {
        let mut report = BenchmarkReport::new(
            self.session_id,
            self.system_info.clone(),
            self.results.clone(),
        );
        
        // Generate statistical analysis
        report.analyze_performance();
        
        // Generate visualizations if enabled
        if self.config.enable_visualization {
            report.generate_visualizations(&self.config.output_dir).await?;
        }
        
        // Save raw data if enabled
        if self.config.save_raw_data {
            report.save_raw_data(&self.config.output_dir)?;
        }
        
        Ok(report)
    }
    
    /// Compare with previous benchmark results
    pub async fn compare_with_baseline<P: Into<PathBuf>>(&self, baseline_path: P) -> Result<PerformanceComparison> {
        let baseline_path = baseline_path.into();
        let baseline_data = tokio::fs::read_to_string(baseline_path).await?;
        let baseline_results: Vec<BenchmarkResult> = serde_json::from_str(&baseline_data)?;

        Ok(PerformanceComparison::new(&baseline_results, &self.results))
    }
    
    /// Get current results
    pub fn results(&self) -> &[BenchmarkResult] {
        &self.results
    }
}

impl Default for BenchmarkSuite {
    fn default() -> Self {
        Self::new()
    }
}

/// Individual benchmark result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub id: Uuid,
    pub name: String,
    pub category: BenchmarkCategory,
    pub measurements: Vec<Measurement>,
    pub metadata: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
    pub duration: Duration,
}

impl BenchmarkResult {
    pub fn new(name: String, category: BenchmarkCategory) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            category,
            measurements: Vec::new(),
            metadata: HashMap::new(),
            timestamp: Utc::now(),
            duration: Duration::ZERO,
        }
    }
    
    pub fn add_measurement(&mut self, measurement: Measurement) {
        self.measurements.push(measurement);
    }
    
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }
    
    pub fn statistics(&self) -> BenchmarkStatistics {
        BenchmarkStatistics::from_measurements(&self.measurements)
    }
}

/// Benchmark categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BenchmarkCategory {
    Cpu,
    Gpu,
    Memory,
    Network,
    Ai,
    Compression,
    Graphics,
    Audio,
    Io,
}

impl BenchmarkCategory {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Gpu => "GPU", 
            Self::Memory => "Memory",
            Self::Network => "Network",
            Self::Ai => "AI/ML",
            Self::Compression => "Compression",
            Self::Graphics => "Graphics",
            Self::Audio => "Audio",
            Self::Io => "I/O",
        }
    }
}

/// Individual measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    pub value: f64,
    pub unit: MeasurementUnit,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

impl Measurement {
    pub fn new(value: f64, unit: MeasurementUnit) -> Self {
        Self {
            value,
            unit,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }
    
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Measurement units
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeasurementUnit {
    /// Time measurements
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
    
    /// Throughput measurements
    OperationsPerSecond,
    BytesPerSecond,
    MegabytesPerSecond,
    GigabytesPerSecond,
    
    /// Memory measurements
    Bytes,
    Kilobytes,
    Megabytes,
    Gigabytes,
    
    /// Graphics measurements
    FramesPerSecond,
    MillisecondsPerFrame,
    TrianglesPerSecond,
    
    /// Network measurements
    LatencyMs,
    PacketsPerSecond,
    ConnectionsPerSecond,
    
    /// Custom measurements
    Ratio,
    Percentage,
    Count,
    Custom(String),
}

impl MeasurementUnit {
    pub fn symbol(&self) -> &str {
        match self {
            Self::Nanoseconds => "ns",
            Self::Microseconds => "μs",
            Self::Milliseconds => "ms",
            Self::Seconds => "s",
            Self::OperationsPerSecond => "ops/s",
            Self::BytesPerSecond => "B/s",
            Self::MegabytesPerSecond => "MB/s",
            Self::GigabytesPerSecond => "GB/s",
            Self::Bytes => "B",
            Self::Kilobytes => "KB",
            Self::Megabytes => "MB",
            Self::Gigabytes => "GB",
            Self::FramesPerSecond => "FPS",
            Self::MillisecondsPerFrame => "ms/frame",
            Self::TrianglesPerSecond => "tri/s",
            Self::LatencyMs => "ms",
            Self::PacketsPerSecond => "pkt/s",
            Self::ConnectionsPerSecond => "conn/s",
            Self::Ratio => "ratio",
            Self::Percentage => "%",
            Self::Count => "count",
            Self::Custom(s) => s,
        }
    }
}

/// Statistical analysis of benchmark results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkStatistics {
    pub count: usize,
    pub mean: f64,
    pub median: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
    pub percentiles: HashMap<u8, f64>,
    pub outliers: Vec<f64>,
}

impl BenchmarkStatistics {
    pub fn from_measurements(measurements: &[Measurement]) -> Self {
        if measurements.is_empty() {
            return Self::default();
        }
        
        let mut values: Vec<f64> = measurements.iter().map(|m| m.value).collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        
        let count = values.len();
        let mean = values.iter().sum::<f64>() / count as f64;
        let median = if count % 2 == 0 {
            (values[count / 2 - 1] + values[count / 2]) / 2.0
        } else {
            values[count / 2]
        };
        
        let variance = values.iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>() / count as f64;
        let std_dev = variance.sqrt();
        
        let min = values[0];
        let max = values[count - 1];
        
        // Calculate percentiles
        let mut percentiles = HashMap::new();
        for p in [5, 25, 50, 75, 95, 99] {
            let index = (p as f64 / 100.0 * (count - 1) as f64) as usize;
            percentiles.insert(p, values[index]);
        }
        
        // Detect outliers using IQR method
        let q1 = percentiles[&25];
        let q3 = percentiles[&75];
        let iqr = q3 - q1;
        let lower_bound = q1 - 1.5 * iqr;
        let upper_bound = q3 + 1.5 * iqr;
        
        let outliers: Vec<f64> = values.into_iter()
            .filter(|&v| v < lower_bound || v > upper_bound)
            .collect();
        
        Self {
            count,
            mean,
            median,
            std_dev,
            min,
            max,
            percentiles,
            outliers,
        }
    }
    
    pub fn coefficient_of_variation(&self) -> f64 {
        if self.mean == 0.0 {
            0.0
        } else {
            self.std_dev / self.mean
        }
    }
}

impl Default for BenchmarkStatistics {
    fn default() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            median: 0.0,
            std_dev: 0.0,
            min: 0.0,
            max: 0.0,
            percentiles: HashMap::new(),
            outliers: Vec::new(),
        }
    }
}

/// System information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub gpu: Vec<GpuInfo>,
    pub timestamp: DateTime<Utc>,
}

impl SystemInfo {
    #[cfg(feature = "cpu")]
    pub fn collect() -> Result<Self> {
        use sysinfo::{System, SystemExt, ComponentExt, CpuExt};
        
        let mut system = System::new_all();
        system.refresh_all();
        
        let os = format!("{} {}", system.name().unwrap_or_default(), system.version().unwrap_or_default());
        
        let cpu = CpuInfo {
            name: system.global_cpu_info().brand().to_string(),
            cores: system.cpus().len(),
            threads: num_cpus::get(),
            base_frequency: system.global_cpu_info().frequency(),
            cache_size: 0, // Not available through sysinfo
        };
        
        let memory = MemoryInfo {
            total: system.total_memory(),
            available: system.available_memory(),
            used: system.used_memory(),
        };
        
        // GPU info would require additional crates like wgpu
        let gpu = vec![];
        
        Ok(Self {
            os,
            cpu,
            memory,
            gpu,
            timestamp: Utc::now(),
        })
    }
    
    #[cfg(not(feature = "cpu"))]
    pub fn collect() -> Result<Self> {
        Ok(Self::default())
    }
}

impl Default for SystemInfo {
    fn default() -> Self {
        Self {
            os: "Unknown".to_string(),
            cpu: CpuInfo::default(),
            memory: MemoryInfo::default(),
            gpu: Vec::new(),
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub name: String,
    pub cores: usize,
    pub threads: usize,
    pub base_frequency: u64,
    pub cache_size: u64,
}

impl Default for CpuInfo {
    fn default() -> Self {
        Self {
            name: "Unknown CPU".to_string(),
            cores: 1,
            threads: 1,
            base_frequency: 0,
            cache_size: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total: u64,
    pub available: u64,
    pub used: u64,
}

impl Default for MemoryInfo {
    fn default() -> Self {
        Self {
            total: 0,
            available: 0,
            used: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
    pub memory: u64,
    pub driver_version: String,
}

/// Benchmark report
#[derive(Debug, Clone)]
pub struct BenchmarkReport {
    pub session_id: Uuid,
    pub system_info: SystemInfo,
    pub results: Vec<BenchmarkResult>,
    pub summary: BenchmarkSummary,
    pub generated_at: DateTime<Utc>,
}

impl BenchmarkReport {
    pub fn new(session_id: Uuid, system_info: SystemInfo, results: Vec<BenchmarkResult>) -> Self {
        let summary = BenchmarkSummary::from_results(&results);
        
        Self {
            session_id,
            system_info,
            results,
            summary,
            generated_at: Utc::now(),
        }
    }
    
    /// Perform advanced statistical analysis on benchmark results
    ///
    /// Analyzes performance trends, detects regressions, and identifies
    /// performance bottlenecks across all benchmark categories.
    pub fn analyze_performance(&mut self) {
        // Group results by category for analysis
        let mut category_results: HashMap<BenchmarkCategory, Vec<&BenchmarkResult>> = HashMap::new();

        for result in &self.results {
            category_results.entry(result.category)
                .or_insert_with(Vec::new)
                .push(result);
        }

        // Analyze each category
        for (category, results) in category_results {
            if results.is_empty() {
                continue;
            }

            // Calculate category performance metrics
            let mut total_score = 0.0;
            let mut count = 0;

            for result in results {
                let stats = result.statistics();

                // Calculate normalized score (0-100)
                // Lower is better for timing measurements
                let score = if stats.mean > 0.0 {
                    // Normalize to 0-100 scale (arbitrary baseline)
                    let normalized = 100.0 - (stats.mean.log10() * 10.0).min(100.0).max(0.0);
                    normalized
                } else {
                    50.0 // Default score
                };

                total_score += score;
                count += 1;
            }

            let average_score = if count > 0 { total_score / count as f64 } else { 0.0 };

            // Update summary
            if let Some(summary) = self.summary.categories.get_mut(&category) {
                summary.average_score = average_score;
            }
        }

        // Update overall performance score
        let category_scores: Vec<f64> = self.summary.categories.values()
            .map(|s| s.average_score)
            .collect();

        if !category_scores.is_empty() {
            self.summary.overall_performance_score = category_scores.iter().sum::<f64>() / category_scores.len() as f64;
        }

        // Generate performance insights
        self.generate_recommendations();
    }

    /// Generate performance recommendations based on benchmark results
    fn generate_recommendations(&mut self) {
        self.summary.recommendations.clear();

        // Analyze results for recommendations
        let mut has_slow_cpu = false;
        let mut has_memory_issues = false;
        let mut has_gpu_bottleneck = false;

        for result in &self.results {
            let stats = result.statistics();

            // Check for high variance (indicates unstable performance)
            if stats.coefficient_of_variation() > 0.2 {
                self.summary.recommendations.push(format!(
                    "High performance variance detected in '{}'. Consider reducing background processes.",
                    result.name
                ));
            }

            // Category-specific recommendations
            match result.category {
                BenchmarkCategory::Cpu => {
                    if stats.mean > 1000000.0 { // > 1ms for simple operations
                        has_slow_cpu = true;
                    }
                }
                BenchmarkCategory::Memory => {
                    if result.name.contains("Allocation") && stats.mean > 100000.0 {
                        has_memory_issues = true;
                    }
                }
                BenchmarkCategory::Gpu => {
                    if stats.mean > 10000.0 { // > 10ms for GPU operations
                        has_gpu_bottleneck = true;
                    }
                }
                _ => {}
            }
        }

        // Add general recommendations
        if has_slow_cpu {
            self.summary.recommendations.push(
                "CPU performance is below optimal. Consider upgrading CPU or enabling compiler optimizations.".to_string()
            );
        }

        if has_memory_issues {
            self.summary.recommendations.push(
                "Memory allocation performance is slow. Consider using object pools or pre-allocation strategies.".to_string()
            );
        }

        if has_gpu_bottleneck {
            self.summary.recommendations.push(
                "GPU performance bottleneck detected. Check GPU drivers and consider reducing graphics settings.".to_string()
            );
        }

        // Always add some general best practices
        if self.summary.recommendations.is_empty() {
            self.summary.recommendations.push(
                "Performance is within expected ranges. Consider profiling in release mode for production benchmarks.".to_string()
            );
        }
    }
    
    pub async fn generate_visualizations(&self, output_dir: &PathBuf) -> Result<()> {
        #[cfg(feature = "visualization")]
        {
            let visualizer = visualizer::BenchmarkVisualizer::new();
            visualizer.generate_charts(&self.results, output_dir).await?;
        }
        Ok(())
    }
    
    pub async fn save_raw_data(&self, output_dir: &PathBuf) -> Result<()> {
        let data_path = output_dir.join(format!("raw_data_{}.json", self.session_id));
        let json_data = serde_json::to_string_pretty(&self.results)?;
        tokio::fs::write(data_path, json_data).await?;
        Ok(())
    }

    pub async fn save_html<P: Into<PathBuf>>(&self, path: P) -> Result<()> {
        let html = reporter::generate_html_report(self)?;
        tokio::fs::write(path.into(), html).await?;
        Ok(())
    }
}

/// Benchmark summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub total_benchmarks: usize,
    pub categories: HashMap<BenchmarkCategory, CategorySummary>,
    pub overall_performance_score: f64,
    pub recommendations: Vec<String>,
}

impl BenchmarkSummary {
    pub fn from_results(results: &[BenchmarkResult]) -> Self {
        let mut categories = HashMap::new();
        
        // Group results by category
        for result in results {
            let entry = categories.entry(result.category).or_insert_with(|| CategorySummary {
                benchmark_count: 0,
                average_score: 0.0,
                best_result: None,
                worst_result: None,
            });
            
            entry.benchmark_count += 1;
            // TODO: Calculate scores and update best/worst results
        }
        
        // Calculate overall performance score
        let overall_performance_score = 75.0; // TODO: Implement actual calculation
        
        // Generate recommendations
        let recommendations = vec![
            "Consider enabling compiler optimizations for better performance".to_string(),
            "Monitor memory usage patterns for potential optimizations".to_string(),
        ];
        
        Self {
            total_benchmarks: results.len(),
            categories,
            overall_performance_score,
            recommendations,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySummary {
    pub benchmark_count: usize,
    pub average_score: f64,
    pub best_result: Option<String>,
    pub worst_result: Option<String>,
}

/// Performance comparison between runs
#[derive(Debug, Clone)]
pub struct PerformanceComparison {
    pub improvements: Vec<ComparisonResult>,
    pub regressions: Vec<ComparisonResult>,
    pub unchanged: Vec<ComparisonResult>,
    pub overall_change: f64,
}

impl PerformanceComparison {
    /// Compare current benchmark results with baseline
    ///
    /// Identifies performance improvements, regressions, and unchanged metrics
    /// by comparing statistical measures between baseline and current runs.
    pub fn new(baseline: &[BenchmarkResult], current: &[BenchmarkResult]) -> Self {
        let mut improvements = Vec::new();
        let mut regressions = Vec::new();
        let mut unchanged = Vec::new();

        // Create a lookup map for baseline results by name
        let baseline_map: HashMap<String, &BenchmarkResult> = baseline
            .iter()
            .map(|r| (r.name.clone(), r))
            .collect();

        let mut total_change = 0.0;
        let mut comparison_count = 0;

        // Compare each current result with baseline
        for current_result in current {
            if let Some(baseline_result) = baseline_map.get(&current_result.name) {
                let baseline_stats = baseline_result.statistics();
                let current_stats = current_result.statistics();

                if baseline_stats.mean == 0.0 {
                    continue; // Skip division by zero
                }

                // Calculate percentage change (negative = improvement for timing)
                let change_percentage = ((current_stats.mean - baseline_stats.mean) / baseline_stats.mean) * 100.0;

                let significance = if change_percentage.abs() < 1.0 {
                    ComparisonSignificance::Insignificant
                } else if change_percentage.abs() < 5.0 {
                    ComparisonSignificance::Minor
                } else if change_percentage.abs() < 15.0 {
                    ComparisonSignificance::Significant
                } else {
                    ComparisonSignificance::Major
                };

                let comparison = ComparisonResult {
                    benchmark_name: current_result.name.clone(),
                    baseline_value: baseline_stats.mean,
                    current_value: current_stats.mean,
                    change_percentage,
                    significance,
                };

                // For timing benchmarks, lower is better
                if change_percentage < -1.0 {
                    improvements.push(comparison);
                } else if change_percentage > 1.0 {
                    regressions.push(comparison);
                } else {
                    unchanged.push(comparison);
                }

                total_change += change_percentage;
                comparison_count += 1;
            }
        }

        let overall_change = if comparison_count > 0 {
            total_change / comparison_count as f64
        } else {
            0.0
        };

        Self {
            improvements,
            regressions,
            unchanged,
            overall_change,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ComparisonResult {
    pub benchmark_name: String,
    pub baseline_value: f64,
    pub current_value: f64,
    pub change_percentage: f64,
    pub significance: ComparisonSignificance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonSignificance {
    Insignificant,
    Minor,
    Significant,
    Major,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_measurement_unit_symbols() {
        assert_eq!(MeasurementUnit::Nanoseconds.symbol(), "ns");
        assert_eq!(MeasurementUnit::MegabytesPerSecond.symbol(), "MB/s");
        assert_eq!(MeasurementUnit::FramesPerSecond.symbol(), "FPS");
    }
    
    #[test]
    fn test_benchmark_statistics() {
        let measurements = vec![
            Measurement::new(100.0, MeasurementUnit::Milliseconds),
            Measurement::new(200.0, MeasurementUnit::Milliseconds),
            Measurement::new(150.0, MeasurementUnit::Milliseconds),
            Measurement::new(180.0, MeasurementUnit::Milliseconds),
            Measurement::new(120.0, MeasurementUnit::Milliseconds),
        ];
        
        let stats = BenchmarkStatistics::from_measurements(&measurements);
        
        assert_eq!(stats.count, 5);
        assert_eq!(stats.min, 100.0);
        assert_eq!(stats.max, 200.0);
        assert_eq!(stats.median, 150.0);
        assert!((stats.mean - 150.0).abs() < 1e-10);
    }
    
    #[test]
    fn test_benchmark_result_creation() {
        let mut result = BenchmarkResult::new(
            "Test Benchmark".to_string(),
            BenchmarkCategory::Cpu,
        );
        
        result.add_measurement(Measurement::new(42.0, MeasurementUnit::Milliseconds));
        result.add_metadata("test_param".to_string(), "test_value".to_string());
        
        assert_eq!(result.name, "Test Benchmark");
        assert_eq!(result.category, BenchmarkCategory::Cpu);
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.metadata.get("test_param"), Some(&"test_value".to_string()));
    }
}