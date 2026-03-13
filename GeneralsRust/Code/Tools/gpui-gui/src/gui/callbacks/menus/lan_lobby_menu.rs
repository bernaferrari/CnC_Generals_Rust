use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/LanLobbyMenu.cpp",
    "crate::gui::callbacks::menus::lan_lobby_menu",
    "LAN Lobby Menu",
    "LAN lobby callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "LanLobbyMenu",
    "LAN Lobby",
    "LAN lobby player list and chat.",
    "LAN",
);
