use std::cell::RefCell;
use std::time::Duration;

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

// THREAD: C++ plain static; driven only by the single game/client thread.
thread_local! {
    static SIM_CLOCK: RefCell<SimulationClock> = RefCell::new(SimulationClock::default());
}

fn with_clock<R>(f: impl FnOnce(&mut SimulationClock) -> R) -> R {
    SIM_CLOCK.with_borrow_mut(f)
}

fn fps_to_delta(target_fps: u32) -> Duration {
    if target_fps == 0 {
        Duration::from_secs_f32(1.0 / 30.0)
    } else {
        Duration::from_secs_f64(1.0 / target_fps as f64)
    }
}

pub fn initialize(target_fps: u32) {
    with_clock(|clock| {
        clock.set_tick_rate(target_fps);
        clock.reset();
    });
}

pub fn set_tick_rate(target_fps: u32) {
    with_clock(|clock| clock.set_tick_rate(target_fps));
}

pub fn advance() {
    with_clock(|clock| clock.advance());
}

pub fn reset() {
    with_clock(|clock| clock.reset());
}

pub fn frame() -> u32 {
    SIM_CLOCK.with_borrow(|clock| clock.frame())
}

pub fn elapsed() -> Duration {
    SIM_CLOCK.with_borrow(|clock| clock.elapsed())
}

pub fn delta() -> Duration {
    SIM_CLOCK.with_borrow(|clock| clock.delta())
}

pub fn snapshot() -> SimulationClock {
    SIM_CLOCK.with_borrow(|clock| clock.clone())
}
