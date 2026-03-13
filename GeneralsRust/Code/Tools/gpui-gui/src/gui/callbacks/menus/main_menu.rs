use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/MainMenu.cpp",
    "crate::gui::callbacks::menus::main_menu",
    "Main Menu",
    "Primary shell landing screen.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "MainMenu",
    "Main Menu",
    "Front-door shell menu for starting or configuring the game.",
    "Shell",
);
