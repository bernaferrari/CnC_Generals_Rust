use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/MapSelectMenu.cpp",
    "crate::gui::callbacks::menus::map_select_menu",
    "Map Select Menu",
    "Map selection screen callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "MapSelectMenu",
    "Map Select",
    "Browse and choose a scenario map.",
    "Shell",
);
