use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/PopupHostGame.cpp",
    "crate::gui::callbacks::menus::popup_host_game",
    "Popup Host Game",
    "Host-game popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "PopupHostGame",
    "Host Game",
    "Host-game popup and confirmation flow.",
    "Popup",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PopupHostGamePort {
    pub game_name: String,
    pub game_description: String,
    pub ladder_password: String,
    pub game_password: String,
    pub allow_observers: bool,
    pub use_stats: bool,
    pub limit_armies: bool,
    pub selected_ladder: String,
}

impl Default for PopupHostGamePort {
    fn default() -> Self {
        Self::sample()
    }
}

impl PopupHostGamePort {
    pub fn sample() -> Self {
        Self {
            game_name: "ZH Ladder Practice".to_string(),
            game_description: "2v2 warmup lobby".to_string(),
            ladder_password: String::new(),
            game_password: "desert".to_string(),
            allow_observers: true,
            use_stats: true,
            limit_armies: false,
            selected_ladder: "Ranked 1v1".to_string(),
        }
    }
}
