use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/NetworkDirectConnect.cpp",
    "crate::gui::callbacks::menus::network_direct_connect",
    "Network Direct Connect",
    "Direct-connect callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "NetworkDirectConnect",
    "Direct Connect",
    "Manual network direct-connect flow.",
    "Shell",
);
