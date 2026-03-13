use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLBuddyOverlay.cpp",
    "crate::gui::callbacks::menus::wol_buddy_overlay",
    "WOL Buddy Overlay",
    "Buddy overlay callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLBuddyOverlay",
    "Buddy Overlay",
    "Online buddy list overlay.",
    "WOL",
);
