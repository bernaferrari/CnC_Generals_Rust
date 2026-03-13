use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/PopupPlayerInfo.cpp",
    "crate::gui::callbacks::menus::popup_player_info",
    "Popup Player Info",
    "Player-info popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "PopupPlayerInfo",
    "Player Info",
    "Popup player profile and stats.",
    "Popup",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PopupPlayerInfoPort {
    pub player_name: String,
    pub faction: String,
    pub clan: String,
    pub wins: u32,
    pub losses: u32,
    pub disconnects: u32,
    pub online_status: String,
}

impl Default for PopupPlayerInfoPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl PopupPlayerInfoPort {
    pub fn sample() -> Self {
        Self {
            player_name: "CommanderFox".to_string(),
            faction: "USA".to_string(),
            clan: "ZH Elite".to_string(),
            wins: 188,
            losses: 74,
            disconnects: 2,
            online_status: "In custom lobby".to_string(),
        }
    }
}
