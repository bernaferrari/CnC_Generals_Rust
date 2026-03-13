use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/InGamePopupMessage.cpp",
    "crate::gui::callbacks::in_game_popup_message",
    "In-Game Popup Message",
    "Ports transient popup messaging shown during gameplay events.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "Popup Message",
    "Transient in-game popup messaging callbacks.",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InGamePopupMessagePort {
    pub visible: bool,
    pub message: String,
    pub fade_lifetime_frames: u16,
    pub elapsed_frames: u16,
}

impl Default for InGamePopupMessagePort {
    fn default() -> Self {
        Self::sample()
    }
}

impl InGamePopupMessagePort {
    pub fn sample() -> Self {
        Self {
            visible: true,
            message: "General promotion available".to_string(),
            fade_lifetime_frames: 90,
            elapsed_frames: 25,
        }
    }

    pub fn progress(&self) -> f32 {
        if self.fade_lifetime_frames == 0 {
            return 1.0;
        }
        self.elapsed_frames as f32 / self.fade_lifetime_frames as f32
    }

    pub fn tick(&mut self) {
        self.elapsed_frames = self.elapsed_frames.saturating_add(1);
        if self.elapsed_frames >= self.fade_lifetime_frames {
            self.visible = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popup_hides_after_fade_lifetime() {
        let mut popup = InGamePopupMessagePort {
            fade_lifetime_frames: 2,
            elapsed_frames: 1,
            ..InGamePopupMessagePort::sample()
        };
        popup.tick();

        assert!(!popup.visible);
    }
}
