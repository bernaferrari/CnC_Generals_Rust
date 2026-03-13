use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/PopupPlayerInfo.cpp",
    "crate::gui::callbacks::menus::popup_player_info",
    "Popup Player Info",
    "Player-info popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "PopupPlayerInfo",
    "Player Info",
    "Popup player profile and stats.",
    "Popup",
);
