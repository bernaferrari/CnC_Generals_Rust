use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLLadderScreen.cpp",
    "crate::gui::callbacks::menus::wol_ladder_screen",
    "WOL Ladder Screen",
    "WOL ladder callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLLadderScreen",
    "WOL Ladder",
    "Online ladder ranking screen.",
    "WOL",
);
