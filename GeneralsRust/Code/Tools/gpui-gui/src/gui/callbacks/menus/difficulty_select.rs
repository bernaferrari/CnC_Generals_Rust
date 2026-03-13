use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/DifficultySelect.cpp",
    "crate::gui::callbacks::menus::difficulty_select",
    "Difficulty Select",
    "Difficulty popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "DifficultySelect",
    "Difficulty Select",
    "Difficulty-selection popup.",
    "Popup",
);
