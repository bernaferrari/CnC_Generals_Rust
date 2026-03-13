use crate::gui::source_catalog::{ControlBarPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "ControlBar/ControlBarOCLTimer.cpp",
    "crate::gui::control_bar::control_bar_ocl_timer",
    "Control Bar OCL Timer",
    "Ports OCL countdown and timer-driven progress presentation.",
);

pub const PORT: ControlBarPort = ControlBarPort::new(
    &RECORD,
    "OCL Timer",
    "Countdown and timed progress elements.",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlBarOclTimerPort {
    pub timer_name: String,
    pub total_frames: u16,
    pub remaining_frames: u16,
}

impl Default for ControlBarOclTimerPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl ControlBarOclTimerPort {
    pub fn progress(&self) -> f32 {
        if self.total_frames == 0 {
            return 1.0;
        }
        1.0 - (self.remaining_frames as f32 / self.total_frames as f32)
    }

    pub fn tick(&mut self) {
        self.remaining_frames = self.remaining_frames.saturating_sub(1);
    }

    pub fn sample() -> Self {
        Self {
            timer_name: "Particle Cannon".to_string(),
            total_frames: 300,
            remaining_frames: 177,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticking_reduces_remaining_frames() {
        let mut timer = ControlBarOclTimerPort::sample();
        timer.tick();
        assert_eq!(timer.remaining_frames, 176);
    }
}
