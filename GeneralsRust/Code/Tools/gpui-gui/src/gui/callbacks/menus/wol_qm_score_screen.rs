use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLQMScoreScreen.cpp",
    "crate::gui::callbacks::menus::wol_qm_score_screen",
    "WOL QM Score Screen",
    "Quick-match score callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLQMScoreScreen",
    "WOL QM Score",
    "Quick-match post-game score screen.",
    "WOL",
);
