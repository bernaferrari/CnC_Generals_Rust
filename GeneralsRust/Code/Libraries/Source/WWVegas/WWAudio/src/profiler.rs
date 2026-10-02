//! Performance profiling and monitoring for audio operations.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Profiling event types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProfileEvent {
    DeviceOpen,
    DeviceClose,
    ChannelCreate,
    ChannelDestroy,
    SourceLoad,
    SourcePlay,
    BufferFill,
    AudioMix,
    CompressionDecode,
    CacheHit,
    CacheMiss,
}

/// Performance metrics for a profiling event
#[derive(Debug, Clone)]
pub struct ProfileMetrics {
    pub event: ProfileEvent,
    pub total_calls: u64,
    pub total_duration: Duration,
    pub average_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
    pub last_call: Instant,
}

/// Active profiling session
struct ProfileSession {
    event: ProfileEvent,
    start_time: Instant,
}

/// Audio profiler for performance monitoring
///
/// Plain owned state: the profiler is driven by a single owner (the thread
/// that instruments the audio paths), so no shared lock is required.
pub struct AudioProfiler {
    enabled: bool,
    metrics: HashMap<ProfileEvent, ProfileMetrics>,
    active_sessions: HashMap<u64, ProfileSession>,
    next_session_id: u64,
}

impl AudioProfiler {
    /// Create new audio profiler
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            metrics: HashMap::new(),
            active_sessions: HashMap::new(),
            next_session_id: 0,
        }
    }

    /// Start profiling an event
    pub fn start_event(&mut self, event: ProfileEvent) -> u64 {
        if !self.enabled {
            return 0;
        }

        self.next_session_id += 1;
        let session_id = self.next_session_id;

        let session = ProfileSession {
            event,
            start_time: Instant::now(),
        };

        self.active_sessions.insert(session_id, session);
        session_id
    }

    /// End profiling an event
    pub fn end_event(&mut self, session_id: u64) {
        if !self.enabled || session_id == 0 {
            return;
        }

        let session = self.active_sessions.remove(&session_id);

        if let Some(session) = session {
            let duration = session.start_time.elapsed();
            self.record_event(session.event, duration);
        }
    }

    /// Record an instant event
    pub fn record_instant(&mut self, event: ProfileEvent) {
        if !self.enabled {
            return;
        }
        self.record_event(event, Duration::ZERO);
    }

    /// Get metrics for all events
    pub fn get_metrics(&self) -> HashMap<ProfileEvent, ProfileMetrics> {
        self.metrics.clone()
    }

    /// Get metrics for specific event
    pub fn get_event_metrics(&self, event: &ProfileEvent) -> Option<ProfileMetrics> {
        self.metrics.get(event).cloned()
    }

    /// Reset all metrics
    pub fn reset(&mut self) {
        self.metrics.clear();
        self.active_sessions.clear();
    }

    /// Enable/disable profiling
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.reset();
        }
    }

    /// Check if profiling is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn record_event(&mut self, event: ProfileEvent, duration: Duration) {
        let metrics = &mut self.metrics;
        let entry = metrics
            .entry(event.clone())
            .or_insert_with(|| ProfileMetrics {
                event,
                total_calls: 0,
                total_duration: Duration::ZERO,
                average_duration: Duration::ZERO,
                min_duration: Duration::MAX,
                max_duration: Duration::ZERO,
                last_call: Instant::now(),
            });

        entry.total_calls += 1;
        entry.total_duration += duration;
        entry.average_duration = entry.total_duration / entry.total_calls as u32;
        entry.min_duration = entry.min_duration.min(duration);
        entry.max_duration = entry.max_duration.max(duration);
        entry.last_call = Instant::now();
    }
}

/// RAII profiling guard that automatically ends profiling on drop
pub struct ProfileGuard<'a> {
    profiler: &'a mut AudioProfiler,
    session_id: u64,
}

impl<'a> ProfileGuard<'a> {
    /// Create new profile guard
    pub fn new(profiler: &'a mut AudioProfiler, event: ProfileEvent) -> Self {
        let session_id = profiler.start_event(event);
        Self {
            profiler,
            session_id,
        }
    }
}

impl<'a> Drop for ProfileGuard<'a> {
    fn drop(&mut self) {
        self.profiler.end_event(self.session_id);
    }
}

/// Convenience macro for profiling code blocks
#[macro_export]
macro_rules! profile {
    ($profiler:expr, $event:expr, $code:block) => {{
        let _guard = $crate::profiler::ProfileGuard::new($profiler, $event);
        $code
    }};
}

impl Default for AudioProfiler {
    fn default() -> Self {
        Self::new(cfg!(debug_assertions))
    }
}
