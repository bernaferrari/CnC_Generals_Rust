use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/GameInfoWindow.cpp",
    "crate::gui::callbacks::menus::game_info_window",
    "Game Info Window",
    "Game info popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "GameInfoWindow",
    "Game Info",
    "Read-only game details popup.",
    "Popup",
);
