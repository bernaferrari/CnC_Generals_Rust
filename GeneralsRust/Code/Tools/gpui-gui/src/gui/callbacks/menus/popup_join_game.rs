use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/PopupJoinGame.cpp",
    "crate::gui::callbacks::menus::popup_join_game",
    "Popup Join Game",
    "Join-game popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "PopupJoinGame",
    "Join Game",
    "Join-game popup and password flow.",
    "Popup",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PopupJoinGamePort {
    pub game_name: String,
    pub password: String,
    pub can_join: bool,
}

impl Default for PopupJoinGamePort {
    fn default() -> Self {
        Self::sample()
    }
}

impl PopupJoinGamePort {
    pub fn sample() -> Self {
        Self {
            game_name: "ZH Ladder Practice".to_string(),
            password: "desert".to_string(),
            can_join: true,
        }
    }
}
