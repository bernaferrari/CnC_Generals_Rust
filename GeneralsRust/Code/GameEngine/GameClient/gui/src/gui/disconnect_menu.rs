use crate::gui::source_catalog::GuiPortRecord;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "DisconnectMenu/DisconnectMenu.cpp",
    "crate::gui::disconnect_menu",
    "Disconnect Menu",
    "Ports the disconnect notification layout and reconnect/back-out flow.",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisconnectState {
    Idle,
    Warning,
    Lost,
}

#[derive(Clone, Debug)]
pub struct DisconnectMenuPort {
    pub state: DisconnectState,
    pub headline: String,
    pub detail: String,
}

impl Default for DisconnectMenuPort {
    fn default() -> Self {
        Self {
            state: DisconnectState::Idle,
            headline: "Connected".to_string(),
            detail: "No active disconnect warnings.".to_string(),
        }
    }
}

impl DisconnectMenuPort {
    pub fn mark_warning(&mut self, detail: impl Into<String>) {
        self.state = DisconnectState::Warning;
        self.headline = "Connection Warning".to_string();
        self.detail = detail.into();
    }

    pub fn mark_lost(&mut self, detail: impl Into<String>) {
        self.state = DisconnectState::Lost;
        self.headline = "Connection Lost".to_string();
        self.detail = detail.into();
    }
}
