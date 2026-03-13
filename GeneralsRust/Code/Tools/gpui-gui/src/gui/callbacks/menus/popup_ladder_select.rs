use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/PopupLadderSelect.cpp",
    "crate::gui::callbacks::menus::popup_ladder_select",
    "Popup Ladder Select",
    "Ladder-select popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "PopupLadderSelect",
    "Ladder Select",
    "Popup ladder selection dialog.",
    "Popup",
);
