use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/PopupCommunicator.cpp",
    "crate::gui::callbacks::menus::popup_communicator",
    "Popup Communicator",
    "Communicator popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "PopupCommunicator",
    "Communicator",
    "Popup communicator and message entry flow.",
    "Popup",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PopupCommunicatorPort {
    pub recipient: String,
    pub message_entry: String,
    pub history: Vec<String>,
    pub can_send: bool,
}

impl Default for PopupCommunicatorPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl PopupCommunicatorPort {
    pub fn send(&mut self) -> bool {
        if !self.can_send || self.message_entry.trim().is_empty() {
            return false;
        }
        self.history
            .push(format!("To {}: {}", self.recipient, self.message_entry));
        self.message_entry.clear();
        true
    }

    pub fn sample() -> Self {
        Self {
            recipient: "CommanderFox".to_string(),
            message_entry: "Queue USA mirror?".to_string(),
            history: vec![
                "CommanderFox: gg last round".to_string(),
                "You: ready for one more?".to_string(),
            ],
            can_send: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sending_moves_text_into_history() {
        let mut communicator = PopupCommunicatorPort::sample();
        assert!(communicator.send());
        assert!(communicator
            .history
            .last()
            .unwrap()
            .contains("Queue USA mirror?"));
        assert!(communicator.message_entry.is_empty());
    }
}
