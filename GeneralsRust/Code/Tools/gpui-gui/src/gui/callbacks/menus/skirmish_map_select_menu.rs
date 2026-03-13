use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/SkirmishMapSelectMenu.cpp",
    "crate::gui::callbacks::menus::skirmish_map_select_menu",
    "Skirmish Map Select Menu",
    "Skirmish map selection callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "SkirmishMapSelectMenu",
    "Skirmish Maps",
    "Select a skirmish battleground.",
    "Shell",
);
