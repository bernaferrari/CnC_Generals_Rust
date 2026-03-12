use log::info;
use std::time::{Duration, SystemTime};

/// RAII helper for logging initialization durations that mirrors the C++ profiling prints.
pub struct InitTimer {
    label: &'static str,
    start: SystemTime,
    log_on_drop: bool,
}

impl InitTimer {
    pub fn new(label: &'static str) -> Self {
        Self {
            label,
            start: SystemTime::now(),
            log_on_drop: true,
        }
    }

    pub fn finish(mut self) -> Duration {
        self.log_on_drop = false;
        let duration = self.start.elapsed().unwrap_or_default();
        info!("{} completed in {:.2}s", self.label, duration.as_secs_f32());
        duration
    }
}

impl Drop for InitTimer {
    fn drop(&mut self) {
        if self.log_on_drop {
            if let Ok(duration) = self.start.elapsed() {
                info!("{} completed in {:.2}s", self.label, duration.as_secs_f32());
            }
        }
    }
}
