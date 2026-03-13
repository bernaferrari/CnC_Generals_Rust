use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLMessageWindow.cpp",
    "crate::gui::callbacks::menus::wol_message_window",
    "WOL Message Window",
    "WOL message callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLMessageWindow",
    "WOL Messages",
    "Online messages and inbox screen.",
    "WOL",
);
