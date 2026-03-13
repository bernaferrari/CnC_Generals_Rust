use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/PopupJoinGame.cpp",
    "crate::gui::callbacks::menus::popup_join_game",
    "Popup Join Game",
    "Join-game popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "PopupJoinGame",
    "Join Game",
    "Join-game popup and password flow.",
    "Popup",
);
