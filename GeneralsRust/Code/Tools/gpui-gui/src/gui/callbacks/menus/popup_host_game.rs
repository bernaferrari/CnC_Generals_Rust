use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/PopupHostGame.cpp",
    "crate::gui::callbacks::menus::popup_host_game",
    "Popup Host Game",
    "Host-game popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "PopupHostGame",
    "Host Game",
    "Host-game popup and confirmation flow.",
    "Popup",
);
