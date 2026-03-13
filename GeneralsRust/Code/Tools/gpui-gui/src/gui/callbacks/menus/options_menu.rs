use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/OptionsMenu.cpp",
    "crate::gui::callbacks::menus::options_menu",
    "Options Menu",
    "Options shell callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "OptionsMenu",
    "Options",
    "Audio, video, gameplay, and control options.",
    "Shell",
);
