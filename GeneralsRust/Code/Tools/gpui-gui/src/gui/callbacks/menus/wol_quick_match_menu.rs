use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLQuickMatchMenu.cpp",
    "crate::gui::callbacks::menus::wol_quick_match_menu",
    "WOL Quick Match Menu",
    "Quick-match callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLQuickMatchMenu",
    "Quick Match",
    "Quick-match setup and queueing.",
    "WOL",
);
