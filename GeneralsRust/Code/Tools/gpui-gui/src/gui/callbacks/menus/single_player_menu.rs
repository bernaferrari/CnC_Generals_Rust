use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/SinglePlayerMenu.cpp",
    "crate::gui::callbacks::menus::single_player_menu",
    "Single Player Menu",
    "Single-player mode selection shell.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "SinglePlayerMenu",
    "Single Player",
    "Campaign and challenge entry points.",
    "Shell",
);
