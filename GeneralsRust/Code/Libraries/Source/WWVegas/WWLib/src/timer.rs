// Auto-generated C++ compatibility shim for timers
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn reset(&mut self) {
        self.start = Instant::now();
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    pub fn elapsed_seconds(&self) -> f32 {
        self.start.elapsed().as_secs_f32()
    }
}
