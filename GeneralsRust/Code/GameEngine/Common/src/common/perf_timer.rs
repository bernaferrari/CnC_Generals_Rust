////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// PerfTimer.rs ////////////////////////////////////////////////////////////////////////////////////
// John McDonald
// July 2002

#[cfg(feature = "perf_timers")]
use parking_lot::Mutex as PerfMutex;
use std::collections::HashMap;
#[cfg(feature = "perf_timers")]
use std::fs::File;
#[cfg(feature = "perf_timers")]
use std::io::Write;
use std::sync::Mutex;
#[cfg(feature = "perf_timers")]
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
#[cfg(feature = "perf_timers")]
use tracing::span::EnteredSpan;

use crate::common::time;
#[cfg(feature = "perf_timers")]
use tracing::{event, Level};

/// Performance timer configuration
#[cfg(any(feature = "debug", feature = "internal"))]
pub const PERF_TIMERS_ENABLED: bool = !cfg!(feature = "no_perf_timers");
#[cfg(not(any(feature = "debug", feature = "internal")))]
pub const PERF_TIMERS_ENABLED: bool = false;

pub const PERFMETRICS_BETWEEN_METRICS: u32 = 30; // Frames between metrics updates

/// High precision timer type
pub type PrecisionTime = u64;

/// Precision timer utilities
pub struct PrecisionTimer;

impl PrecisionTimer {
    /// Initialize precision timer (placeholder)
    pub fn init() {
        // In real implementation, would calibrate timer frequency
    }

    /// Get high precision time
    pub fn get_time() -> PrecisionTime {
        // Use system time in nanoseconds as high precision timer
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as PrecisionTime
    }

    /// Get ticks per second (nanoseconds)
    pub fn get_ticks_per_sec() -> PrecisionTime {
        1_000_000_000 // 1 billion nanoseconds per second
    }

    /// Get ticks per millisecond
    pub fn get_ticks_per_msec() -> f64 {
        1_000_000.0 // 1 million nanoseconds per millisecond
    }

    /// Get ticks per microsecond
    pub fn get_ticks_per_usec() -> f64 {
        1_000.0 // 1 thousand nanoseconds per microsecond
    }
}

#[cfg(feature = "perf_timers")]
#[derive(Default)]
struct PerfGatherState {
    running_time_gross: PrecisionTime,
    running_time_net: PrecisionTime,
    call_count: u32,
}

#[cfg(feature = "perf_timers")]
#[derive(Clone, Debug)]
struct PerfSample {
    elapsed_ns: PrecisionTime,
    running_time_gross: PrecisionTime,
    running_time_net: PrecisionTime,
    call_count: u32,
    ignored: bool,
}

#[cfg(feature = "perf_timers")]
#[derive(Default, Clone, Debug)]
struct PerfSnapshot {
    identifier: Arc<str>,
    running_time_gross: PrecisionTime,
    running_time_net: PrecisionTime,
    call_count: u32,
}

#[cfg(feature = "perf_timers")]
/// Performance data gathering
pub struct PerfGather {
    identifier: Arc<str>,
    net_time_only: bool,
    state: PerfMutex<PerfGatherState>,
}

#[cfg(feature = "perf_timers")]
impl PerfGather {
    pub const PERF_GROSSTIME: u32 = 0x01;
    pub const PERF_NETTIME: u32 = 0x02;
    pub const PERF_CALLCOUNT: u32 = 0x04;

    /// Create a new performance gatherer backed by an `Arc`
    pub fn new(identifier: &str, net_only: bool) -> Arc<Self> {
        assert!(
            !identifier.contains(','),
            "PerfGather names must not contain commas"
        );

        Arc::new(Self {
            identifier: Arc::<str>::from(identifier),
            net_time_only: net_only,
            state: PerfMutex::new(PerfGatherState::default()),
        })
    }

    /// Get identifier
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    fn start_sample(&self) -> PrecisionTime {
        PrecisionTimer::get_time()
    }

    fn finalize_sample(&self, start_time: PrecisionTime, ignored: bool) -> PerfSample {
        let end_time = PrecisionTimer::get_time();
        let elapsed = end_time.saturating_sub(start_time);

        let mut state = self.state.lock();
        let mut sample = PerfSample {
            elapsed_ns: elapsed,
            running_time_gross: state.running_time_gross,
            running_time_net: state.running_time_net,
            call_count: state.call_count,
            ignored,
        };

        if !ignored {
            state.running_time_gross = state.running_time_gross.saturating_add(elapsed);
            state.running_time_net = state.running_time_net.saturating_add(elapsed);
            state.call_count = state.call_count.saturating_add(1);

            sample.running_time_gross = state.running_time_gross;
            sample.running_time_net = state.running_time_net;
            sample.call_count = state.call_count;
        }

        sample
    }

    fn emit_tracing_sample(&self, sample: &PerfSample) {
        let elapsed_ms = sample.elapsed_ns as f64 / 1_000_000.0;
        event!(
            target: "perf_timers",
            Level::TRACE,
            timer = %self.identifier,
            elapsed_ns = sample.elapsed_ns,
            elapsed_ms,
            total_gross_ns = sample.running_time_gross,
            total_net_ns = sample.running_time_net,
            call_count = sample.call_count,
            ignored = sample.ignored,
            "perf_timer_sample"
        );
    }

    fn snapshot(&self) -> PerfSnapshot {
        let state = self.state.lock();
        PerfSnapshot {
            identifier: self.identifier.clone(),
            running_time_gross: state.running_time_gross,
            running_time_net: state.running_time_net,
            call_count: state.call_count,
        }
    }

    fn reset(&self) {
        let mut state = self.state.lock();
        *state = PerfGatherState::default();
    }

    /// Reset all registered PerfGather instances
    pub fn reset_all() {
        perf_registry().reset_all();
    }

    /// Dump all performance data into the tracing pipeline (and stdout for legacy parity)
    pub fn dump_all(frame: u32) {
        perf_registry().export_to_tracing(Some(frame));
    }

    /// Display the current performance graph snapshot via tracing events
    pub fn display_graph(frame: u32) {
        perf_registry().export_to_tracing(Some(frame));
    }
}

#[cfg(feature = "perf_timers")]
/// Automatic performance gathering RAII guard
pub struct AutoPerfGather {
    gather: Arc<PerfGather>,
    start_time: PrecisionTime,
    ignored: bool,
    span: Option<EnteredSpan>,
}

#[cfg(feature = "perf_timers")]
impl AutoPerfGather {
    pub fn new(gather: Arc<PerfGather>, ignored: bool) -> Self {
        let span = tracing::trace_span!("perf_timer", timer = %gather.identifier());
        let start_time = gather.start_sample();

        Self {
            gather,
            start_time,
            ignored,
            span: Some(span.entered()),
        }
    }
}

#[cfg(feature = "perf_timers")]
impl Drop for AutoPerfGather {
    fn drop(&mut self) {
        let sample = self.gather.finalize_sample(self.start_time, self.ignored);
        self.gather.emit_tracing_sample(&sample);

        if let Some(span) = self.span.take() {
            drop(span);
        }
    }
}

#[cfg(feature = "perf_timers")]
#[derive(Default)]
struct PerfGatherRegistry {
    timers: PerfMutex<Vec<Arc<PerfGather>>>,
}

#[cfg(feature = "perf_timers")]
impl PerfGatherRegistry {
    fn register(&self, gather: Arc<PerfGather>) -> Arc<PerfGather> {
        let mut timers = self.timers.lock();
        if timers
            .iter()
            .any(|existing| existing.identifier().eq(gather.identifier()))
        {
            return gather;
        }

        timers.push(gather.clone());
        gather
    }

    fn reset_all(&self) {
        for timer in self.timers.lock().iter() {
            timer.reset();
        }
        event!(target: "perf_timers", Level::DEBUG, "reset_all_perf_timers");
    }

    fn snapshot(&self) -> Vec<Arc<PerfGather>> {
        self.timers.lock().clone()
    }

    fn export_to_tracing(&self, frame: Option<u32>) {
        let timers = self.snapshot();
        if let Some(frame) = frame {
            println!("PerfGather::dump_all frame {}", frame);
            for timer in timers {
                let snapshot = timer.snapshot();
                let total_ms = snapshot.running_time_net as f64 / 1_000_000.0;
                event!(
                    target: "perf_timers",
                    Level::DEBUG,
                    frame,
                    timer = %snapshot.identifier,
                    total_gross_ns = snapshot.running_time_gross,
                    total_net_ns = snapshot.running_time_net,
                    total_net_ms = total_ms,
                    call_count = snapshot.call_count,
                    "perf_timer_totals"
                );
            }
        } else {
            for timer in timers {
                let snapshot = timer.snapshot();
                let total_ms = snapshot.running_time_net as f64 / 1_000_000.0;
                event!(
                    target: "perf_timers",
                    Level::DEBUG,
                    timer = %snapshot.identifier,
                    total_gross_ns = snapshot.running_time_gross,
                    total_net_ns = snapshot.running_time_net,
                    total_net_ms = total_ms,
                    call_count = snapshot.call_count,
                    "perf_timer_totals"
                );
            }
        }
    }
}

#[cfg(feature = "perf_timers")]
static PERF_GATHER_REGISTRY: OnceLock<PerfGatherRegistry> = OnceLock::new();

#[cfg(feature = "perf_timers")]
fn perf_registry() -> &'static PerfGatherRegistry {
    PERF_GATHER_REGISTRY.get_or_init(PerfGatherRegistry::default)
}

#[cfg(feature = "perf_timers")]
pub fn register_perf_timer(identifier: &str, net_only: bool) -> Arc<PerfGather> {
    let gather = PerfGather::new(identifier, net_only);
    perf_registry().register(gather.clone())
}

/// Performance timer for detailed analysis
pub struct PerfTimer {
    identifier: String,
    crash_with_info: bool,
    start_frame: u32,
    end_frame: Option<u32>,
    last_frame: u32,
    output_info: bool,
    running_time: Duration,
    call_count: u32,
    start_instant: Option<Instant>,
}

impl PerfTimer {
    /// Create a new performance timer
    pub fn new(
        identifier: &str,
        crash_with_info: bool,
        start_frame: u32,
        end_frame: Option<u32>,
    ) -> Self {
        Self {
            identifier: identifier.to_string(),
            crash_with_info,
            start_frame,
            end_frame,
            last_frame: 0,
            output_info: true,
            running_time: Duration::default(),
            call_count: 0,
            start_instant: None,
        }
    }

    /// Start timing
    pub fn start_timer(&mut self) {
        let current_frame = self.get_current_frame();
        if current_frame >= self.start_frame
            && (self.end_frame.is_none() || current_frame <= self.end_frame.unwrap())
        {
            self.start_instant = Some(Instant::now());
        }
    }

    /// Stop timing
    pub fn stop_timer(&mut self) {
        let current_frame = self.get_current_frame();
        if let Some(start) = self.start_instant.take() {
            if current_frame >= self.start_frame
                && (self.end_frame.is_none() || current_frame <= self.end_frame.unwrap())
            {
                self.running_time += start.elapsed();
                self.call_count += 1;
                self.last_frame = current_frame;
            }
        }

        if let Some(end_frame) = self.end_frame {
            if current_frame >= end_frame {
                self.output_info();
            }
        }
    }

    /// Output performance information
    fn output_info(&mut self) {
        if !self.output_info {
            return;
        }
        self.output_info = false;

        let total_time_ms = self.running_time.as_secs_f64() * 1000.0;
        let frame_count = (self.last_frame - self.start_frame + 1) as f64;
        let avg_time_per_frame = total_time_ms / frame_count;
        let avg_time_per_call = total_time_ms / self.call_count as f64;
        let avg_calls_per_frame = self.call_count as f64 / frame_count;
        let max_possible_fps = 1000.0 / avg_time_per_frame;

        let output = format!(
            "{}\n\
            Average Time (per call): {:.4} ms\n\
            Average Time (per frame): {:.4} ms\n\
            Average calls per frame: {:.2}\n\
            Number of calls: {}\n\
            Max possible FPS: {:.4}",
            self.identifier,
            avg_time_per_call,
            avg_time_per_frame,
            avg_calls_per_frame,
            self.call_count,
            max_possible_fps
        );

        if self.crash_with_info {
            panic!("Performance info:\n{}", output);
        } else {
            println!("Performance info:\n{}", output);
        }
    }

    /// Show metrics (for real-time display)
    pub fn show_metrics(&mut self) -> Option<String> {
        let total_time_ms = self.running_time.as_secs_f64() * 1000.0;
        let frame_count = (self.last_frame - self.start_frame + 1) as f64;
        let avg_time_per_frame = total_time_ms / frame_count;
        let avg_time_per_call = total_time_ms / self.call_count as f64;

        let result = format!(
            "{}: {:.2}ms / call, {:.2}ms / frame",
            self.identifier, avg_time_per_call, avg_time_per_frame
        );

        // Reset for next measurement period
        self.call_count = 0;
        self.running_time = Duration::default();
        let current_frame = self.get_current_frame();
        self.start_frame = current_frame + 1;
        if self.end_frame.is_some() {
            self.end_frame = Some(self.start_frame + PERFMETRICS_BETWEEN_METRICS);
        }

        Some(result)
    }

    /// Get current frame (placeholder)
    fn get_current_frame(&self) -> u32 {
        time::frame()
    }

    /// Get identifier
    pub fn get_identifier(&self) -> &str {
        &self.identifier
    }

    /// Get total running time
    pub fn get_running_time(&self) -> Duration {
        self.running_time
    }

    /// Get call count
    pub fn get_call_count(&self) -> u32 {
        self.call_count
    }
}

impl Drop for PerfTimer {
    fn drop(&mut self) {
        if self.end_frame.is_none() {
            self.output_info();
        }
    }
}

/// Performance metrics output manager
pub struct PerfMetricsOutput {
    output_stats: HashMap<String, String>,
}

impl PerfMetricsOutput {
    pub fn new() -> Self {
        Self {
            output_stats: HashMap::new(),
        }
    }

    pub fn get_stats_string(&mut self, id: &str) -> &mut String {
        self.output_stats
            .entry(id.to_string())
            .or_insert_with(String::new)
    }

    pub fn clear_stats_string(&mut self, id: &str) {
        self.output_stats.remove(id);
    }

    pub fn get_all_stats(&self) -> &HashMap<String, String> {
        &self.output_stats
    }

    pub fn display_metrics(&self) {
        println!("Performance Metrics:");
        for (id, stats) in &self.output_stats {
            println!("{}: {}", id, stats);
        }
    }
}

impl Default for PerfMetricsOutput {
    fn default() -> Self {
        Self::new()
    }
}

// Global performance metrics instance
lazy_static::lazy_static! {
    static ref PERF_METRICS: Mutex<PerfMetricsOutput> = Mutex::new(PerfMetricsOutput {
        output_stats: HashMap::new(),
    });
}

/// Get global performance metrics
pub fn get_perf_metrics() -> std::sync::MutexGuard<'static, PerfMetricsOutput> {
    PERF_METRICS.lock().unwrap()
}

/// Macros for performance timing (enabled only with perf_timers feature)
#[macro_export]
macro_rules! declare_perf_timer {
    ($id:ident) => {
        #[cfg(feature = "perf_timers")]
        fn get_perf_timer_handle() -> std::sync::Arc<$crate::common::perf_timer::PerfGather> {
            static INSTANCE: std::sync::OnceLock<
                std::sync::Arc<$crate::common::perf_timer::PerfGather>,
            > = std::sync::OnceLock::new();

            INSTANCE
                .get_or_init(|| {
                    $crate::common::perf_timer::register_perf_timer(stringify!($id), true)
                })
                .clone()
        }
    };
}

#[macro_export]
macro_rules! declare_total_perf_timer {
    ($id:ident) => {
        #[cfg(feature = "perf_timers")]
        fn get_total_perf_timer_handle() -> std::sync::Arc<$crate::common::perf_timer::PerfGather> {
            static INSTANCE: std::sync::OnceLock<
                std::sync::Arc<$crate::common::perf_timer::PerfGather>,
            > = std::sync::OnceLock::new();

            INSTANCE
                .get_or_init(|| {
                    $crate::common::perf_timer::register_perf_timer(stringify!($id), false)
                })
                .clone()
        }
    };
}

#[macro_export]
macro_rules! use_perf_timer {
    ($id:ident) => {
        #[cfg(feature = "perf_timers")]
        let _auto_perf =
            $crate::common::perf_timer::AutoPerfGather::new(get_perf_timer_handle(), false);
    };
}

#[macro_export]
macro_rules! ignore_perf_timer {
    ($id:ident) => {
        #[cfg(feature = "perf_timers")]
        let _auto_perf =
            $crate::common::perf_timer::AutoPerfGather::new(get_perf_timer_handle(), true);
    };
}

/// No-op versions when performance timers are disabled
#[cfg(not(feature = "perf_timers"))]
pub struct PerfGather;

#[cfg(not(feature = "perf_timers"))]
impl PerfGather {
    pub fn new(_identifier: &str, _net_only: bool) -> Self {
        Self
    }
    pub fn start_timer(&mut self) {}
    pub fn stop_timer(&mut self) {}
    pub fn reset(&mut self) {}
    pub fn reset_all() {}
    pub fn dump_all(_frame: u32) {}
    pub fn display_graph(_frame: u32) {}
}

#[cfg(not(feature = "perf_timers"))]
pub struct AutoPerfGather;

#[cfg(not(feature = "perf_timers"))]
impl AutoPerfGather {
    pub fn new<T>(_guard: T, _ignored: bool) -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_precision_timer() {
        let time1 = PrecisionTimer::get_time();
        sleep(Duration::from_millis(1));
        let time2 = PrecisionTimer::get_time();

        assert!(time2 > time1);
    }

    #[test]
    fn test_perf_timer() {
        let mut timer = PerfTimer::new("test", false, 0, None);

        timer.start_timer();
        sleep(Duration::from_millis(1));
        timer.stop_timer();

        assert!(timer.get_call_count() > 0);
        assert!(timer.get_running_time() > Duration::default());
    }

    #[cfg(feature = "perf_timers")]
    #[test]
    fn perf_gather_records_samples() {
        let gather = super::register_perf_timer("perf_gather_records_samples", true);
        super::PerfGather::reset_all();

        {
            let _guard = super::AutoPerfGather::new(gather.clone(), false);
            sleep(Duration::from_millis(1));
        }

        let snapshot = gather.snapshot();
        assert_eq!(snapshot.call_count, 1);
        assert!(snapshot.running_time_gross > 0);
    }

    #[cfg(feature = "perf_timers")]
    #[test]
    fn perf_gather_ignores_samples() {
        let gather = super::register_perf_timer("perf_gather_ignores_samples", true);
        super::PerfGather::reset_all();

        {
            let _guard = super::AutoPerfGather::new(gather.clone(), true);
            sleep(Duration::from_millis(1));
        }

        let snapshot = gather.snapshot();
        assert_eq!(snapshot.call_count, 0);
        assert_eq!(snapshot.running_time_gross, 0);
    }

    #[test]
    fn test_perf_metrics_output() {
        let mut metrics = PerfMetricsOutput::new();

        {
            let stats = metrics.get_stats_string("test");
            stats.push_str("test data");
        }

        assert!(metrics.get_all_stats().contains_key("test"));
        assert_eq!(metrics.get_all_stats()["test"], "test data");
    }
}
