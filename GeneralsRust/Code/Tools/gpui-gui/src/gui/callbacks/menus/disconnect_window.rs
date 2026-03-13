use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/DisconnectWindow.cpp",
    "crate::gui::callbacks::menus::disconnect_window",
    "Disconnect Window",
    "Disconnect-window callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "DisconnectWindow",
    "Disconnect",
    "Disconnect and connection-loss handling screen.",
    "Popup",
);
