use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/EstablishConnectionsWindow.cpp",
    "crate::gui::callbacks::menus::establish_connections_window",
    "Establish Connections Window",
    "Connection-establishment callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "EstablishConnectionsWindow",
    "Establish Connections",
    "Connection-establishment progress screen.",
    "Popup",
);
