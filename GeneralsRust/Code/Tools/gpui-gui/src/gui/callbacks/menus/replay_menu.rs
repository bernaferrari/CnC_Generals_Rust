use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/ReplayMenu.cpp",
    "crate::gui::callbacks::menus::replay_menu",
    "Replay Menu",
    "Replay-browser callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "ReplayMenu",
    "Replay Menu",
    "Browse and launch saved replays.",
    "Shell",
);
