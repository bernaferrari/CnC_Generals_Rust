use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/ReplayControls.cpp",
    "crate::gui::callbacks::replay_controls",
    "Replay Controls",
    "Ports replay playback controls and time navigation callbacks.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "Replay Controls",
    "Replay playback and timeline callbacks.",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayPlaybackStatePort {
    Playing,
    Paused,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayControlsPort {
    pub playback_state: ReplayPlaybackStatePort,
    pub speed_multiplier: u8,
    pub timeline_position: f32,
}

impl Default for ReplayControlsPort {
    fn default() -> Self {
        Self {
            playback_state: ReplayPlaybackStatePort::Paused,
            speed_multiplier: 1,
            timeline_position: 0.0,
        }
    }
}

impl ReplayControlsPort {
    pub fn play(&mut self) {
        self.playback_state = ReplayPlaybackStatePort::Playing;
    }

    pub fn pause(&mut self) {
        self.playback_state = ReplayPlaybackStatePort::Paused;
    }

    pub fn set_speed(&mut self, multiplier: u8) {
        self.speed_multiplier = multiplier.max(1);
    }

    pub fn seek(&mut self, position: f32) {
        self.timeline_position = position.clamp(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_and_pause_update_state() {
        let mut replay = ReplayControlsPort::default();
        replay.play();
        assert_eq!(replay.playback_state, ReplayPlaybackStatePort::Playing);
        replay.pause();
        assert_eq!(replay.playback_state, ReplayPlaybackStatePort::Paused);
    }

    #[test]
    fn seek_clamps_to_timeline_bounds() {
        let mut replay = ReplayControlsPort::default();
        replay.seek(1.5);
        assert_eq!(replay.timeline_position, 1.0);
    }
}
