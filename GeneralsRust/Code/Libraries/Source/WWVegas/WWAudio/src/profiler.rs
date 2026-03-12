//! Performance profiling and monitoring for audio operations.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
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
pub struct AudioProfiler {
    enabled: bool,
    metrics: Arc<RwLock<HashMap<ProfileEvent, ProfileMetrics>>>,
    active_sessions: Arc<RwLock<HashMap<u64, ProfileSession>>>,
    next_session_id: Arc<RwLock<u64>>,
}

impl AudioProfiler {
    /// Create new audio profiler
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            metrics: Arc::new(RwLock::new(HashMap::new())),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            next_session_id: Arc::new(RwLock::new(0)),
        }
    }

    /// Start profiling an event
    pub fn start_event(&self, event: ProfileEvent) -> u64 {
        if !self.enabled {
            return 0;
        }

        let session_id = {
            let mut next_id = self.next_session_id.write();
            *next_id += 1;
            *next_id
        };

        let session = ProfileSession {
            event,
            start_time: Instant::now(),
        };

        self.active_sessions.write().insert(session_id, session);
        session_id
    }

    /// End profiling an event
    pub fn end_event(&self, session_id: u64) {
        if !self.enabled || session_id == 0 {
            return;
        }

        let session = {
            let mut active = self.active_sessions.write();
            active.remove(&session_id)
        };

        if let Some(session) = session {
            let duration = session.start_time.elapsed();
            self.record_event(session.event, duration);
        }
    }

    /// Record an instant event
    pub fn record_instant(&self, event: ProfileEvent) {
        if !self.enabled {
            return;
        }
        self.record_event(event, Duration::ZERO);
    }

    /// Get metrics for all events
    pub fn get_metrics(&self) -> HashMap<ProfileEvent, ProfileMetrics> {
        self.metrics.read().clone()
    }

    /// Get metrics for specific event
    pub fn get_event_metrics(&self, event: &ProfileEvent) -> Option<ProfileMetrics> {
        self.metrics.read().get(event).cloned()
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.metrics.write().clear();
        self.active_sessions.write().clear();
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

    fn record_event(&self, event: ProfileEvent, duration: Duration) {
        let mut metrics = self.metrics.write();
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
    profiler: &'a AudioProfiler,
    session_id: u64,
}

impl<'a> ProfileGuard<'a> {
    /// Create new profile guard
    pub fn new(profiler: &'a AudioProfiler, event: ProfileEvent) -> Self {
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
