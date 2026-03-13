use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLMapSelectMenu.cpp",
    "crate::gui::callbacks::menus::wol_map_select_menu",
    "WOL Map Select Menu",
    "Online map-selection callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLMapSelectMenu",
    "WOL Maps",
    "Select maps for online sessions.",
    "WOL",
);
