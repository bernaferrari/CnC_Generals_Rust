use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/SkirmishGameOptionsMenu.cpp",
    "crate::gui::callbacks::menus::skirmish_game_options_menu",
    "Skirmish Game Options Menu",
    "Skirmish setup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "SkirmishGameOptionsMenu",
    "Skirmish Setup",
    "Configure players, AI, and match rules for skirmish.",
    "Shell",
);
