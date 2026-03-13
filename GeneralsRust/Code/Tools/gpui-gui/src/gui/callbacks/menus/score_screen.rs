use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/ScoreScreen.cpp",
    "crate::gui::callbacks::menus::score_screen",
    "Score Screen",
    "Post-match score callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "ScoreScreen",
    "Score Screen",
    "Post-match summary and performance breakdown.",
    "HUD",
);
