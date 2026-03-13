use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/KeyboardOptionsMenu.cpp",
    "crate::gui::callbacks::menus::keyboard_options_menu",
    "Keyboard Options Menu",
    "Keyboard configuration callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "KeyboardOptionsMenu",
    "Keyboard Options",
    "Key binding and keyboard settings.",
    "Shell",
);
