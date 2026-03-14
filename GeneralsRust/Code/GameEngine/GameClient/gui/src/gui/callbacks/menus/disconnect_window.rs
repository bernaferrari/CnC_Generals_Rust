use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/DisconnectWindow.cpp",
    "crate::gui::callbacks::menus::disconnect_window",
    "Disconnect Window",
    "Disconnect-window callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "DisconnectWindow",
    "Disconnect",
    "Disconnect and connection-loss handling screen.",
    "Popup",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DisconnectWindowPort {
    pub headline: String,
    pub reason: String,
    pub reconnect_allowed: bool,
    pub elapsed_ms: u32,
    pub timeout_ms: u32,
}

impl Default for DisconnectWindowPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl DisconnectWindowPort {
    pub fn tick(&mut self, delta_ms: u32) {
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
    }

    pub fn timed_out(&self) -> bool {
        self.elapsed_ms >= self.timeout_ms
    }

    pub fn sample() -> Self {
        Self {
            headline: "Connection Lost".to_string(),
            reason: "The remote host stopped responding during match setup.".to_string(),
            reconnect_allowed: false,
            elapsed_ms: 2_500,
            timeout_ms: 5_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_flips_after_elapsed_time() {
        let mut window = DisconnectWindowPort::sample();
        assert!(!window.timed_out());
        window.tick(3_000);
        assert!(window.timed_out());
    }
}
