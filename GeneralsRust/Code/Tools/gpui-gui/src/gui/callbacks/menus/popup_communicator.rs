use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/PopupCommunicator.cpp",
    "crate::gui::callbacks::menus::popup_communicator",
    "Popup Communicator",
    "Communicator popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "PopupCommunicator",
    "Communicator",
    "Popup communicator and message entry flow.",
    "Popup",
);
