use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLStatusMenu.cpp",
    "crate::gui::callbacks::menus::wol_status_menu",
    "WOL Status Menu",
    "WOL status callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLStatusMenu",
    "WOL Status",
    "Online status and service state screen.",
    "WOL",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WolStatusMenuPort {
    pub service_name: String,
    pub status_lines: Vec<String>,
    pub can_disconnect: bool,
}

impl Default for WolStatusMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl WolStatusMenuPort {
    pub fn sample() -> Self {
        Self {
            service_name: "GameSpy Services".to_string(),
            status_lines: vec![
                "Authenticating profile".to_string(),
                "Fetching ladder data".to_string(),
                "Connection stable".to_string(),
            ],
            can_disconnect: true,
        }
    }
}
