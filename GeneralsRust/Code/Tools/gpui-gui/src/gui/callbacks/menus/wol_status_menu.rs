use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLStatusMenu.cpp",
    "crate::gui::callbacks::menus::wol_status_menu",
    "WOL Status Menu",
    "WOL status callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLStatusMenu",
    "WOL Status",
    "Online status and service state screen.",
    "WOL",
);
