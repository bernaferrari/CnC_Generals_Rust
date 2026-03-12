use std::time::Duration;

use parking_lot::RwLock;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct SimulationClock {
    frame: u32,
    elapsed: Duration,
    tick_delta: Duration,
}

impl SimulationClock {
    pub fn new(target_fps: u32) -> Self {
        Self {
            frame: 0,
            elapsed: Duration::ZERO,
            tick_delta: fps_to_delta(target_fps),
        }
    }

    pub fn advance(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        self.elapsed += self.tick_delta;
    }

    pub fn frame(&self) -> u32 {
        self.frame
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub fn delta(&self) -> Duration {
        self.tick_delta
    }

    pub fn reset(&mut self) {
        self.frame = 0;
        self.elapsed = Duration::ZERO;
    }

    pub fn set_tick_rate(&mut self, target_fps: u32) {
        self.tick_delta = fps_to_delta(target_fps);
    }
}

impl Default for SimulationClock {
    fn default() -> Self {
        Self::new(30)
    }
}

static SIM_CLOCK: OnceLock<RwLock<SimulationClock>> = OnceLock::new();

fn clock_cell() -> &'static RwLock<SimulationClock> {
    SIM_CLOCK.get_or_init(|| RwLock::new(SimulationClock::default()))
}

fn fps_to_delta(target_fps: u32) -> Duration {
    if target_fps == 0 {
        Duration::from_secs_f32(1.0 / 30.0)
    } else {
        Duration::from_secs_f64(1.0 / target_fps as f64)
    }
}

pub fn initialize(target_fps: u32) {
    let mut guard = clock_cell().write();
    guard.set_tick_rate(target_fps);
    guard.reset();
}

pub fn set_tick_rate(target_fps: u32) {
    clock_cell().write().set_tick_rate(target_fps);
}

pub fn advance() {
    clock_cell().write().advance();
}

pub fn reset() {
    clock_cell().write().reset();
}

pub fn frame() -> u32 {
    clock_cell().read().frame()
}

pub fn elapsed() -> Duration {
    clock_cell().read().elapsed()
}

pub fn delta() -> Duration {
    clock_cell().read().delta()
}

pub fn snapshot() -> SimulationClock {
    clock_cell().read().clone()
}
