use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/LanGameOptionsMenu.cpp",
    "crate::gui::callbacks::menus::lan_game_options_menu",
    "LAN Game Options Menu",
    "LAN game-options callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "LanGameOptionsMenu",
    "LAN Game Options",
    "Configure a LAN match before launch.",
    "LAN",
);
