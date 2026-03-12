//! Performance timer utilities

use crate::common::*;
use std::time::Instant;

/// Performance timer for measuring execution time
pub struct PerfTimer {
    start_time: Instant,
}

impl PerfTimer {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Real {
        self.start_time.elapsed().as_secs_f32()
    }

    pub fn reset(&mut self) {
        self.start_time = Instant::now();
    }
}
