use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/PopupReplay.cpp",
    "crate::gui::callbacks::menus::popup_replay",
    "Popup Replay",
    "Replay-save popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "PopupReplay",
    "Replay Popup",
    "Replay save and naming popup.",
    "Popup",
);
