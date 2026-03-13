use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
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
