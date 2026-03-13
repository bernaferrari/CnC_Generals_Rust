use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLCustomScoreScreen.cpp",
    "crate::gui::callbacks::menus::wol_custom_score_screen",
    "WOL Custom Score Screen",
    "Custom online score callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLCustomScoreScreen",
    "WOL Custom Score",
    "Custom online match score screen.",
    "WOL",
);
