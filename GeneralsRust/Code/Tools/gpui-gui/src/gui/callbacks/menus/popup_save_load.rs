use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/PopupSaveLoad.cpp",
    "crate::gui::callbacks::menus::popup_save_load",
    "Popup Save Load",
    "Save/load popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "SaveLoadMenu",
    "Save / Load",
    "Popup save-load flow and slot management.",
    "Popup",
);
