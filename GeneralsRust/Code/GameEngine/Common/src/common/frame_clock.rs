use std::time::{Duration, Instant};

/// A simple clock for tracking frame time and delta time.
/// This mimics the functionality needed for the game loop.
#[derive(Debug, Clone)]
pub struct FrameClock {
    start_time: Instant,
    last_frame_time: Instant,
    frame_count: u64,
    delta_time: Duration,
}

impl FrameClock {
    /// Create a new FrameClock.
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            start_time: now,
            last_frame_time: now,
            frame_count: 0,
            delta_time: Duration::from_millis(16), // Default to ~60fps
        }
    }

    /// Advance the clock to the next frame.
    /// Returns the timing information for the new frame.
    pub fn next_frame(&mut self) -> FrameTiming {
        let now = Instant::now();
        self.delta_time = now.duration_since(self.last_frame_time);
        self.last_frame_time = now;
        self.frame_count += 1;

        FrameTiming {
            frame_number: self.frame_count,
            delta_time: self.delta_time,
            total_time: now.duration_since(self.start_time),
        }
    }

    /// Advance the clock by a fixed amount of time.
    pub fn advance_fixed(&mut self, delta: Duration) -> FrameTiming {
        self.delta_time = delta;
        self.last_frame_time += delta;
        self.frame_count += 1;

        FrameTiming {
            frame_number: self.frame_count,
            delta_time: self.delta_time,
            total_time: self.last_frame_time.duration_since(self.start_time),
        }
    }
}

impl Default for FrameClock {
    fn default() -> Self {
        Self::new()
    }
}

/// Timing information for a single frame.
#[derive(Debug, Clone, Copy)]
pub struct FrameTiming {
    pub frame_number: u64,
    pub delta_time: Duration,
    pub total_time: Duration,
}

impl FrameTiming {
    pub fn delta_seconds(&self) -> f32 {
        self.delta_time.as_secs_f32()
    }
}
