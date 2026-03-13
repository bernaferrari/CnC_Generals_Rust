use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/LanMapSelectMenu.cpp",
    "crate::gui::callbacks::menus::lan_map_select_menu",
    "LAN Map Select Menu",
    "LAN map-selection callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "LanMapSelectMenu",
    "LAN Maps",
    "Choose the map for a LAN match.",
    "LAN",
);
