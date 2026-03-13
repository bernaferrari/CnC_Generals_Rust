use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

use super::lan_lobby_menu::LanLobbyMenuPort;
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLLobbyMenu.cpp",
    "crate::gui::callbacks::menus::wol_lobby_menu",
    "WOL Lobby Menu",
    "Online lobby callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLLobbyMenu",
    "WOL Lobby",
    "Online lobby and room list.",
    "WOL",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WolLobbyMenuPort {
    pub lobby: LanLobbyMenuPort,
    pub room_id: String,
    pub ranked_room: bool,
}

impl Default for WolLobbyMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl WolLobbyMenuPort {
    pub fn sample() -> Self {
        Self {
            lobby: LanLobbyMenuPort::sample(),
            room_id: "Room #ZH-2481".to_string(),
            ranked_room: false,
        }
    }
}
